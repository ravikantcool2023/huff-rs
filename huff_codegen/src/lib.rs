#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![warn(unused_extern_crates)]
#![forbid(unsafe_code)]
#![forbid(where_clauses_object_safety)]

use huff_utils::{
    abi::*,
    artifact::*,
    ast::*,
    bytecode::*,
    error::CodegenError,
    evm::Opcode,
    prelude::{bytes32_to_string, pad_n_bytes, CodegenErrorKind, FileSource},
    types::EToken,
};
use std::{fs, path::Path, str::FromStr};

/// ### Codegen
///
/// Code Generation Manager responsible for generating the code for the Huff Language.
#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Codegen {
    /// The Input AST
    pub ast: Option<Contract>,
    /// A cached codegen output artifact
    pub artifact: Option<Artifact>,
    /// Intermediate main bytecode store
    pub main_bytecode: Option<String>,
    /// Intermediate constructor bytecode store
    pub constructor_bytecode: Option<String>,
}

impl Codegen {
    /// Public associated function to instantiate a new Codegen instance.
    pub fn new() -> Self {
        Self { ast: None, artifact: None, main_bytecode: None, constructor_bytecode: None }
    }

    /// Generates main bytecode from a Contract AST
    ///
    /// # Arguments
    ///
    /// * `ast` - Optional Contract Abstract Syntax Tree
    pub fn roll(ast: Option<Contract>) -> Result<String, CodegenError> {
        // Grab the AST
        let contract = match &ast {
            Some(a) => a,
            None => {
                tracing::error!(target: "codegen", "MISSING BOTH STATEFUL AND PARAMETER AST!");
                return Err(CodegenError {
                    kind: CodegenErrorKind::MissingAst,
                    span: None,
                    token: None,
                })
            }
        };

        // Find the main macro
        let m_macro: MacroDefinition = if let Some(m) = contract.find_macro_by_name("MAIN") {
            m
        } else {
            tracing::error!(target: "codegen", "MISSING \"MAIN\" MACRO!");
            return Err(CodegenError {
                kind: CodegenErrorKind::MissingMacroDefinition("MAIN".to_string()),
                span: None,
                token: None,
            })
        };

        tracing::info!(target: "codegen", "MAIN MACRO FOUND: {:?}", m_macro);

        // For each MacroInvocation Statement, recurse into bytecode
        let bytecode_res: BytecodeRes = Codegen::recurse_bytecode(
            m_macro.clone(),
            ast,
            &mut vec![m_macro],
            0,
            Vec::default(),
            &mut vec![],
        )?;
        tracing::info!(target: "codegen", "RECURSED BYTECODE: {:?}", bytecode_res);
        let bytecode = bytecode_res.bytes.iter().map(|byte| byte.0.to_string()).collect();
        tracing::info!(target: "codegen", "FINAL BYTECODE: {:?}", bytecode);

        // Return
        Ok(bytecode)
    }

    /// Gracefully get the Contract AST
    pub fn graceful_ast_grab(&self, ast: Option<Contract>) -> Result<Contract, CodegenError> {
        match ast {
            Some(a) => Ok(a),
            None => match &self.ast {
                Some(a) => Ok(a.clone()),
                None => {
                    tracing::error!("Neither Codegen AST was set nor passed in as a parameter to Codegen::construct()!");
                    Err(CodegenError {
                        kind: CodegenErrorKind::MissingAst,
                        span: None,
                        token: None,
                    })
                }
            },
        }
    }

