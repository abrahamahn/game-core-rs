use game_core::{
    ActionContext, GameAction, GameDefinitionRef, GameRules, GameSessionRef, GameStatus,
    GameVersion, RandomSeed, RandomSource, SEEDED_RANDOM_ALGORITHM, SeededRandom, Transition,
    replay, replay_from_snapshot,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    turns: u8,
    total: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Roll;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rolled(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Outcome(u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuleError;

struct DiceRules;

impl GameRules for DiceRules {
    type State = State;
    type Action = Roll;
    type Event = Rolled;
    type Outcome = Outcome;
    type Error = RuleError;

    fn validate(
        &self,
        _context: &ActionContext<'_>,
        _state: &Self::State,
        _action: &Self::Action,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn transition(
        &self,
        _context: &ActionContext<'_>,
        state: &Self::State,
        _action: &Self::Action,
        random: &mut dyn RandomSource,
    ) -> Transition<Self::State, Self::Event, Self::Outcome> {
        let rolled = u8::try_from(random.next_index(6).unwrap() + 1).unwrap();
        let next = State {
            turns: state.turns + 1,
            total: state.total + u16::from(rolled),
        };
        if next.turns == 2 {
            Transition::finish(next, vec![Rolled(rolled)], Outcome(next.total))
        } else {
            Transition::continue_with(next, vec![Rolled(rolled)])
        }
    }
}

fn reference() -> GameSessionRef {
    GameSessionRef::new(
        GameDefinitionRef::new("example.roll", "1.0.0").unwrap(),
        "roll-session",
    )
    .unwrap()
}

fn actions() -> Vec<GameAction<Roll>> {
    vec![
        GameAction::new(GameVersion::new(1), None, Roll),
        GameAction::new(GameVersion::new(2), None, Roll),
    ]
}

#[test]
fn seeded_randomness_is_versioned_and_stable() {
    assert_eq!(SEEDED_RANDOM_ALGORITHM, "splitmix64-v1");
    let mut random = SeededRandom::new(RandomSeed::new(0));
    assert_eq!(random.next_u64(), 0xe220_a839_7b1d_cdaf);
    assert_eq!(random.next_u64(), 0x6e78_9e6a_a1b9_65f4);
    assert_eq!(random.next_u64(), 0x06c4_5d18_8009_454f);
    assert!(random.next_index(0).is_err());
}

#[test]
fn replay_with_the_same_seed_reconstructs_identical_state_outcome_and_events() {
    let mut first_random = SeededRandom::new(RandomSeed::new(42));
    let first = replay(
        reference(),
        State { turns: 0, total: 0 },
        &DiceRules,
        actions(),
        &mut first_random,
    )
    .unwrap();
    let mut second_random = SeededRandom::new(RandomSeed::new(42));
    let second = replay(
        reference(),
        State { turns: 0, total: 0 },
        &DiceRules,
        actions(),
        &mut second_random,
    )
    .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.session().status(), GameStatus::Finished);
    assert_eq!(first.session().version(), GameVersion::new(3));
    assert_eq!(first.transitions().len(), 2);
}

#[test]
fn replay_can_continue_from_a_snapshot_without_reapplying_prior_actions() {
    let mut initial_random = SeededRandom::new(RandomSeed::new(19));
    let first = replay(
        reference(),
        State { turns: 0, total: 0 },
        &DiceRules,
        vec![GameAction::new(GameVersion::new(1), None, Roll)],
        &mut initial_random,
    )
    .unwrap();
    let snapshot = first.session().snapshot();
    let mut continuation = initial_random;
    let restored = replay_from_snapshot(
        snapshot,
        &DiceRules,
        vec![GameAction::new(GameVersion::new(2), None, Roll)],
        &mut continuation,
    )
    .unwrap();

    assert_eq!(restored.session().status(), GameStatus::Finished);
    assert_eq!(restored.session().version(), GameVersion::new(3));
}
