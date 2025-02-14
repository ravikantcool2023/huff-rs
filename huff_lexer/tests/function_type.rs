use huff_lexer::*;
use huff_utils::prelude::*;
use std::ops::Deref;

#[test]
fn parses_function_type() {
    let fn_types = [
        ("view", TokenKind::View),
        ("pure", TokenKind::Pure),
        ("payable", TokenKind::Payable),
        ("nonpayable", TokenKind::NonPayable),
    ];

    for (fn_type, fn_type_kind) in fn_types {
        let source = &format!("#define function test() {fn_type} returns (uint256)");
        let flattened_source = FullFileSource { source, file: None, spans: vec![] };
        let mut lexer = Lexer::new(flattened_source.clone());
        assert_eq!(lexer.source, flattened_source);

        let _ = lexer.next(); // #define
        let _ = lexer.next(); // whitespace
        let _ = lexer.next(); // function
        let _ = lexer.next(); // whitespace
        let _ = lexer.next(); // fn name "test"
        let _ = lexer.next(); // open parenthesis
        let _ = lexer.next(); // close parenthesis
        let _ = lexer.next(); // whitespace

        // Lex view first
        let tok = lexer.next().unwrap().unwrap();
        let type_span = Span::new(24..24 + fn_type.len(), None);
        assert_eq!(tok, Token::new(fn_type_kind, type_span.clone()));
        assert_eq!(lexer.current_span().deref(), &type_span);

        let _ = lexer.next(); // whitespace
        let _ = lexer.next(); // returns
        let _ = lexer.next(); // whitespace
        let _ = lexer.next(); // open parenthesis
        let _ = lexer.next(); // uint256
        let _ = lexer.next(); // close parenthesis

        // We covered the whole source
        assert_eq!(lexer.current_span().end, source.len());
        assert!(lexer.eof);
    }
}