    /// Generates constructor bytecode from a Contract AST
    ///
    /// # Arguments
    ///
    /// * `ast` - Optional Contract Abstract Syntax Tree
    pub fn construct(ast: Option<Contract>) -> Result<String, CodegenError> {
        // Grab the AST
        let contract = match &ast {
            Some(a) => a,
            None => {
                tracing::error!(target: "codegen", "Neither Codegen AST was set nor passed in as a parameter to Codegen::construct()!");
                return Err(CodegenError {
                    kind: CodegenErrorKind::MissingAst,
                    span: None,
                    token: None,
                })
            }
        };

        // Find the constructor macro
        let c_macro: MacroDefinition = if let Some(m) = contract.find_macro_by_name("CONSTRUCTOR") {
            m
        } else {
            tracing::error!(target: "codegen", "'CONSTRUCTOR' Macro definition missing in AST!");
            return Err(CodegenError {
                kind: CodegenErrorKind::MissingConstructor,
                span: None,
                token: None,
            })
        };

        tracing::info!(target: "codegen", "CONSTRUCTOR MACRO FOUND: {:?}", c_macro);

        // For each MacroInvocation Statement, recurse into bytecode
        let bytecode_res: BytecodeRes = Codegen::recurse_bytecode(
            c_macro.clone(),
            ast,
            &mut vec![c_macro],
            0,
            Vec::default(),
            &mut vec![],
        )?;
        tracing::info!(target: "codegen", "RECURSED BYTECODE: {:?}", bytecode_res);
        let bytecode = bytecode_res.bytes.iter().map(|byte| byte.0.to_string()).collect();
        tracing::info!(target: "codegen", "FINAL BYTECODE: {:?}", bytecode);

        // Return
        Ok(bytecode)
    }

