# game-core

`game-core` is a small Rust library for deterministic, authoritative game sessions. It provides
the mechanisms many games share while leaving every concrete game's state and rules in the
consuming application.

It is not a game implementation, lobby, network protocol, persistence layer, matchmaking system,
UI framework, economy, or random-number service. It contains no rules for any particular card,
board, simulation, or competitive game.

This crate is a reusable mechanics package, not a universal game bounded context. Consumers should
adopt the session lifecycle only when its meaning matches their occurrence exactly; Blackjack,
Baccarat, Poker, Rooms, tournaments, fairness provenance, settlement, and presentation retain
their own domain authority.

## Responsibilities

The library owns:

- validated, versioned game-definition, session, participant, and result identities;
- a strict `Created -> Active -> Finished` session lifecycle;
- optimistic concurrency through monotonic `GameVersion` checks;
- the `GameRules` boundary for validation and deterministic state transitions;
- ordered domain events and terminal outcomes returned by accepted actions;
- a minimal participant roster without game-specific roles, seats, teams, or capacity policy;
- injected randomness plus a portable, versioned seeded implementation;
- storage-neutral snapshots, restoration, and deterministic action replay.

The central model is:

```text
Current state + validated action + deterministic rules + injected randomness
                                  |
                                  v
                    New state + ordered events + optional outcome
```

## Invariants

- An action is evaluated only against an active authoritative session.
- The caller's expected version must equal the current version.
- Rejected actions do not mutate session state or consume randomness.
- Accepted actions advance the version exactly once.
- A terminal outcome finishes the session; a finished session cannot accept another action.
- A snapshot cannot represent a lifecycle/version/outcome combination the core could not produce.
- Replay uses the same validation, lifecycle, version, transition, and randomness path as live play.
- Rule meaning is pinned by a `GameDefinitionRef`, not by a mutable global name.
- `SeededRandom` exposes opaque checkpoints so adapters can persist or restore an exact stream
  position without depending on implementation fields.

## Basic usage

```rust
use game_core::{
    ActionContext, GameAction, GameDefinitionRef, GameRules, GameSession, GameSessionRef,
    GameVersion, RandomSeed, RandomSource, SeededRandom, Transition,
};

#[derive(Clone)]
struct State(u8);
struct Increment;
struct Advanced;
struct Finished;
struct Rules;

impl GameRules for Rules {
    type State = State;
    type Action = Increment;
    type Event = Advanced;
    type Outcome = Finished;
    type Error = &'static str;

    fn validate(
        &self,
        _context: &ActionContext<'_>,
        state: &State,
        _action: &Increment,
    ) -> Result<(), Self::Error> {
        (state.0 < 3).then_some(()).ok_or("already finished")
    }

    fn transition(
        &self,
        _context: &ActionContext<'_>,
        state: &State,
        _action: &Increment,
        _random: &mut dyn RandomSource,
    ) -> Transition<State, Advanced, Finished> {
        let next = State(state.0 + 1);
        if next.0 == 3 {
            Transition::finish(next, vec![Advanced], Finished)
        } else {
            Transition::continue_with(next, vec![Advanced])
        }
    }
}

let definition = GameDefinitionRef::new("example.counter", "1.0.0")?;
let reference = GameSessionRef::new(definition, "session-1")?;
let mut session = GameSession::create(reference, State(0));
session.start(GameVersion::ZERO)?;

let mut random = SeededRandom::new(RandomSeed::new(42));
let command = GameAction::new(GameVersion::new(1), None, Increment);
let accepted = session.apply(&Rules, &command, &mut random)?;
assert_eq!(accepted.next_version(), GameVersion::new(2));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Extension points

Implement `GameRules` with application-owned `State`, `Action`, `Event`, `Outcome`, and rejection
types. Put turns, rounds, phases, teams, roles, timers, and legal-move policy in those types when
the game needs them. Supply a `RandomSource` appropriate to the application, and persist
`GameSnapshot` plus accepted actions through an adapter of your choice.

When continuing from a snapshot, the supplied random stream must be restored to the point
corresponding to that snapshot. A seed plus the number/order of prior draws, or an application-owned
randomness snapshot, is part of the replay input; `game-core` never consults a process-global RNG.

The core intentionally does not define repository, clock, event-sink, HTTP, WebSocket, database,
or serialization interfaces. Those abstractions are only useful when an application chooses a
technology and an atomic persistence model. Serialization adapters should reconstruct snapshots
through `GameSnapshot::new`, which validates core invariants before restoration.

## Dependency philosophy

`game-core` has no runtime dependencies and never imports a consuming application. Applications
depend inward on this crate, implement concrete rules, and adapt storage and transport outside the
domain path. The live and replay paths should receive the same pinned definition version and
randomness algorithm.

## Development

Rust 1.94 or newer is required.

```sh
cargo build --all-targets
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo doc --no-deps
```

The repository is independently buildable and has no filesystem dependency on another project.
