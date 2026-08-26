use game_core::{
    GameDefinitionKey, GameDefinitionRef, GameResultRef, GameSessionRef, IdentityError,
};

#[test]
fn canonical_identifiers_are_domain_neutral_and_versioned() {
    let definition = GameDefinitionRef::new("board.strategy.standard", "rules-2.1").unwrap();
    let session = GameSessionRef::new(definition, "room-7#match-42").unwrap();
    let result = GameResultRef::new(session, "result-42", 1).unwrap();

    assert_eq!(
        result.session().definition().key().as_str(),
        "board.strategy.standard"
    );
    assert_eq!(result.session().session_id().as_str(), "room-7#match-42");
    assert_eq!(result.result_id().as_str(), "result-42");
    assert_eq!(result.version(), 1);
}

#[test]
fn malformed_identifiers_fail_closed() {
    for invalid in [
        "",
        "Board.Strategy",
        "board..strategy",
        " board",
        "board/game",
    ] {
        assert_eq!(
            GameDefinitionKey::new(invalid),
            Err(IdentityError::InvalidDefinitionKey)
        );
    }
    let definition = GameDefinitionRef::new("board.strategy", "v1").unwrap();
    assert_eq!(
        GameSessionRef::new(definition, " session"),
        Err(IdentityError::InvalidSessionId)
    );
}