    /// Recurses a MacroDefinition to generate Bytecode
    pub fn recurse_bytecode(
        macro_def: MacroDefinition,
        ast: Option<Contract>,
        scope: &mut Vec<MacroDefinition>,
        mut offset: usize,
        jump_tables: Vec<JumpTable>,
        mis: &mut Vec<(usize, MacroInvocation)>,
    ) -> Result<BytecodeRes, CodegenError> {
        let mut final_bytes: Vec<Bytes> = vec![];

        tracing::info!(target: "codegen", "RECURSING MACRO DEFINITION \"{}\" [SCOPE: {}]", macro_def.name, scope.len());

        // Grab the AST
        let contract = match &ast {
            Some(a) => a,
            None => {
                tracing::error!(target: "codegen", "Neither Codegen AST was set nor passed in as a parameter to Codegen::construct()!");
                return Err(CodegenError {
                    kind: CodegenErrorKind::MissingAst,
                    span: None,
                    token: None,
                })
            }
        };

        // Generate the macro bytecode
        let irb = macro_def.to_irbytecode()?;
        tracing::info!(target: "codegen", "GENERATED IRBYTECODE: {:?}", irb);
        let irbz: Vec<IRByte> = irb.0;

        let mut jump_table = JumpTable::new();
        let mut jump_indices = JumpIndices::new();

        for (index, ir_byte) in irbz.iter().enumerate() {
            match &ir_byte {
                IRByte::Bytes(b) => {
                    offset += b.0.len() / 2;
                    tracing::debug!(target: "codegen", "RECURSE_BYTECODE FOUND BYTES: {:?}", b);
                    final_bytes.push(b.clone())
                }
                IRByte::Constant(name) => {
                    let constant = if let Some(m) = contract
                        .constants
                        .iter()
                        .filter(|const_def| const_def.name.eq(name))
                        .cloned()
                        .collect::<Vec<ConstantDefinition>>()
                        .get(0)
                    {
                        m.clone()
                    } else {
                        tracing::error!(target: "codegen", "MISSING CONSTANT DEFINITION \"{}\"", name);

                        // TODO we should try and find the constant defined in other files here
                        return Err(CodegenError {
                            kind: CodegenErrorKind::MissingConstantDefinition,
                            span: None,
                            token: None,
                        })
                    };

                    tracing::info!(target: "codegen", "FOUND CONSTANT DEFINITION: {:?}", constant);

                    let push_bytes = match constant.value {
                        ConstVal::Literal(l) => {
                            let hex_literal: String = bytes32_to_string(&l, false);
                            format!("{:02x}{}", 95 + hex_literal.len() / 2, hex_literal)
                        }
                        ConstVal::FreeStoragePointer(fsp) => {
                            // If this is reached in codegen stage, the `derive_storage_pointers`
                            // method was not called on the AST.
                            tracing::error!(target: "codegen", "STORAGE POINTERS INCORRECTLY DERIVED FOR \"{:?}\"", fsp);
                            return Err(CodegenError {
                                kind: CodegenErrorKind::StoragePointersNotDerived,
                                span: None,
                                token: None,
                            })
                        }
                    };

                    offset += push_bytes.len() / 2;
                    tracing::info!(target: "codegen", "OFFSET: {}, PUSH BYTES: {:?}", offset, push_bytes);
                    final_bytes.push(Bytes(push_bytes))
                }
                IRByte::Statement(s) => {
                    tracing::debug!(target: "codegen", "Got Statement: {:?}", s);
                    match s {
                        Statement::MacroInvocation(mi) => {
                            // Get the macro that matches this invocation and turn into bytecode
                            let ir_macro =
                                if let Some(m) = contract.find_macro_by_name(&mi.macro_name) {
                                    m
                                } else {
                                    // TODO: this is where the file imports must be resolved .. in
                                    // case macro definition is external
                                    tracing::error!(
                                        target: "codegen",
                                        "MISSING MACRO INVOCATION \"{}\"",
                                        mi.macro_name
                                    );
                                    return Err(CodegenError {
                                        kind: CodegenErrorKind::MissingMacroDefinition(
                                            mi.macro_name.clone(),
                                        ),
                                        span: None,
                                        token: None,
                                    })
                                };

                            tracing::info!(target: "codegen", "FOUND INNER MACRO: {:?}", ir_macro);

                            // Recurse
                            scope.push(ir_macro.clone());
                            mis.push((index, mi.clone()));
                            let res: BytecodeRes = if let Ok(res) = Codegen::recurse_bytecode(
                                ir_macro.clone(),
                                ast.clone(),
                                scope,
                                offset,
                                jump_tables.clone(),
                                mis,
                            ) {
                                res
                            } else {
                                tracing::error!(
                                    target: "codegen",
                                    "FAILED TO RECURSE INTO MACRO \"{}\"",
                                    ir_macro.name
                                );
                                return Err(CodegenError {
                                    kind: CodegenErrorKind::FailedMacroRecursion,
                                    span: None,
                                    token: None,
                                })
                            };

                            // Set jump table values
                            jump_table.insert(index, res.unmatched_jumps);
                            jump_indices = jump_indices
                                .into_iter()
                                .chain(res.jump_indices)
                                .collect::<JumpIndices>();

                            // Increase offset by byte length of recursed macro
                            offset += res.bytes.iter().map(|b| b.0.len()).sum::<usize>() / 2;

                            final_bytes = final_bytes
                                .iter()
                                .cloned()
                                .chain(res.bytes.iter().cloned())
                                .collect();
                        }
                        Statement::Label(label) => {
                            tracing::info!(target: "codegen", "RECURSE BYTECODE GOT LABEL: {:?}", label);
                            jump_indices.insert(label.name.clone(), offset);
                            offset += 1;
                            final_bytes.push(Bytes(Opcode::Jumpdest.to_string()));
                        }
                        Statement::LabelCall(label) => {
                            tracing::info!(target: "codegen", "RECURSE BYTECODE GOT LABEL CALL: {}", label);
                            jump_table.insert(
                                index,
                                vec![Jump { label: label.to_owned(), bytecode_index: 0 }],
                            );
                            offset += 3;
                            final_bytes.push(Bytes(format!("{}xxxx", Opcode::Push2)));
                        }
                        s => {
                            tracing::error!(target: "codegen", "UNEXPECTED STATEMENT: {:?}", s);
                            return Err(CodegenError {
                                kind: CodegenErrorKind::InvalidMacroStatement,
                                span: None,
                                token: None,
                            })
                        }
                    }
                }
                IRByte::ArgCall(arg_name) => {
                    if let Err(e) = Codegen::bubble_arg_call(
                        arg_name,
                        &mut final_bytes,
                        &macro_def,
                        contract,
                        scope,
                        &mut offset,
                        &jump_tables,
                        mis,
                        &mut jump_table,
                    ) {
                        return Err(e)
                    }
                    tracing::error!(target: "codegen", "^^ BUBBLING FINISHED ^^ LEFT OVER MACRO INVOCATIONS: {:?}", mis);
                    // tracing::error!(target: "codegen", "^^ BUBBLING FINISHED ^^ LEFT OVER SCOPE:
                    // {:?}", scope);
                    tracing::error!(target: "codegen", "^^ BUBBLING FINISHED ^^ CURRENT MACRO DEF: {:?}", macro_def);
                }
            }
        }

        // We're done, let's pop off the macro invocation
        if mis.pop().is_none() {
            tracing::warn!(target: "codegen", "ATTEMPTED MACRO INVOCATION POP FAILED AT SCOPE: {}", scope.len());
        }

        let mut cur_index = offset;
        let mut indices = vec![cur_index]; // first index is the current offset
        indices.append(
            &mut final_bytes
                .iter()
                .map(|b| {
                    cur_index += b.0.len() / 2;
                    cur_index
                })
                .collect::<Vec<usize>>(),
        );

        let bytecode: String = final_bytes.iter().map(|byte| byte.0.to_string()).collect();
        tracing::info!(target: "codegen", "GENERATED BYECODE EXCLUDING JUMPS: {}", hex::encode(bytecode));

        let mut unmatched_jumps = Jumps::default();
        let final_bytes =
            final_bytes.iter().enumerate().fold(Vec::default(), |mut acc, (index, b)| {
                let mut formatted_bytes = b.clone();

                if let Some(jt) = jump_table.get(&index) {
                    for jump in jt {
                        if let Some(jump_index) = jump_indices.get(&jump.label) {
                            let jump_value = pad_n_bytes(&format!("{:x}", jump_index), 2);

                            let before = &formatted_bytes.0[0..jump.bytecode_index + 2];
                            let after = &formatted_bytes.0[jump.bytecode_index + 6..];

                            // Check if a jump dest placeholder is present
                            if &formatted_bytes.0[jump.bytecode_index + 2..jump.bytecode_index + 6] != "xxxx" {
                                tracing::error!(
                                    target: "codegen",
                                    "JUMP DESTINATION PLACEHOLDER NOT FOUND FOR JUMPLABEL {}",
                                    jump.label
                                );
                            }

                            formatted_bytes = Bytes(format!("{}{}{}", before, jump_value, after));
                        } else {
                            let jump_offset = (indices[index] - offset) * 2;

                            unmatched_jumps.push(Jump {
                                label: jump.label.clone(),
                                bytecode_index: jump_offset + jump.bytecode_index,
                            })
                        }
                    }
                }

                acc.push(formatted_bytes);
                acc
            });

        Ok(BytecodeRes { bytes: final_bytes, jump_tables, jump_indices, unmatched_jumps })
    }

