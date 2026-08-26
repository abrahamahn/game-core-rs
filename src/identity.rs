use std::fmt;

const MAX_REFERENCE_LENGTH: usize = 160;

/// Validation failures for stable game-domain identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityError {
    InvalidDefinitionKey,
    InvalidDefinitionVersion,
    InvalidSessionId,
    InvalidResultId,
    InvalidResultVersion,
}

impl IdentityError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidDefinitionKey => "GAME_INVALID_DEFINITION_KEY",
            Self::InvalidDefinitionVersion => "GAME_INVALID_DEFINITION_VERSION",
            Self::InvalidSessionId => "GAME_INVALID_SESSION_ID",
            Self::InvalidResultId => "GAME_INVALID_RESULT_ID",
            Self::InvalidResultVersion => "GAME_INVALID_RESULT_VERSION",
        }
    }
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for IdentityError {}

pub type IdentityResult<T> = Result<T, IdentityError>;

/// Stable, dot-separated identity for a family of rules.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameDefinitionKey(String);

impl GameDefinitionKey {
    /// Creates a canonical definition key such as `board.chess.standard`.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, uppercase, whitespace, and punctuation-heavy
    /// keys.
    pub fn new(value: impl Into<String>) -> IdentityResult<Self> {
        let value = value.into();
        if !is_definition_key(&value) {
            return Err(IdentityError::InvalidDefinitionKey);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Immutable version label for a game definition.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameDefinitionVersion(String);

impl GameDefinitionVersion {
    /// Creates a canonical version label.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, non-ASCII, or non-canonical labels.
    pub fn new(value: impl Into<String>) -> IdentityResult<Self> {
        let value = value.into();
        if !is_version(&value) {
            return Err(IdentityError::InvalidDefinitionVersion);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reference to one immutable interpretation of a game's rules.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameDefinitionRef {
    key: GameDefinitionKey,
    version: GameDefinitionVersion,
}

impl GameDefinitionRef {
    /// Creates a versioned definition reference.
    ///
    /// # Errors
    ///
    /// Returns the corresponding key or version validation error.
    pub fn new(key: impl Into<String>, version: impl Into<String>) -> IdentityResult<Self> {
        Ok(Self {
            key: GameDefinitionKey::new(key)?,
            version: GameDefinitionVersion::new(version)?,
        })
    }

    #[must_use]
    pub const fn key(&self) -> &GameDefinitionKey {
        &self.key
    }

    #[must_use]
    pub const fn version(&self) -> &GameDefinitionVersion {
        &self.version
    }
}

/// Stable identity for one runtime game session.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameSessionId(String);

impl GameSessionId {
    /// Creates a session identity.
    ///
    /// # Errors
    ///
    /// Rejects empty, untrimmed, control-bearing, or oversized identifiers.
    pub fn new(value: impl Into<String>) -> IdentityResult<Self> {
        let value = value.into();
        if !is_reference_id(&value) {
            return Err(IdentityError::InvalidSessionId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Typed reference to one runtime game session and its immutable rules.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameSessionRef {
    definition: GameDefinitionRef,
    session_id: GameSessionId,
}

impl GameSessionRef {
    /// Creates a typed session reference.
    ///
    /// # Errors
    ///
    /// Returns `InvalidSessionId` for a malformed identity.
    pub fn new(
        definition: GameDefinitionRef,
        session_id: impl Into<String>,
    ) -> IdentityResult<Self> {
        Ok(Self {
            definition,
            session_id: GameSessionId::new(session_id)?,
        })
    }

    #[must_use]
    pub const fn definition(&self) -> &GameDefinitionRef {
        &self.definition
    }

    #[must_use]
    pub const fn session_id(&self) -> &GameSessionId {
        &self.session_id
    }
}

/// Stable identity for an accepted game result.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameResultId(String);

impl GameResultId {
    /// Creates a result identity.
    ///
    /// # Errors
    ///
    /// Rejects empty, untrimmed, control-bearing, or oversized identifiers.
    pub fn new(value: impl Into<String>) -> IdentityResult<Self> {
        let value = value.into();
        if !is_reference_id(&value) {
            return Err(IdentityError::InvalidResultId);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Versioned reference to an immutable result for one session.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GameResultRef {
    session: GameSessionRef,
    result_id: GameResultId,
    version: u32,
}

impl GameResultRef {
    /// Creates a versioned result reference.
    ///
    /// # Errors
    ///
    /// Rejects malformed result identities and version zero.
    pub fn new(
        session: GameSessionRef,
        result_id: impl Into<String>,
        version: u32,
    ) -> IdentityResult<Self> {
        if version == 0 {
            return Err(IdentityError::InvalidResultVersion);
        }
        Ok(Self {
            session,
            result_id: GameResultId::new(result_id)?,
            version,
        })
    }

    #[must_use]
    pub const fn session(&self) -> &GameSessionRef {
        &self.session
    }

    #[must_use]
    pub const fn result_id(&self) -> &GameResultId {
        &self.result_id
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }
}

fn is_definition_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_LENGTH
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_lowercase)
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn is_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_LENGTH
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn is_reference_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REFERENCE_LENGTH
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
