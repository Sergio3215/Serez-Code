/// MIR → LLVM IR emission.
///
/// Activated with the `llvm` cargo feature:
///   cargo build --features llvm
///
/// Requires LLVM 17 installed and the `inkwell` crate.
/// Without the feature flag the module compiles as a no-op stub so the rest
/// of the compiler pipeline can be developed and tested independently.
///
/// Type mapping:
///   SzType::Int      → i64
///   SzType::Decimal  → double
///   SzType::Bool     → i1
///   SzType::Str      → { i64, i8* }   (len + heap ptr)
///   SzType::Array(T) → { i64, T* }    (len + heap ptr)
///   SzType::Class(X) → %X = type { fields… }
///   SzType::Void     → void

// ─────────────────────────────────────────────────────────────────────────────
// Feature-gated implementation (requires `inkwell` + LLVM 17)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "llvm")]
pub mod emit {
    use inkwell::{
        builder::Builder,
        context::Context,
        module::Module,
        types::{BasicMetadataTypeEnum, BasicTypeEnum},
        values::{BasicValueEnum, FunctionValue},
        AddressSpace, OptimizationLevel,
    };
    use std::collections::HashMap;

    use crate::compiler::{
        hir::{HirBinOp, HirUnaryOp},
        mir::*,
        types::SzType,
    };

