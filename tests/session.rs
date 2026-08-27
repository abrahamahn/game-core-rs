use game_core::{
    ActionContext, GameAction, GameDefinitionRef, GameExecutionError, GameRules, GameSession,
    GameSessionRef, GameSnapshot, GameStatus, GameVersion, ParticipantError, ParticipantId,
    ParticipantRoster, ParticipantStatus, RandomSeed, RandomSource, SeededRandom, SessionError,
    Transition,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct State {
    roster: ParticipantRoster,
    position: u8,
    target: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Advance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Advanced { by: u8, position: u8 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Outcome {
    winner: ParticipantId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuleError {
    MissingActor,
    InactiveActor,
    AlreadyAtTarget,
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for RuleError {}

struct RaceRules;

impl GameRules for RaceRules {
    type State = State;
    type Action = Action;
    type Event = Event;
    type Outcome = Outcome;
    type Error = RuleError;

    fn validate(
        &self,
        context: &ActionContext<'_>,
        state: &Self::State,
        _action: &Self::Action,
    ) -> Result<(), Self::Error> {
        let actor = context.actor().ok_or(RuleError::MissingActor)?;
        if !state.roster.is_active(actor) {
            return Err(RuleError::InactiveActor);
        }
        if state.position >= state.target {
            return Err(RuleError::AlreadyAtTarget);
        }
        Ok(())
    }

    fn transition(
        &self,
        context: &ActionContext<'_>,
        state: &Self::State,
        _action: &Self::Action,
        random: &mut dyn RandomSource,
    ) -> Transition<Self::State, Self::Event, Self::Outcome> {
        let by = u8::try_from(random.next_index(2).unwrap() + 1).unwrap();
        let position = state.position.saturating_add(by).min(state.target);
        let next = State {
            roster: state.roster.clone(),
            position,
            target: state.target,
        };
        let events = vec![Event::Advanced { by, position }];
        if position == state.target {
            Transition::finish(
                next,
                events,
                Outcome {
                    winner: context.actor().unwrap().clone(),
                },
            )
        } else {
            Transition::continue_with(next, events)
        }
    }
}

fn session_ref() -> GameSessionRef {
    GameSessionRef::new(
        GameDefinitionRef::new("example.race", "rules-v1").unwrap(),
        "race-42",
    )
    .unwrap()
}

fn initial_state() -> (State, ParticipantId) {
    let participant = ParticipantId::new("participant-a").unwrap();
    let mut roster = ParticipantRoster::new();
    roster.join(participant.clone()).unwrap();
    (
        State {
            roster,
            position: 0,
            target: 2,
        },
        participant,
    )
}

#[test]
fn participant_lifecycle_preserves_identity_and_rejects_impossible_transitions() {
    let participant = ParticipantId::new("participant-a").unwrap();
    let mut roster = ParticipantRoster::new();
    roster.join(participant.clone()).unwrap();

    assert!(roster.is_active(&participant));
    assert_eq!(roster.len(), 1);
    assert_eq!(
        roster.join(participant.clone()),
        Err(ParticipantError::AlreadyExists)
    );
    roster.leave(&participant).unwrap();
    assert_eq!(
        roster.get(&participant).unwrap().status(),
        ParticipantStatus::Left
    );
    assert_eq!(
        roster.leave(&participant),
        Err(ParticipantError::AlreadyLeft)
    );
}

#[test]
fn valid_action_advances_state_emits_events_and_finishes_once() {
    let (state, participant) = initial_state();
    let mut session = GameSession::create(session_ref(), state);
    assert_eq!(session.status(), GameStatus::Created);
    assert_eq!(
        session.start(GameVersion::ZERO).unwrap(),
        GameVersion::new(1)
    );

    let mut random = SeededRandom::new(RandomSeed::new(0));
    let action = GameAction::new(
        GameVersion::new(1),
        Some(participant.clone()),
        Action::Advance,
    );
    let applied = session.apply(&RaceRules, &action, &mut random).unwrap();

    assert_eq!(applied.prior_version(), GameVersion::new(1));
    assert_eq!(applied.next_version(), GameVersion::new(2));
    assert_eq!(applied.events().len(), 1);
    assert_eq!(session.state().position, 2);
    assert_eq!(session.status(), GameStatus::Finished);
    assert_eq!(session.outcome().unwrap().winner, participant);

    let again = GameAction::new(GameVersion::new(2), None, Action::Advance);
    assert_eq!(
        session.apply(&RaceRules, &again, &mut random),
        Err(GameExecutionError::Session(
            SessionError::InvalidLifecycleTransition {
                status: GameStatus::Finished,
                operation: game_core::LifecycleOperation::ApplyAction,
            }
        ))
    );
}

#[test]
fn invalid_action_and_version_conflict_do_not_mutate_state_or_randomness() {
    let (state, _) = initial_state();
    let mut session = GameSession::create(session_ref(), state);
    session.start(GameVersion::ZERO).unwrap();
    let before = session.snapshot();
    let mut random = SeededRandom::new(RandomSeed::new(8));
    let checkpoint = random.checkpoint();
    let mut pristine = random;

    let wrong_version = GameAction::new(GameVersion::ZERO, None, Action::Advance);
    assert!(matches!(
        session.apply(&RaceRules, &wrong_version, &mut random),
        Err(GameExecutionError::Session(
            SessionError::VersionConflict { .. }
        ))
    ));
    let missing_actor = GameAction::new(GameVersion::new(1), None, Action::Advance);
    assert_eq!(
        session.apply(&RaceRules, &missing_actor, &mut random),
        Err(GameExecutionError::ActionRejected(RuleError::MissingActor))
    );

    assert_eq!(session.snapshot(), before);
    assert_eq!(random.next_u64(), pristine.next_u64());
    random.restore(checkpoint);
    let mut restored = SeededRandom::new(RandomSeed::new(8));
    assert_eq!(random.next_u64(), restored.next_u64());
}

#[test]
fn snapshots_restore_authoritative_state_and_reject_impossible_combinations() {
    let (state, _) = initial_state();
    let mut session: GameSession<State, Outcome> =
        GameSession::create(session_ref(), state.clone());
    session.start(GameVersion::ZERO).unwrap();
    let restored = GameSession::restore(session.snapshot()).unwrap();

    assert_eq!(restored, session);
    assert_eq!(
        GameSnapshot::<State, Outcome>::new(
            session_ref(),
            GameVersion::new(7),
            GameStatus::Created,
            state,
            None,
        ),
        Err(SessionError::InvalidSnapshot)
    );
}

#[test]
fn a_created_session_cannot_start_twice_or_apply_before_starting() {
    let (state, participant) = initial_state();
    let mut session: GameSession<State, Outcome> = GameSession::create(session_ref(), state);
    let command = GameAction::new(GameVersion::ZERO, Some(participant), Action::Advance);
    let mut random = SeededRandom::new(RandomSeed::new(1));
    assert!(matches!(
        session.apply(&RaceRules, &command, &mut random),
        Err(GameExecutionError::Session(
            SessionError::InvalidLifecycleTransition { .. }
        ))
    ));
    session.start(GameVersion::ZERO).unwrap();
    assert!(matches!(
        session.start(GameVersion::new(1)),
        Err(SessionError::InvalidLifecycleTransition { .. })
    ));
}
