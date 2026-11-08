use crate::identity::GameSessionRef;
use crate::participant::ParticipantId;
use crate::randomness::RandomSource;
use crate::session::{GameStatus, GameVersion};

/// An authoritative command with an optimistic version precondition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameAction<A> {
    expected_version: GameVersion,
    actor: Option<ParticipantId>,
    payload: A,
}

impl<A> GameAction<A> {
    #[must_use]
    pub const fn new(
        expected_version: GameVersion,
        actor: Option<ParticipantId>,
        payload: A,
    ) -> Self {
        Self {
            expected_version,
            actor,
            payload,
        }
    }

    #[must_use]
    pub const fn expected_version(&self) -> GameVersion {
        self.expected_version
    }

    #[must_use]
    pub const fn actor(&self) -> Option<&ParticipantId> {
        self.actor.as_ref()
    }

    #[must_use]
    pub const fn payload(&self) -> &A {
        &self.payload
    }

    #[must_use]
    pub fn into_payload(self) -> A {
        self.payload
    }
}

/// Read-only facts available to rule validation and transition execution.
#[derive(Clone, Copy, Debug)]
pub struct ActionContext<'a> {
    pub(crate) session: &'a GameSessionRef,
    pub(crate) actor: Option<&'a ParticipantId>,
    pub(crate) version: GameVersion,
}

impl<'a> ActionContext<'a> {
    #[must_use]
    pub const fn session(&self) -> &'a GameSessionRef {
        self.session
    }

    #[must_use]
    pub const fn actor(&self) -> Option<&'a ParticipantId> {
        self.actor
    }

    #[must_use]
    pub const fn version(&self) -> GameVersion {
        self.version
    }
}

/// Accepted domain transition: new state, ordered events, and optional terminal outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition<S, E, O> {
    pub(crate) state: S,
    pub(crate) events: Vec<E>,
    pub(crate) outcome: Option<O>,
}

impl<S, E, O> Transition<S, E, O> {
    #[must_use]
    pub fn continue_with(state: S, events: Vec<E>) -> Self {
        Self {
            state,
            events,
            outcome: None,
        }
    }

    #[must_use]
    pub fn finish(state: S, events: Vec<E>, outcome: O) -> Self {
        Self {
            state,
            events,
            outcome: Some(outcome),
        }
    }
}

/// Rules supplied by a concrete game domain.
///
/// Validation cannot consume randomness. Once validation succeeds,
/// `transition` is infallible and must deterministically return the next state
/// for the supplied state, action, and random stream.
pub trait GameRules {
    type State;
    type Action;
    type Event;
    type Outcome;
    type Error;

    /// Validates actor authorization and game-specific action legality.
    ///
    /// # Errors
    ///
    /// Returns a domain error without changing state or consuming randomness.
    fn validate(
        &self,
        context: &ActionContext<'_>,
        state: &Self::State,
        action: &Self::Action,
    ) -> Result<(), Self::Error>;

    fn transition(
        &self,
        context: &ActionContext<'_>,
        state: &Self::State,
        action: &Self::Action,
        random: &mut dyn RandomSource,
    ) -> Transition<Self::State, Self::Event, Self::Outcome>;
}

/// Events and authoritative version produced by one accepted action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedTransition<E> {
    pub(crate) prior_version: GameVersion,
    pub(crate) next_version: GameVersion,
    pub(crate) status: GameStatus,
    pub(crate) events: Vec<E>,
}

impl<E> AppliedTransition<E> {
    #[must_use]
    pub const fn prior_version(&self) -> GameVersion {
        self.prior_version
    }

    #[must_use]
    pub const fn next_version(&self) -> GameVersion {
        self.next_version
    }

    #[must_use]
    pub const fn status(&self) -> GameStatus {
        self.status
    }

    #[must_use]
    pub fn events(&self) -> &[E] {
        &self.events
    }

    #[must_use]
    pub fn into_events(self) -> Vec<E> {
        self.events
    }
}
