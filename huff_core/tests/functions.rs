use huff_codegen::Codegen;
use huff_lexer::Lexer;
use huff_parser::Parser;
use huff_utils::prelude::{FileSource, FullFileSource, Token};
use std::sync::Arc;

#[test]
fn test_function() {
    let source: &str = r#"
        #define function test1(uint256,uint256,uint256) pure returns(uint256)
        #define function test2(uint256,uint256,uint256) pure returns(uint256)
        #define function test3(uint256,uint256,uint256) pure returns(uint256)

        #define fn MUL_DIV_DOWN() = takes (3) returns (1) {
            // Input stack:      [x, y, denominator]
            dup3              // [denominator, x, y, denominator]
            dup3              // [y, denominator, x, y, denominator]
            dup3              // [x, y, denominator, x, y, denominator]

            mul               // [x * y, denominator, x, y, denominator]
            0x00 mstore       // [denominator, x, y, denominator]

            iszero iszero     // [denominator != 0, x, y, denominator]

            swap1             // [x, denominator != 0, y, denominator]
            dup1              // [x, x, denominator != 0, y, denominator]
            iszero            // [x == 0, x, denominator != 0, y, denominator]

            swap1             // [x, x == 0, denominator != 0, y, denominator]
            0x00 mload        // [x * y, x, x == 0, denominator != 0, y, denominator]
            div               // [(x * y) / x, x == 0, denominator != 0, y, denominator]

            dup4              // [y, (x * y) / x, x == 0, denominator != 0, y, denominator]
            eq                // [y == (x * y) / x, x == 0, denominator != 0, y, denominator]
            or                // [y == (x * y) / x | x == 0, denominator != 0, y, denominator]
            and               // [(y == (x * y) / x | x == 0) & denominator != 0, y, denominator]

            iszero fail jumpi // Revert if (y == (x * y) / x | x == 0) & denominator != 0 is not satisfied

            pop               // [denominator]
            0x00 mload        // [x * y, denominator]
            div               // [(x * y) / denominator]

            0x01 finish jumpi

            fail:
                0x00 0x00 revert
            finish:
            // Return stack:     [(x * y) / denominator]
        }

        #define macro TEST_1() = takes(0) returns(0) {
            0x44 calldataload // [denominator]
            0x24 calldataload // [y, denominator]
            0x04 calldataload // [x, y, denominator]
            MUL_DIV_DOWN()
            0x00 mstore
            0x20 0x00 return
        }

        #define macro TEST_2() = takes(0) returns(0) {
            0x44 calldataload // [denominator]
            0x24 calldataload // [y, denominator]
            0x04 calldataload // [x, y, denominator]
            MUL_DIV_DOWN()
            0x00 mstore
            0x20 0x00 return
        }

        #define macro TEST_3() = takes(0) returns(0) {
            0x44 calldataload // [denominator]
            0x24 calldataload // [y, denominator]
            0x04 calldataload // [x, y, denominator]
            MUL_DIV_DOWN()
            0x00 mstore
            0x20 0x00 return
        }

        #define macro MAIN() = takes (0) returns (0) {
            0x00 calldataload 0xE0 shr
            dup1 __FUNC_SIG(test1) eq test_one jumpi
            dup1 __FUNC_SIG(test2) eq test_two jumpi
            dup1 __FUNC_SIG(test3) eq test_three jumpi

            test_one:
                TEST_1()
            test_two:
                TEST_2()
            test_three:
                TEST_3()
        }
    "#;

    // Parse tokens
    let flattened_source = FullFileSource { source, file: None, spans: vec![] };
    let lexer = Lexer::new(flattened_source);
    let tokens = lexer.into_iter().map(|x| x.unwrap()).collect::<Vec<Token>>();
    let mut parser = Parser::new(tokens, None);

    // Parse the AST
    let mut contract = parser.parse().unwrap();

    // Derive storage pointers
    contract.derive_storage_pointers();

    // Instantiate Codegen
    let cg = Codegen::new();

    // The codegen instance should have no artifact
    assert!(cg.artifact.is_none());

    // Have the Codegen create the runtime bytecode
    let rbytes = Codegen::generate_main_bytecode(&contract, None).unwrap();
    // Churn
    let mut cg = Codegen::new();
    let artifact =
        cg.churn(Arc::clone(&Arc::new(FileSource::default())), vec![], &rbytes, "", false).unwrap();
    assert_eq!(artifact.bytecode, String::from("60ad8060093d393df360003560e01c8063075900201461002757806319715c0d1461004457806327902d6914610061575b60443560243560043561003b92919061007e565b60005260206000f35b60443560243560043561005892919061007e565b60005260206000f35b60443560243560043561007592919061007e565b60005260206000f35b828282026000521515908015906000510483141716156100a457506000510460016100aa575b60006000fd5b9056"));
}

