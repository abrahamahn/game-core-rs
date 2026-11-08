use crate::identity::GameSessionRef;
use crate::session::{GameStatus, GameVersion, SessionError};

/// Storage-neutral snapshot. Serialization is deliberately adapter-owned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameSnapshot<S, O> {
    pub(crate) reference: GameSessionRef,
    pub(crate) version: GameVersion,
    pub(crate) status: GameStatus,
    pub(crate) state: S,
    pub(crate) outcome: Option<O>,
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

    pub(crate) fn validate(&self) -> Result<(), SessionError> {
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