    /// Arg Call Bubbling
    #[allow(clippy::too_many_arguments)]
    pub fn bubble_arg_call(
        arg_name: &str,
        bytegen: &mut Vec<Bytes>,
        macro_def: &MacroDefinition,
        contract: &Contract,
        scope: &mut Vec<MacroDefinition>,
        offset: &mut usize,
        jump_tables: &Vec<JumpTable>,
        // mis: Parent macro invocations and their indices
        mis: &mut Vec<(usize, MacroInvocation)>,
        jump_table: &mut JumpTable,
    ) -> Result<(), CodegenError> {
        // Args can be literals, labels, opcodes, or constants
        // !! IF THERE IS AMBIGUOUS NOMENCLATURE
        // !! (E.G. BOTH OPCODE AND LABEL ARE THE SAME STRING)
        // !! COMPILATION _WILL_ ERROR

        tracing::warn!(target: "codegen", "**BUBBLING** \"{}\"", macro_def.name);

        // Check Constant Definitions
        if let Some(constant) = contract
            .constants
            .iter()
            .filter(|const_def| const_def.name.eq(arg_name))
            .cloned()
            .collect::<Vec<ConstantDefinition>>()
            .get(0)
        {
            tracing::info!(target: "codegen", "ARGCALL IS CONSTANT: {:?}", constant);
            let push_bytes = match &constant.value {
                ConstVal::Literal(l) => {
                    let hex_literal: String = bytes32_to_string(l, false);
                    format!("{:02x}{}", 95 + hex_literal.len() / 2, hex_literal)
                }
                ConstVal::FreeStoragePointer(fsp) => {
                    // If this is reached in codegen stage, the
                    // `derive_storage_pointers`
                    // method was not called on the AST.
                    tracing::error!(target: "codegen", "STORAGE POINTERS INCORRECTLY DERIVED FOR \"{:?}\"", fsp);
                    return Err(CodegenError {
                        kind: CodegenErrorKind::StoragePointersNotDerived,
                        span: None,
                        token: None,
                    })
                }
            };
            *offset += push_bytes.len() / 2;
            tracing::info!(target: "codegen", "OFFSET: {}, PUSH BYTES: {:?}", offset, push_bytes);
            bytegen.push(Bytes(push_bytes));
        } else if let Ok(o) = Opcode::from_str(arg_name) {
            // Check Opcode Definition
            let b = Bytes(o.to_string());
            *offset += b.0.len() / 2;
            tracing::info!(target: "codegen", "RECURSE_BYTECODE ARG CALL FOUND OPCODE: {:?}", b);
            bytegen.push(b);
        } else if let Some(macro_invoc) = mis.last() {
            // Literal & Arg Call Check
            // First get this arg_nam position in the macro definition params
            if let Some(pos) = macro_def
                .parameters
                .iter()
                .position(|r| r.name.as_ref().map_or(false, |s| s.eq(arg_name)))
            {
                tracing::info!(target: "codegen", "GOT \"{}\" POS IN ARG LIST: {}", arg_name, pos);

                if let Some(arg) = macro_invoc.1.args.get(pos) {
                    tracing::info!(target: "codegen", "GOT \"{:?}\" ARG FROM MACRO INVOCATION", arg);
                    match arg {
                        MacroArg::Literal(l) => {
                            tracing::info!(target: "codegen", "GOT LITERAL {:?} ARG FROM MACRO INVOCATION", l);

                            let hex_literal: String = bytes32_to_string(l, false);
                            let push_bytes =
                                format!("{:02x}{}", 95 + hex_literal.len() / 2, hex_literal);
                            let b = Bytes(push_bytes);
                            *offset += b.0.len() / 2;
                            bytegen.push(b);
                        }
                        MacroArg::ArgCall(ac) => {
                            tracing::info!(target: "codegen", "GOT ARG CALL \"{}\" ARG FROM MACRO INVOCATION", ac);
                            tracing::debug!(target: "codegen", "~~~ BUBBLING UP ARG CALL");
                            let mut new_scope = Vec::from(&scope[..scope.len().saturating_sub(1)]);
                            let bubbled_macro_invocation = new_scope.last().unwrap().clone();
                            tracing::debug!(target: "codegen", "BUBBLING UP WITH MACRO DEF: {:?}", bubbled_macro_invocation);
                            tracing::debug!(target: "codegen", "CURRENT MACRO DEF: {:?}", macro_def);

                            // Only remove an invocation if not at bottom level, otherwise we'll
                            // remove one too many
                            if mis.last().unwrap().clone().1.macro_name.eq(&macro_def.name) {
                                return Codegen::bubble_arg_call(
                                    arg_name,
                                    bytegen,
                                    &bubbled_macro_invocation,
                                    contract,
                                    &mut new_scope,
                                    offset,
                                    &Vec::from(&jump_tables[..jump_tables.len().saturating_sub(1)]),
                                    &mut Vec::from(&mis[..mis.len().saturating_sub(1)]),
                                    jump_table,
                                )
                            } else {
                                return Codegen::bubble_arg_call(
                                    arg_name,
                                    bytegen,
                                    &bubbled_macro_invocation,
                                    contract,
                                    &mut new_scope,
                                    offset,
                                    &Vec::from(&jump_tables[..jump_tables.len().saturating_sub(1)]),
                                    mis,
                                    jump_table,
                                )
                            }
                        }
                        MacroArg::Ident(iden) => {
                            tracing::warn!(target: "codegen", "FOUND IDENT ARG IN \"{}\" MACRO INVOCATION: \"{}\"!", macro_invoc.1.macro_name, iden);
                            // TODO ----------------------
                        }
                    }
                } else {
                    tracing::warn!(target: "codegen", "\"{}\" FOUND IN MACRO DEF BUT NOT IN MACRO INVOCATION!", arg_name);
                }
            } else {
                tracing::warn!(target: "codegen", "\"{}\" NOT IN ARG LIST", arg_name);
            }
        } else {
            // Label can be defined in parent
            // Assume Label Call Otherwise
            tracing::info!(target: "codegen", "RECURSE_BYTECODE ARG CALL DEFAULTING TO LABEL CALL: \"{}\"", arg_name);
            jump_table.insert(
                mis.last().map(|mi| mi.0).unwrap_or_else(|| 0),
                vec![Jump { label: arg_name.to_owned(), bytecode_index: 0 }],
            );
            *offset += 3;
            bytegen.push(Bytes(format!("{}xxxx", Opcode::Push2)));
        }

        Ok(())
    }