#[test]
fn test_nested_function() {
    let source: &str = r#"
        #define function test1(uint256,uint256,uint256) pure returns(uint256)
        #define function test2(uint256,uint256,uint256) pure returns(uint256)
        #define function test3(uint256,uint256,uint256) pure returns(uint256)

        #define fn MUL_DIV_DOWN() = takes (3) returns (1) {
            // Input stack:      [x, y, denominator]
            dup3              // [denominator, x, y, denominator]
            dup3              // [y, denominator, x, y, denominator]
            dup3              // [x, y, denominator, x, y, denominator]

            mul               // [x * y, denominator, x, y, denominator]
            0x00 mstore       // [denominator, x, y, denominator]

            iszero iszero     // [denominator != 0, x, y, denominator]

            swap1             // [x, denominator != 0, y, denominator]
            dup1              // [x, x, denominator != 0, y, denominator]
            iszero            // [x == 0, x, denominator != 0, y, denominator]

            swap1             // [x, x == 0, denominator != 0, y, denominator]
            0x00 mload        // [x * y, x, x == 0, denominator != 0, y, denominator]
            div               // [(x * y) / x, x == 0, denominator != 0, y, denominator]

            dup4              // [y, (x * y) / x, x == 0, denominator != 0, y, denominator]
            eq                // [y == (x * y) / x, x == 0, denominator != 0, y, denominator]
            or                // [y == (x * y) / x | x == 0, denominator != 0, y, denominator]
            and               // [(y == (x * y) / x | x == 0) & denominator != 0, y, denominator]

            iszero fail jumpi // Revert if (y == (x * y) / x | x == 0) & denominator != 0 is not satisfied

            pop               // [denominator]
            0x00 mload        // [x * y, denominator]
            div               // [(x * y) / denominator]

            0x01 finish jumpi

            fail:
                0x00 0x00 revert
            finish:
            // Return stack:     [(x * y) / denominator]
        }

        #define fn MUL_DIV_DOWN_2() = takes (3) returns (1) {
            MUL_DIV_DOWN()
        }

        #define macro TEST_1() = takes(0) returns(0) {
            0x44 calldataload // [denominator]
            0x24 calldataload // [y, denominator]
            0x04 calldataload // [x, y, denominator]
            MUL_DIV_DOWN_2()
            0x00 mstore
            0x20 0x00 return
        }

        #define macro MAIN() = takes (0) returns (0) {
            0x00 calldataload 0xE0 shr
            dup1 __FUNC_SIG(test1) eq test_one jumpi

            test_one:
                TEST_1()
        }
    "#;

    // Parse tokens
    let flattened_source = FullFileSource { source, file: None, spans: vec![] };
    let lexer = Lexer::new(flattened_source);
    let tokens = lexer.into_iter().map(|x| x.unwrap()).collect::<Vec<Token>>();
    let mut parser = Parser::new(tokens, None);

    // Parse the AST
    let mut contract = parser.parse().unwrap();

    // Derive storage pointers
    contract.derive_storage_pointers();

    // Instantiate Codegen
    let cg = Codegen::new();

    // The codegen instance should have no artifact
    assert!(cg.artifact.is_none());

    // Have the Codegen create the runtime bytecode
    let rbytes = Codegen::generate_main_bytecode(&contract, None).unwrap();
    // Churn
    let mut cg = Codegen::new();
    let artifact =
        cg.churn(Arc::clone(&Arc::new(FileSource::default())), vec![], &rbytes, "", false).unwrap();
    assert_eq!(artifact.bytecode, String::from("606b8060093d393df360003560e01c80630759002014610011575b60443560243560043561002592919061005d565b60005260206000f35b82828202600052151590801590600051048314171615610054575060005104600161005a575b60006000fd5b90565b61006892919061002e565b9056"));
}
