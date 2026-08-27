use std::fmt;

use crate::{GameSessionRef, ParticipantId, RandomSource};

/// Monotonic optimistic-concurrency version of authoritative session state.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameVersion(u64);

impl GameVersion {
    pub const ZERO: Self = Self(0);
    /// Largest version represented exactly by both Rust and TypeScript implementations.
    pub const MAX_SAFE: Self = Self(9_007_199_254_740_991);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Creates a version that can be represented exactly by both implementations.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::VersionExhausted`] above [`Self::MAX_SAFE`].
    pub const fn try_new(value: u64) -> Result<Self, SessionError> {
        if value > Self::MAX_SAFE.0 {
            Err(SessionError::VersionExhausted)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Result<Self, SessionError> {
        if self >= Self::MAX_SAFE {
            return Err(SessionError::VersionExhausted);
        }
        Ok(Self(self.0 + 1))
    }
}

/// Core-owned lifecycle. Domain phases and rounds stay inside the game's state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameStatus {
    Created,
    Active,
    Finished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleOperation {
    Start,
    ApplyAction,
}

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
    session: &'a GameSessionRef,
    actor: Option<&'a ParticipantId>,
    version: GameVersion,
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
    state: S,
    events: Vec<E>,
    outcome: Option<O>,
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
    prior_version: GameVersion,
    next_version: GameVersion,
    status: GameStatus,
    events: Vec<E>,
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

/// Authoritative in-memory session aggregate, independent of storage and transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameSession<S, O> {
    reference: GameSessionRef,
    version: GameVersion,
    status: GameStatus,
    state: S,
    outcome: Option<O>,
}

impl<S, O> GameSession<S, O> {
    #[must_use]
    pub const fn create(reference: GameSessionRef, initial_state: S) -> Self {
        Self {
            reference,
            version: GameVersion::ZERO,
            status: GameStatus::Created,
            state: initial_state,
            outcome: None,
        }
    }

    /// Starts a created session using optimistic concurrency.
    ///
    /// # Errors
    ///
    /// Returns a version conflict, invalid lifecycle transition, or exhausted version.
    pub fn start(&mut self, expected_version: GameVersion) -> Result<GameVersion, SessionError> {
        self.ensure_version(expected_version)?;
        if self.status != GameStatus::Created {
            return Err(SessionError::InvalidLifecycleTransition {
                status: self.status,
                operation: LifecycleOperation::Start,
            });
        }
        let next_version = self.version.next()?;
        self.version = next_version;
        self.status = GameStatus::Active;
        Ok(next_version)
    }

    /// Validates and atomically applies one game-specific action.
    ///
    /// The core checks version and lifecycle before invoking domain validation.
    /// Rejected actions leave core state and injected randomness untouched.
    ///
    /// # Errors
    ///
    /// Returns a core session error or the concrete game's rejection error.
    pub fn apply<R>(
        &mut self,
        rules: &R,
        action: &GameAction<R::Action>,
        random: &mut dyn RandomSource,
    ) -> Result<AppliedTransition<R::Event>, GameExecutionError<R::Error>>
    where
        R: GameRules<State = S, Outcome = O>,
    {
        self.ensure_version(action.expected_version())?;
        if self.status != GameStatus::Active {
            return Err(SessionError::InvalidLifecycleTransition {
                status: self.status,
                operation: LifecycleOperation::ApplyAction,
            }
            .into());
        }
        let next_version = self.version.next()?;
        let context = ActionContext {
            session: &self.reference,
            actor: action.actor(),
            version: self.version,
        };
        rules
            .validate(&context, &self.state, action.payload())
            .map_err(GameExecutionError::ActionRejected)?;
        let Transition {
            state,
            events,
            outcome,
        } = rules.transition(&context, &self.state, action.payload(), random);
        let prior_version = self.version;
        self.state = state;
        self.outcome = outcome;
        self.version = next_version;
        self.status = if self.outcome.is_some() {
            GameStatus::Finished
        } else {
            GameStatus::Active
        };
        Ok(AppliedTransition {
            prior_version,
            next_version,
            status: self.status,
            events,
        })
    }

    #[must_use]
    pub const fn reference(&self) -> &GameSessionRef {
        &self.reference
    }

    #[must_use]
    pub const fn version(&self) -> GameVersion {
        self.version
    }

    #[must_use]
    pub const fn status(&self) -> GameStatus {
        self.status
    }

    #[must_use]
    pub const fn state(&self) -> &S {
        &self.state
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<&O> {
        self.outcome.as_ref()
    }

    #[must_use]
    pub fn into_state(self) -> S {
        self.state
    }

    /// Restores a validated snapshot.
    ///
    /// # Errors
    ///
    /// Rejects lifecycle/version/outcome combinations the core cannot produce.
    pub fn restore(snapshot: GameSnapshot<S, O>) -> Result<Self, SessionError> {
        snapshot.validate()?;
        Ok(Self {
            reference: snapshot.reference,
            version: snapshot.version,
            status: snapshot.status,
            state: snapshot.state,
            outcome: snapshot.outcome,
        })
    }

    fn ensure_version(&self, expected: GameVersion) -> Result<(), SessionError> {
        if expected != self.version {
            return Err(SessionError::VersionConflict {
                expected,
                actual: self.version,
            });
        }
        Ok(())
    }
}

impl<S: Clone, O: Clone> GameSession<S, O> {
    #[must_use]
    pub fn snapshot(&self) -> GameSnapshot<S, O> {
        GameSnapshot {
            reference: self.reference.clone(),
            version: self.version,
            status: self.status,
            state: self.state.clone(),
            outcome: self.outcome.clone(),
        }
    }
}

/// Storage-neutral snapshot. Serialization is deliberately adapter-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameSnapshot<S, O> {
    reference: GameSessionRef,
    version: GameVersion,
    status: GameStatus,
    state: S,
    outcome: Option<O>,
}

impl<S, O> GameSnapshot<S, O> {
    /// Creates a snapshot at a serialization boundary.
    ///
    /// # Errors
    ///
    /// Rejects lifecycle/version/outcome combinations the core cannot produce.
    pub fn new(
        reference: GameSessionRef,
        version: GameVersion,
        status: GameStatus,
        state: S,
        outcome: Option<O>,
    ) -> Result<Self, SessionError> {
        let snapshot = Self {
            reference,
            version,
            status,
            state,
            outcome,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    #[must_use]
    pub const fn reference(&self) -> &GameSessionRef {
        &self.reference
    }

    #[must_use]
    pub const fn version(&self) -> GameVersion {
        self.version
    }

    #[must_use]
    pub const fn status(&self) -> GameStatus {
        self.status
    }

    #[must_use]
    pub const fn state(&self) -> &S {
        &self.state
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<&O> {
        self.outcome.as_ref()
    }

    fn validate(&self) -> Result<(), SessionError> {
        let valid = self.version <= GameVersion::MAX_SAFE
            && match self.status {
                GameStatus::Created => self.version == GameVersion::ZERO && self.outcome.is_none(),
                GameStatus::Active => self.version > GameVersion::ZERO && self.outcome.is_none(),
                GameStatus::Finished => {
                    self.version > GameVersion::new(1) && self.outcome.is_some()
                }
            };
        if !valid {
            return Err(SessionError::InvalidSnapshot);
        }
        Ok(())
    }
}

/// Core lifecycle and concurrency failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionError {
    VersionConflict {
        expected: GameVersion,
        actual: GameVersion,
    },
    InvalidLifecycleTransition {
        status: GameStatus,
        operation: LifecycleOperation,
    },
    VersionExhausted,
    InvalidSnapshot,
}

impl SessionError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::VersionConflict { .. } => "GAME_VERSION_CONFLICT",
            Self::InvalidLifecycleTransition { .. } => "GAME_INVALID_LIFECYCLE_TRANSITION",
            Self::VersionExhausted => "GAME_VERSION_EXHAUSTED",
            Self::InvalidSnapshot => "GAME_INVALID_SNAPSHOT",
        }
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SessionError {}

/// Distinguishes core authority failures from concrete rule rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameExecutionError<E> {
    Session(SessionError),
    ActionRejected(E),
}

impl<E> From<SessionError> for GameExecutionError<E> {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl<E: fmt::Display> fmt::Display for GameExecutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(error) => error.fmt(formatter),
            Self::ActionRejected(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for GameExecutionError<E> {}
