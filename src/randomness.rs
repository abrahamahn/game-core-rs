use std::fmt;

/// Pinned algorithm identifier for [`SeededRandom`].
pub const SEEDED_RANDOM_ALGORITHM: &str = "splitmix64-v1";

/// Application-supplied or persisted seed material.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RandomSeed(u64);

impl RandomSeed {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Opaque position in the pinned seeded-random stream.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RandomCheckpoint(u64);

impl RandomCheckpoint {
    /// Reconstructs a previously persisted checkpoint for the pinned seeded algorithm.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Small injected randomness boundary used by deterministic rules.
pub trait RandomSource {
    fn next_u64(&mut self) -> u64;

    /// Samples uniformly from `0..upper_exclusive` without modulo bias.
    ///
    /// # Errors
    ///
    /// Rejects an empty range or a range not representable by this source.
    fn next_index(&mut self, upper_exclusive: usize) -> Result<usize, RandomError> {
        if upper_exclusive == 0 {
            return Err(RandomError::EmptyRange);
        }
        let upper = u64::try_from(upper_exclusive).map_err(|_| RandomError::RangeTooLarge)?;
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return usize::try_from(value % upper).map_err(|_| RandomError::RangeTooLarge);
            }
        }
    }
}

/// Portable deterministic pseudo-random stream with a pinned algorithm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeededRandom {
    state: u64,
}

impl SeededRandom {
    #[must_use]
    pub const fn new(seed: RandomSeed) -> Self {
        Self { state: seed.get() }
    }

    #[must_use]
    pub const fn checkpoint(&self) -> RandomCheckpoint {
        RandomCheckpoint(self.state)
    }

    pub const fn restore(&mut self, checkpoint: RandomCheckpoint) {
        self.state = checkpoint.get();
    }
}

impl RandomSource for SeededRandom {
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }
}

/// Shuffles a slice using the injected source and Fisher-Yates ordering.
///
/// # Errors
///
/// Propagates a random range error.
pub fn shuffle<T>(values: &mut [T], random: &mut dyn RandomSource) -> Result<(), RandomError> {
    for upper in (2..=values.len()).rev() {
        let selected = random.next_index(upper)?;
        values.swap(upper - 1, selected);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RandomError {
    EmptyRange,
    RangeTooLarge,
}

impl fmt::Display for RandomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyRange => "GAME_RANDOM_EMPTY_RANGE",
            Self::RangeTooLarge => "GAME_RANDOM_RANGE_TOO_LARGE",
        })
    }
}

impl std::error::Error for RandomError {}