    /// Generate a codegen artifact
    ///
    /// # Arguments
    ///
    /// * `args` - A vector of Tokens representing constructor arguments
    /// * `main_bytecode` - The compiled MAIN Macro bytecode
    /// * `constructor_bytecode` - The compiled `CONSTRUCTOR` Macro bytecode
    pub fn churn(
        &mut self,
        file: FileSource,
        args: Vec<ethers::abi::token::Token>,
        main_bytecode: &str,
        constructor_bytecode: &str,
    ) -> Result<Artifact, CodegenError> {
        let mut artifact: &mut Artifact = if let Some(art) = &mut self.artifact {
            art
        } else {
            self.artifact = Some(Artifact::default());
            self.artifact.as_mut().unwrap()
        };

        let contract_length = main_bytecode.len() / 2;
        let constructor_length = constructor_bytecode.len() / 2;

        let contract_size = format!("{:04x}", contract_length);
        let contract_code_offset = format!("{:04x}", 13 + constructor_length);

        let encoded: Vec<Vec<u8>> =
            args.iter().map(|tok| ethers::abi::encode(&[tok.clone()])).collect();
        let hex_args: Vec<String> = encoded.iter().map(|tok| hex::encode(tok.as_slice())).collect();
        let constructor_args = hex_args.join("");

        // Generate the final bytecode
        let bootstrap_code = format!("61{}8061{}6000396000f3", contract_size, contract_code_offset);
        let constructor_code = format!("{}{}", constructor_bytecode, bootstrap_code);
        artifact.bytecode =
            format!("{}{}{}", constructor_code, main_bytecode, constructor_args).to_lowercase();
        artifact.runtime = main_bytecode.to_string().to_lowercase();
        artifact.file = file;
        Ok(artifact.clone())
    }

