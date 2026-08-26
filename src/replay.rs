use crate::{
    AppliedTransition, GameAction, GameExecutionError, GameRules, GameSession, GameSessionRef,
    GameSnapshot, GameVersion, RandomSource,
};

/// Reconstructed authoritative state and the transitions generated while replaying.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplayResult<S, O, E> {
    session: GameSession<S, O>,
    transitions: Vec<AppliedTransition<E>>,
}

/// Result type shared by initial-state and snapshot replay operations.
pub type ReplayExecutionResult<R> = Result<
    ReplayResult<<R as GameRules>::State, <R as GameRules>::Outcome, <R as GameRules>::Event>,
    GameExecutionError<<R as GameRules>::Error>,
>;

impl<S, O, E> ReplayResult<S, O, E> {
    #[must_use]
    pub const fn session(&self) -> &GameSession<S, O> {
        &self.session
    }

    #[must_use]
    pub fn transitions(&self) -> &[AppliedTransition<E>] {
        &self.transitions
    }

    #[must_use]
    pub fn into_parts(self) -> (GameSession<S, O>, Vec<AppliedTransition<E>>) {
        (self.session, self.transitions)
    }
}

/// Reconstructs a session from its initial state and ordered accepted actions.
///
/// The core starts the session at version zero, then enforces every recorded
/// action's version and lifecycle precondition.
///
/// # Errors
///
/// Returns the first lifecycle, version, or domain-rule failure.
pub fn replay<R, I>(
    reference: GameSessionRef,
    initial_state: R::State,
    rules: &R,
    actions: I,
    random: &mut dyn RandomSource,
) -> ReplayExecutionResult<R>
where
    R: GameRules,
    I: IntoIterator<Item = GameAction<R::Action>>,
{
    let mut session = GameSession::create(reference, initial_state);
    session.start(GameVersion::ZERO)?;
    apply_recorded_actions(session, rules, actions, random)
}

/// Continues replay from a validated snapshot.
///
/// # Errors
///
/// Returns an invalid-snapshot error or the first action failure.
pub fn replay_from_snapshot<R, I>(
    snapshot: GameSnapshot<R::State, R::Outcome>,
    rules: &R,
    actions: I,
    random: &mut dyn RandomSource,
) -> ReplayExecutionResult<R>
where
    R: GameRules,
    I: IntoIterator<Item = GameAction<R::Action>>,
{
    let session = GameSession::restore(snapshot)?;
    apply_recorded_actions(session, rules, actions, random)
}

fn apply_recorded_actions<R, I>(
    mut session: GameSession<R::State, R::Outcome>,
    rules: &R,
    actions: I,
    random: &mut dyn RandomSource,
) -> ReplayExecutionResult<R>
where
    R: GameRules,
    I: IntoIterator<Item = GameAction<R::Action>>,
{
    let mut transitions = Vec::new();
    for action in actions {
        transitions.push(session.apply(rules, &action, random)?);
    }
    Ok(ReplayResult {
        session,
        transitions,
    })
}
