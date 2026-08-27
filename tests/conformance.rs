use game_core::{RandomSeed, RandomSource, SEEDED_RANDOM_ALGORITHM, SeededRandom, shuffle};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RandomnessVector {
    seed: String,
    outputs: Vec<String>,
    index_upper_exclusive: usize,
    indexes: Vec<usize>,
    shuffle_input: Vec<u8>,
    shuffled: Vec<u8>,
}

#[derive(Deserialize)]
struct RandomnessFixture {
    profile: String,
    algorithm: String,
    vectors: Vec<RandomnessVector>,
}

#[test]
fn randomness_matches_the_cross_language_conformance_corpus() {
    let fixture: RandomnessFixture =
        serde_json::from_str(include_str!("../fixtures/randomness-v1.json")).unwrap();
    assert_eq!(fixture.profile, "game-core-randomness-v1");
    assert_eq!(fixture.algorithm, SEEDED_RANDOM_ALGORITHM);
    for vector in fixture.vectors {
        let seed = RandomSeed::new(vector.seed.parse().unwrap());
        let mut raw = SeededRandom::new(seed);
        let actual = vector
            .outputs
            .iter()
            .map(|_| raw.next_u64().to_string())
            .collect::<Vec<_>>();
        assert_eq!(actual, vector.outputs, "seed {} raw output", vector.seed);

        let mut bounded = SeededRandom::new(seed);
        let indexes = vector
            .indexes
            .iter()
            .map(|_| bounded.next_index(vector.index_upper_exclusive).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(indexes, vector.indexes, "seed {} indexes", vector.seed);

        let mut values = vector.shuffle_input;
        shuffle(&mut values, &mut SeededRandom::new(seed)).unwrap();
        assert_eq!(values, vector.shuffled, "seed {} shuffle", vector.seed);
    }
}
