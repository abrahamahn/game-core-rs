use std::collections::BTreeMap;
use std::fmt;

const MAX_PARTICIPANT_ID_LENGTH: usize = 160;

/// Stable identity for a participant in a game domain.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParticipantId(String);

impl ParticipantId {
    /// Creates a participant identity.
    ///
    /// # Errors
    ///
    /// Rejects empty, untrimmed, control-bearing, or oversized identifiers.
    pub fn new(value: impl Into<String>) -> Result<Self, ParticipantError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_PARTICIPANT_ID_LENGTH
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(ParticipantError::InvalidId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Generic participation state. Game-specific readiness and roles stay in the game rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipantStatus {
    Active,
    Left,
}

/// One participant and their session-level presence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Participant {
    id: ParticipantId,
    status: ParticipantStatus,
}

impl Participant {
    #[must_use]
    pub const fn new(id: ParticipantId) -> Self {
        Self {
            id,
            status: ParticipantStatus::Active,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &ParticipantId {
        &self.id
    }

    #[must_use]
    pub const fn status(&self) -> ParticipantStatus {
        self.status
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self.status, ParticipantStatus::Active)
    }
}

/// Minimal participant registry. Capacity, teams, seats, and roles are rule-owned policy.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParticipantRoster {
    participants: BTreeMap<ParticipantId, Participant>,
}

impl ParticipantRoster {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            participants: BTreeMap::new(),
        }
    }

    /// Adds one active participant.
    ///
    /// # Errors
    ///
    /// Returns `AlreadyExists` when the identity has ever been registered.
    pub fn join(&mut self, id: ParticipantId) -> Result<(), ParticipantError> {
        if self.participants.contains_key(&id) {
            return Err(ParticipantError::AlreadyExists);
        }
        self.participants.insert(id.clone(), Participant::new(id));
        Ok(())
    }

    /// Marks a participant as having left without erasing their identity.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` or `AlreadyLeft` when the transition is invalid.
    pub fn leave(&mut self, id: &ParticipantId) -> Result<(), ParticipantError> {
        let participant = self
            .participants
            .get_mut(id)
            .ok_or(ParticipantError::NotFound)?;
        if !participant.is_active() {
            return Err(ParticipantError::AlreadyLeft);
        }
        participant.status = ParticipantStatus::Left;
        Ok(())
    }

    #[must_use]
    pub fn get(&self, id: &ParticipantId) -> Option<&Participant> {
        self.participants.get(id)
    }

    #[must_use]
    pub fn is_active(&self, id: &ParticipantId) -> bool {
        self.get(id).is_some_and(Participant::is_active)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.participants.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Participant> {
        self.participants.values()
    }
}

/// Invalid participant identity or lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParticipantError {
    InvalidId,
    AlreadyExists,
    NotFound,
    AlreadyLeft,
}

impl ParticipantError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidId => "GAME_INVALID_PARTICIPANT_ID",
            Self::AlreadyExists => "GAME_PARTICIPANT_ALREADY_EXISTS",
            Self::NotFound => "GAME_PARTICIPANT_NOT_FOUND",
            Self::AlreadyLeft => "GAME_PARTICIPANT_ALREADY_LEFT",
        }
    }
}

impl fmt::Display for ParticipantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ParticipantError {}
