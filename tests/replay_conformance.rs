use game_core::{
    ActionContext, GameAction, GameDefinitionRef, GameExecutionError, GameRules, GameSession,
    GameSessionRef, GameSnapshot, GameStatus, GameVersion, ParticipantId, RandomCheckpoint,
    RandomSeed, RandomSource, SeededRandom, Transition, replay, replay_from_snapshot,
};
use serde::Deserialize;

#[derive(Clone, Debug, Eq, PartialEq)]
struct State {
    participant: String,
    target: u16,
    turns: u16,
    total: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Event {
    kind: String,
    value: u16,
    total: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Outcome {
    winner: String,
    total: u16,
}

struct Rules;

impl GameRules for Rules {
    type State = State;
    type Action = ();
    type Event = Event;
    type Outcome = Outcome;
    type Error = &'static str;

    fn validate(
        &self,
        context: &ActionContext<'_>,
        state: &Self::State,
        _action: &Self::Action,
    ) -> Result<(), Self::Error> {
        if context.actor().map(ParticipantId::as_str) != Some(state.participant.as_str()) {
            return Err("NOT_ACTIVE_PARTICIPANT");
        }
        Ok(())
    }

    fn transition(
        &self,
        _context: &ActionContext<'_>,
        state: &Self::State,
        _action: &Self::Action,
        random: &mut dyn RandomSource,
    ) -> Transition<Self::State, Self::Event, Self::Outcome> {
        let value = u16::try_from(random.next_index(6).unwrap() + 1).unwrap();
        let total = state.total.saturating_add(value).min(state.target);
        let next = State {
            participant: state.participant.clone(),
            target: state.target,
            turns: state.turns + 1,
            total,
        };
        let events = vec![Event {
            kind: "rolled".to_owned(),
            value,
            total,
        }];
        if total == state.target {
            Transition::finish(
                next,
                events,
                Outcome {
                    winner: state.participant.clone(),
                    total,
                },
            )
        } else {
            Transition::continue_with(next, events)
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DefinitionFixture {
    key: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActionFixture {
    expected_version: u64,
    actor: String,
    accepted: bool,
    error: Option<String>,
    version: u64,
    status: String,
    turns: u16,
    total: u16,
    events: Vec<Event>,
    outcome: Option<Outcome>,
    #[serde(default)]
    capture_snapshot: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StateFixture {
    version: u64,
    status: String,
    turns: u16,
    total: u16,
    random_checkpoint: String,
    outcome: Option<Outcome>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReplayFixture {
    profile: String,
    seed: String,
    definition: DefinitionFixture,
    session_id: String,
    participant: String,
    target: u16,
    actions: Vec<ActionFixture>,
    snapshot: StateFixture,
    r#final: StateFixture,
}

fn status(value: &str) -> GameStatus {
    match value {
        "created" => GameStatus::Created,
        "active" => GameStatus::Active,
        "finished" => GameStatus::Finished,
        _ => panic!("fixture contains an unsupported game status"),
    }
}

fn reference(fixture: &ReplayFixture) -> GameSessionRef {
    GameSessionRef::new(
        GameDefinitionRef::new(&fixture.definition.key, &fixture.definition.version).unwrap(),
        &fixture.session_id,
    )
    .unwrap()
}

fn initial_state(fixture: &ReplayFixture) -> State {
    State {
        participant: fixture.participant.clone(),
        target: fixture.target,
        turns: 0,
        total: 0,
    }
}

fn action(vector: &ActionFixture) -> GameAction<()> {
    GameAction::new(
        GameVersion::new(vector.expected_version),
        Some(ParticipantId::new(&vector.actor).unwrap()),
        (),
    )
}

fn assert_session(
    session: &GameSession<State, Outcome>,
    version: u64,
    expected_status: &str,
    turns: u16,
    total: u16,
    outcome: Option<&Outcome>,
) {
    assert_eq!(session.version().get(), version);
    assert_eq!(session.status(), status(expected_status));
    assert_eq!(session.state().turns, turns);
    assert_eq!(session.state().total, total);
    assert_eq!(session.outcome(), outcome);
}

#[test]
fn lifecycle_replay_matches_the_cross_language_corpus() {
    let fixture: ReplayFixture =
        serde_json::from_str(include_str!("../fixtures/replay-v1.json")).unwrap();
    assert_eq!(fixture.profile, "game-core-replay-v1");
    let seed = RandomSeed::new(fixture.seed.parse().unwrap());
    let mut random = SeededRandom::new(seed);
    let mut session = GameSession::create(reference(&fixture), initial_state(&fixture));
    session.start(GameVersion::ZERO).unwrap();
    let mut captured: Option<(GameSnapshot<State, Outcome>, RandomCheckpoint)> = None;

    for vector in &fixture.actions {
        let checkpoint = random.checkpoint();
        let result = session.apply(&Rules, &action(vector), &mut random);
        if vector.accepted {
            let applied = result.expect("fixture action should be accepted");
            assert_eq!(applied.events(), vector.events);
        } else {
            let error = result.expect_err("fixture action should be rejected");
            assert_eq!(error.to_string(), vector.error.as_deref().unwrap());
            assert_eq!(random.checkpoint(), checkpoint);
        }
        assert_session(
            &session,
            vector.version,
            &vector.status,
            vector.turns,
            vector.total,
            vector.outcome.as_ref(),
        );
        if vector.capture_snapshot {
            captured = Some((session.snapshot(), random.checkpoint()));
        }
    }

    let (snapshot, checkpoint) = captured.expect("fixture captures an active snapshot");
    assert_eq!(snapshot.version().get(), fixture.snapshot.version);
    assert_eq!(snapshot.status(), status(&fixture.snapshot.status));
    assert_eq!(snapshot.state().turns, fixture.snapshot.turns);
    assert_eq!(snapshot.state().total, fixture.snapshot.total);
    assert_eq!(
        checkpoint.get().to_string(),
        fixture.snapshot.random_checkpoint
    );
    assert_session(
        &session,
        fixture.r#final.version,
        &fixture.r#final.status,
        fixture.r#final.turns,
        fixture.r#final.total,
        fixture.r#final.outcome.as_ref(),
    );
    assert_eq!(
        random.checkpoint().get().to_string(),
        fixture.r#final.random_checkpoint
    );

    let accepted = fixture
        .actions
        .iter()
        .filter(|vector| vector.accepted)
        .map(action)
        .collect::<Vec<_>>();
    let mut replay_random = SeededRandom::new(seed);
    let replayed = replay(
        reference(&fixture),
        initial_state(&fixture),
        &Rules,
        accepted,
        &mut replay_random,
    )
    .unwrap();
    assert_eq!(replayed.session(), &session);

    let continuation = action(&fixture.actions[5]);
    let mut continuation_random = SeededRandom::new(seed);
    continuation_random.restore(checkpoint);
    let continued =
        replay_from_snapshot(snapshot, &Rules, [continuation], &mut continuation_random).unwrap();
    assert_eq!(continued.session(), &session);
    assert_eq!(continuation_random.checkpoint(), random.checkpoint());
}

#[test]
fn malformed_recorded_history_stops_without_consuming_randomness() {
    let fixture: ReplayFixture =
        serde_json::from_str(include_str!("../fixtures/replay-v1.json")).unwrap();
    let seed = RandomSeed::new(fixture.seed.parse().unwrap());
    let mut random = SeededRandom::new(seed);
    let checkpoint = random.checkpoint();
    let stale = action(&fixture.actions[3]);
    let result = replay(
        reference(&fixture),
        initial_state(&fixture),
        &Rules,
        [stale],
        &mut random,
    );
    assert!(matches!(result, Err(GameExecutionError::Session(_))));
    assert_eq!(random.checkpoint(), checkpoint);
}