    /// Encode constructor arguments as ethers::abi::token::Token
    pub fn encode_constructor_args(args: Vec<String>) -> Vec<ethers::abi::token::Token> {
        let tokens: Vec<ethers::abi::token::Token> =
            args.iter().map(|tok| EToken::try_from(tok.clone()).unwrap().0).collect();
        tokens
    }

    /// Export
    ///
    /// Writes a Codegen Artifact out to the specified file.
    ///
    /// # Arguments
    ///
    /// * `out` - Output location to write the serialized json artifact to.
    pub fn export(output: String, art: &Artifact) -> Result<(), CodegenError> {
        let serialized_artifact = serde_json::to_string(art).unwrap();
        // Try to create the parent directory
        let file_path = Path::new(&output);
        if let Some(p) = file_path.parent() {
            if let Err(e) = fs::create_dir_all(p) {
                return Err(CodegenError {
                    kind: CodegenErrorKind::IOError(e.to_string()),
                    span: None,
                    token: None,
                })
            }
        }
        if let Err(e) = fs::write(file_path, serialized_artifact) {
            return Err(CodegenError {
                kind: CodegenErrorKind::IOError(e.to_string()),
                span: None,
                token: None,
            })
        }
        Ok(())
    }

    /// Abi Generation
    ///
    /// Generates an ABI for the given Ast.
    /// Stores the generated ABI in the Codegen `artifact`.
    ///
    /// # Arguments
    ///
    /// * `ast` - The Contract Abstract Syntax Tree
    /// * `output` - An optional output path
    pub fn abi_gen(&mut self, ast: Contract, output: Option<String>) -> Result<Abi, CodegenError> {
        let abi: Abi = ast.into();

        // Set the abi on self
        let art: &Artifact = match &mut self.artifact {
            Some(artifact) => {
                artifact.abi = Some(abi.clone());
                artifact
            }
            None => {
                self.artifact = Some(Artifact { abi: Some(abi.clone()), ..Default::default() });
                self.artifact.as_ref().unwrap()
            }
        };

        // If an output's specified, write the artifact out
        if let Some(o) = output {
            if let Err(e) = Codegen::export(o, art) {
                // Error message is sent to tracing in `export` if an error occurs
                return Err(e)
            }
        }

        // Return the abi
        Ok(abi)
    }
}