    pub struct LlvmEmitter<'ctx> {
        pub context: &'ctx Context,
        pub module:  Module<'ctx>,
        builder:     Builder<'ctx>,
        /// temp index → LLVM value for the current function
        temps: HashMap<Temp, BasicValueEnum<'ctx>>,
        /// named variable → alloca pointer for the current function
        vars:  HashMap<String, inkwell::values::PointerValue<'ctx>>,
    }

    impl<'ctx> LlvmEmitter<'ctx> {
        pub fn new(context: &'ctx Context, module_name: &str) -> Self {
            LlvmEmitter {
                module:  context.create_module(module_name),
                builder: context.create_builder(),
                context,
                temps: HashMap::new(),
                vars:  HashMap::new(),
            }
        }

        // ── Type conversion ───────────────────────────────────────────────────

        fn llvm_type(&self, ty: &SzType) -> BasicTypeEnum<'ctx> {
            match ty {
                SzType::Int     => self.context.i64_type().into(),
                SzType::Decimal => self.context.f64_type().into(),
                SzType::Bool    => self.context.bool_type().into(),
                SzType::Str | SzType::Array(_) => {
                    // { i64, i8* } — length + heap pointer
                    let i64_t = self.context.i64_type();
                    let ptr_t = self.context.i8_type().ptr_type(AddressSpace::default());
                    self.context.struct_type(&[i64_t.into(), ptr_t.into()], false).into()
                }
                SzType::Class(name) => {
                    // Opaque named struct — fields filled in during class lowering
                    self.context.opaque_struct_type(name).into()
                }
                SzType::Null | SzType::Void | SzType::Unknown => {
                    self.context.i64_type().into() // placeholder
                }
                SzType::Dict(_, _) | SzType::Function { .. } | SzType::Enum(_) => {
                    self.context.i64_type().into() // phase 2
                }
            }
        }

        // ── Program emission ──────────────────────────────────────────────────

        pub fn emit_program(&mut self, program: &MirProgram) {
            for func in &program.functions {
                self.emit_function(func);
            }
        }

        // ── Function emission ─────────────────────────────────────────────────

        fn emit_function(&mut self, func: &MirFunction) {
            self.temps.clear();
            self.vars.clear();

            // Build LLVM function signature
            let param_types: Vec<BasicMetadataTypeEnum<'ctx>> = func.params.iter()
                .map(|(_, ty)| self.llvm_type(ty).into())
                .collect();

            let fn_type = if matches!(func.ret_type, SzType::Void) {
                self.context.void_type().fn_type(&param_types, false)
            } else {
                let ret_ty = self.llvm_type(&func.ret_type);
                ret_ty.fn_type(&param_types, false)
            };

            let fn_val: FunctionValue<'ctx> = self.module.add_function(&func.name, fn_type, None);

            // Create LLVM basic blocks (one per MIR block)
            let llvm_blocks: HashMap<String, inkwell::basic_block::BasicBlock<'ctx>> = func.blocks.iter()
                .map(|bb| {
                    let lbl = fn_val.append_basic_block(&bb.label);
                    (bb.label.clone(), lbl)
                })
                .collect();

            // Emit instructions for each block
            for bb in &func.blocks {
                let llvm_bb = llvm_blocks[&bb.label];
                self.builder.position_at_end(llvm_bb);

                for instr in &bb.instrs {
                    self.emit_instr(instr, fn_val);
                }

                self.emit_terminator(&bb.term, &llvm_blocks);
            }
        }

        // ── Instruction emission ──────────────────────────────────────────────

        fn emit_instr(&mut self, instr: &MirInstr, _fn_val: FunctionValue<'ctx>) {
            match instr {
                MirInstr::Copy(t, val) => {
                    let v = self.resolve_val(val);
                    self.temps.insert(*t, v);
                }

                MirInstr::Store(name, val) => {
                    let v = self.resolve_val(val);
                    if let Some(&ptr) = self.vars.get(name) {
                        self.builder.build_store(ptr, v).unwrap();
                    } else {
                        // First store → create alloca
                        let alloca = self.builder.build_alloca(v.get_type(), name).unwrap();
                        self.builder.build_store(alloca, v).unwrap();
                        self.vars.insert(name.clone(), alloca);
                    }
                }

                MirInstr::Load(t, name) => {
                    if let Some(&ptr) = self.vars.get(name) {
                        let v = self.builder.build_load(ptr.get_type(), ptr, &format!("t{}", t)).unwrap();
                        self.temps.insert(*t, v);
                    }
                }

                MirInstr::BinOp(t, op, lhs, rhs) => {
                    let lv = self.resolve_val(lhs);
                    let rv = self.resolve_val(rhs);
                    let result = self.emit_binop(op, lv, rv, *t);
                    self.temps.insert(*t, result);
                }

                MirInstr::UnaryOp(t, op, val) => {
                    let v = self.resolve_val(val);
                    let result = match op {
                        HirUnaryOp::Neg => {
                            if v.is_int_value() {
                                self.builder.build_int_neg(v.into_int_value(), &format!("t{}", t))
                                    .unwrap().into()
                            } else {
                                self.builder.build_float_neg(v.into_float_value(), &format!("t{}", t))
                                    .unwrap().into()
                            }
                        }
                        HirUnaryOp::Not => {
                            self.builder.build_not(v.into_int_value(), &format!("t{}", t))
                                .unwrap().into()
                        }
                    };
                    self.temps.insert(*t, result);
                }

                MirInstr::Call(result_t, name, args) => {
                    let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args.iter()
                        .map(|a| self.resolve_val(a).into())
                        .collect();
                    if let Some(fn_val) = self.module.get_function(name) {
                        let call = self.builder.build_call(fn_val, &arg_vals, "call").unwrap();
                        if let Some(t) = result_t {
                            if let Some(v) = call.try_as_basic_value().left() {
                                self.temps.insert(*t, v);
                            }
                        }
                    }
                }

                MirInstr::Out(val) => {
                    // Emit call to runtime __sz_out(val)
                    let v = self.resolve_val(val);
                    if let Some(fn_val) = self.module.get_function("__sz_out") {
                        self.builder.build_call(fn_val, &[v.into()], "").unwrap();
                    }
                }

                // Phase 2: method calls, index, field, new
                MirInstr::MethodCall(_, _, _, _)
                | MirInstr::IndexLoad(_, _, _)
                | MirInstr::IndexStore(_, _, _)
                | MirInstr::FieldLoad(_, _, _)
                | MirInstr::FieldStore(_, _, _)
                | MirInstr::New(_, _, _) => {
                    // TODO: phase 2 — requires struct layout and vtable support
                }
            }
        }

        fn emit_terminator(
            &mut self,
            term: &Terminator,
            blocks: &HashMap<String, inkwell::basic_block::BasicBlock<'ctx>>,
        ) {
            match term {
                Terminator::Jump(lbl) => {
                    self.builder.build_unconditional_branch(blocks[lbl]).unwrap();
                }
                Terminator::Branch(cond, t_lbl, f_lbl) => {
                    let cond_val = self.resolve_val(cond).into_int_value();
                    self.builder.build_conditional_branch(cond_val, blocks[t_lbl], blocks[f_lbl]).unwrap();
                }
                Terminator::Return(None) => {
                    self.builder.build_return(None).unwrap();
                }
                Terminator::Return(Some(val)) => {
                    let v = self.resolve_val(val);
                    self.builder.build_return(Some(&v)).unwrap();
                }
            }
        }

        // ── Binary operations ─────────────────────────────────────────────────

        fn emit_binop(&mut self, op: &HirBinOp, lv: BasicValueEnum<'ctx>, rv: BasicValueEnum<'ctx>, t: Temp) -> BasicValueEnum<'ctx> {
            let name = format!("t{}", t);
            let is_float = lv.is_float_value();

            match op {
                HirBinOp::Add => if is_float {
                    self.builder.build_float_add(lv.into_float_value(), rv.into_float_value(), &name).unwrap().into()
                } else {
                    self.builder.build_int_add(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into()
                },
                HirBinOp::Sub => if is_float {
                    self.builder.build_float_sub(lv.into_float_value(), rv.into_float_value(), &name).unwrap().into()
                } else {
                    self.builder.build_int_sub(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into()
                },
                HirBinOp::Mul => if is_float {
                    self.builder.build_float_mul(lv.into_float_value(), rv.into_float_value(), &name).unwrap().into()
                } else {
                    self.builder.build_int_mul(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into()
                },
                HirBinOp::Div => if is_float {
                    self.builder.build_float_div(lv.into_float_value(), rv.into_float_value(), &name).unwrap().into()
                } else {
                    self.builder.build_int_signed_div(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into()
                },
                HirBinOp::Mod => {
                    self.builder.build_int_signed_rem(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into()
                },
                HirBinOp::Eq => if is_float {
                    self.builder.build_float_compare(inkwell::FloatPredicate::OEQ, lv.into_float_value(), rv.into_float_value(), &name).unwrap().into()
                } else {
                    self.builder.build_int_compare(inkwell::IntPredicate::EQ, lv.into_int_value(), rv.into_int_value(), &name).unwrap().into()
                },
                HirBinOp::Ne => if is_float {
                    self.builder.build_float_compare(inkwell::FloatPredicate::ONE, lv.into_float_value(), rv.into_float_value(), &name).unwrap().into()
                } else {
                    self.builder.build_int_compare(inkwell::IntPredicate::NE, lv.into_int_value(), rv.into_int_value(), &name).unwrap().into()
                },
                HirBinOp::Lt => self.builder.build_int_compare(inkwell::IntPredicate::SLT, lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::Le => self.builder.build_int_compare(inkwell::IntPredicate::SLE, lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::Gt => self.builder.build_int_compare(inkwell::IntPredicate::SGT, lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::Ge => self.builder.build_int_compare(inkwell::IntPredicate::SGE, lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::And => self.builder.build_and(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::Or  => self.builder.build_or(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::BitAnd => self.builder.build_and(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::BitOr  => self.builder.build_or(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::BitXor => self.builder.build_xor(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::Shl   => self.builder.build_left_shift(lv.into_int_value(), rv.into_int_value(), &name).unwrap().into(),
                HirBinOp::Shr   => self.builder.build_right_shift(lv.into_int_value(), rv.into_int_value(), false, &name).unwrap().into(),
                HirBinOp::Pow   => lv, // phase 2: call __sz_pow(lv, rv)
            }
        }

        // ── Value resolution ──────────────────────────────────────────────────

        fn resolve_val(&self, val: &MirVal) -> BasicValueEnum<'ctx> {
            match val {
                MirVal::Temp(t)         => self.temps[t],
                MirVal::ConstInt(i)     => self.context.i64_type().const_int(*i as u64, true).into(),
                MirVal::ConstDecimal(d) => self.context.f64_type().const_float(*d).into(),
                MirVal::ConstBool(b)    => self.context.bool_type().const_int(*b as u64, false).into(),
                MirVal::ConstStr(_)     => self.context.i64_type().const_zero().into(), // phase 2
                MirVal::Null            => self.context.i64_type().const_zero().into(),
            }
        }

        // ── Output ────────────────────────────────────────────────────────────

        /// Write the LLVM IR to a `.ll` file.
        pub fn write_ir(&self, path: &str) -> Result<(), String> {
            self.module.print_to_file(path).map_err(|e| e.to_string())
        }

        /// Compile to a native object file using LLVM's JIT.
        pub fn compile_to_object(&self, path: &str) -> Result<(), String> {
            use inkwell::targets::{
                CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
            };
            Target::initialize_native(&InitializationConfig::default())
                .map_err(|e| e.to_string())?;

            let triple  = TargetMachine::get_default_triple();
            let target  = Target::from_triple(&triple).map_err(|e| e.to_string())?;
            let machine = target.create_target_machine(
                &triple,
                "generic", "",
                OptimizationLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            ).ok_or("Could not create target machine")?;

            machine.write_to_file(&self.module, FileType::Object, path.as_ref())
                .map_err(|e| e.to_string())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Stub — used when the `llvm` feature is not active
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "llvm"))]
pub mod emit {
    use crate::compiler::mir::MirProgram;

    pub struct LlvmEmitter;

    impl LlvmEmitter {
        pub fn new() -> Self { LlvmEmitter }

        pub fn emit_program(&self, _program: &MirProgram) {
            eprintln!("LLVM emission is not enabled. Rebuild with: cargo build --features llvm");
        }
    }
}
