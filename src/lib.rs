//! Deterministic, authoritative game-session primitives.
//!
//! `game-core` owns generic identity, lifecycle, optimistic-version,
//! participant, randomness, snapshot, and replay mechanisms. A consuming game
//! supplies its own state, actions, events, outcomes, and rules.
//!
//! The central execution boundary is [`GameRules`]: validation observes only
//! the current state and action context, while an accepted action deterministically
//! produces a [`Transition`] from injected randomness.

mod identity;
mod participant;
mod randomness;
mod replay;
mod session;

pub use identity::{
    GameDefinitionKey, GameDefinitionRef, GameDefinitionVersion, GameResultId, GameResultRef,
    GameSessionId, GameSessionRef, IdentityError, IdentityResult,
};
pub use participant::{
    Participant, ParticipantError, ParticipantId, ParticipantRoster, ParticipantStatus,
};
pub use randomness::{
    RandomError, RandomSeed, RandomSource, SEEDED_RANDOM_ALGORITHM, SeededRandom, shuffle,
};
pub use replay::{ReplayExecutionResult, ReplayResult, replay, replay_from_snapshot};
pub use session::{
    ActionContext, AppliedTransition, GameAction, GameExecutionError, GameRules, GameSession,
    GameSnapshot, GameStatus, GameVersion, LifecycleOperation, SessionError, Transition,
};
