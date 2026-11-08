use std::fmt;

use crate::identity::{GameSessionRef, MAX_SAFE_INTEGER};
use crate::randomness::RandomSource;
use crate::rules::{ActionContext, AppliedTransition, GameAction, GameRules, Transition};
use crate::snapshot::GameSnapshot;

/// Monotonic optimistic-concurrency version of authoritative session state.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameVersion(u64);

impl GameVersion {
    pub const ZERO: Self = Self(0);
    /// Largest version represented exactly by both Rust and TypeScript implementations.
    pub const MAX_SAFE: Self = Self(MAX_SAFE_INTEGER);

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
