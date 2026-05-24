/// HIR → MIR lowering (three-address code generation).
///
/// Flattens every nested HIR expression into a sequence of single-operation
/// instructions, each assigning its result to a fresh temporary.
/// Control flow constructs (if, while, for) become basic blocks connected
/// by jumps and conditional branches.
use crate::compiler::hir::*;
use crate::compiler::mir::*;
use crate::compiler::types::SzType;

// ── Public entry point ────────────────────────────────────────────────────────

pub struct MirLowerer {
    temp_counter:  usize,
    block_counter: usize,
}

impl MirLowerer {
    pub fn new() -> Self {
        MirLowerer { temp_counter: 0, block_counter: 0 }
    }

    pub fn lower_program(&mut self, program: &HirProgram) -> MirProgram {
        let functions = program.functions.iter()
            .map(|f| self.lower_function(f))
            .collect();
        MirProgram { functions }
    }

    fn lower_function(&mut self, func: &HirFunction) -> MirFunction {
        let params: Vec<(String, SzType)> = func.params.iter()
            .map(|p| (p.name.clone(), p.ty.clone()))
            .collect();

        let mut fl = FnLowerer {
            mir: self,
            blocks: Vec::new(),
            cur_label: "entry".to_string(),
            cur_instrs: Vec::new(),
            loop_stack: Vec::new(),
        };

        for stmt in &func.body {
            fl.lower_stmt(stmt);
        }

        // Always seal the last open block. Terminal blocks like while_exit,
        // for_exit, and merge may be empty — seal them unconditionally so they
        // appear in the block list. Unreachable blocks (after an explicit return
        // or break) are structurally valid; LLVM discards them during codegen.
        fl.seal(Terminator::Return(None));

        MirFunction {
            name: func.name.clone(),
            params,
            ret_type: func.ret_type.clone(),
            blocks: fl.blocks,
        }
    }

    fn fresh_temp(&mut self) -> Temp {
        let t = self.temp_counter;
        self.temp_counter += 1;
        t
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        let n = self.block_counter;
        self.block_counter += 1;
        format!("{}_{}", prefix, n)
    }
}

// ── Per-function lowerer ──────────────────────────────────────────────────────

struct FnLowerer<'a> {
    mir: &'a mut MirLowerer,
    blocks: Vec<BasicBlock>,
    cur_label: String,
    cur_instrs: Vec<MirInstr>,
    /// Stack of (break_label, continue_label) for nested loops
    loop_stack: Vec<(String, String)>,
}

impl<'a> FnLowerer<'a> {
    // ── Block management ─────────────────────────────────────────────────────

    /// Close the current block with `term` and start a new open block.
    fn seal(&mut self, term: Terminator) {
        let instrs = std::mem::take(&mut self.cur_instrs);
        let label  = self.cur_label.clone();
        self.blocks.push(BasicBlock { label, instrs, term });
    }

    fn start(&mut self, label: String) {
        self.cur_label = label;
    }

    fn fresh_temp(&mut self) -> Temp  { self.mir.fresh_temp() }
    fn fresh_label(&mut self, p: &str) -> String { self.mir.fresh_label(p) }

    // ── Statement lowering ───────────────────────────────────────────────────

    fn lower_stmt(&mut self, stmt: &HirStmt) {
        match stmt {
            HirStmt::Let { name, value, .. } => {
                let val = self.lower_expr(value);
                self.cur_instrs.push(MirInstr::Store(name.clone(), val));
            }

            HirStmt::Assign(lval, value) => {
                let val = self.lower_expr(value);
                match lval {
                    HirLValue::Var(name) => {
                        self.cur_instrs.push(MirInstr::Store(name.clone(), val));
                    }
                    HirLValue::Index { array, index } => {
                        let arr = self.lower_expr(array);
                        let idx = self.lower_expr(index);
                        self.cur_instrs.push(MirInstr::IndexStore(arr, idx, val));
                    }
                    HirLValue::Field { object, field } => {
                        let obj = self.lower_expr(object);
                        self.cur_instrs.push(MirInstr::FieldStore(obj, field.clone(), val));
                    }
                }
            }

            HirStmt::If { cond, then_body, else_body } => {
                let cond_val   = self.lower_expr(cond);
                let then_lbl   = self.fresh_label("then");
                let merge_lbl  = self.fresh_label("merge");
                let else_lbl   = if else_body.is_empty() {
                    merge_lbl.clone()
                } else {
                    self.fresh_label("else")
                };

                self.seal(Terminator::Branch(cond_val, then_lbl.clone(), else_lbl.clone()));

                // then block
                self.start(then_lbl);
                for s in then_body { self.lower_stmt(s); }
                self.seal(Terminator::Jump(merge_lbl.clone()));

                // else block (only if non-empty)
                if !else_body.is_empty() {
                    self.start(else_lbl);
                    for s in else_body { self.lower_stmt(s); }
                    self.seal(Terminator::Jump(merge_lbl.clone()));
                }

                self.start(merge_lbl);
            }

            HirStmt::While { cond, body } => {
                let cond_lbl = self.fresh_label("while_cond");
                let body_lbl = self.fresh_label("while_body");
                let exit_lbl = self.fresh_label("while_exit");

                self.seal(Terminator::Jump(cond_lbl.clone()));

                // condition block
                self.start(cond_lbl.clone());
                let cond_val = self.lower_expr(cond);
                self.seal(Terminator::Branch(cond_val, body_lbl.clone(), exit_lbl.clone()));

                // body block
                self.start(body_lbl);
                self.loop_stack.push((exit_lbl.clone(), cond_lbl.clone()));
                for s in body { self.lower_stmt(s); }
                self.loop_stack.pop();
                self.seal(Terminator::Jump(cond_lbl));

                self.start(exit_lbl);
            }

            HirStmt::For { init, cond, update, body } => {
                self.lower_stmt(init);

                let cond_lbl   = self.fresh_label("for_cond");
                let body_lbl   = self.fresh_label("for_body");
                let update_lbl = self.fresh_label("for_update");
                let exit_lbl   = self.fresh_label("for_exit");

                self.seal(Terminator::Jump(cond_lbl.clone()));

                // condition
                self.start(cond_lbl.clone());
                let cond_val = self.lower_expr(cond);
                self.seal(Terminator::Branch(cond_val, body_lbl.clone(), exit_lbl.clone()));

                // body
                self.start(body_lbl);
                self.loop_stack.push((exit_lbl.clone(), update_lbl.clone()));
                for s in body { self.lower_stmt(s); }
                self.loop_stack.pop();
                self.seal(Terminator::Jump(update_lbl.clone()));

                // update
                self.start(update_lbl);
                self.lower_stmt(update);
                self.seal(Terminator::Jump(cond_lbl));

                self.start(exit_lbl);
            }

            HirStmt::Return(val) => {
                let mir_val = val.as_ref().map(|v| self.lower_expr(v));
                // Dead code after return needs a fresh block for structural validity
                let dead_lbl = self.fresh_label("dead");
                self.seal(Terminator::Return(mir_val));
                self.start(dead_lbl);
            }

            HirStmt::Break => {
                if let Some((exit_lbl, _)) = self.loop_stack.last().cloned() {
                    let dead_lbl = self.fresh_label("dead");
                    self.seal(Terminator::Jump(exit_lbl));
                    self.start(dead_lbl);
                }
            }

            HirStmt::Continue => {
                if let Some((_, cont_lbl)) = self.loop_stack.last().cloned() {
                    let dead_lbl = self.fresh_label("dead");
                    self.seal(Terminator::Jump(cont_lbl));
                    self.start(dead_lbl);
                }
            }

            HirStmt::Out(expr) => {
                let val = self.lower_expr(expr);
                self.cur_instrs.push(MirInstr::Out(val));
            }

            HirStmt::Block(stmts) => {
                for s in stmts { self.lower_stmt(s); }
            }

            HirStmt::ExprStmt(expr) => {
                self.lower_expr(expr); // result discarded
            }
        }
    }

    // ── Expression lowering ──────────────────────────────────────────────────
    //
    // Returns the MirVal that holds the result of `expr`.
    // Constant literals are returned as MirVal::Const* directly (no instruction needed).
    // Everything else allocates a fresh temporary and emits an instruction.

    fn lower_expr(&mut self, expr: &HirExpr) -> MirVal {
        match expr {
            HirExpr::LitInt(i)      => MirVal::ConstInt(*i),
            HirExpr::LitDecimal(d)  => MirVal::ConstDecimal(*d),
            HirExpr::LitBool(b)     => MirVal::ConstBool(*b),
            HirExpr::LitStr(s)      => MirVal::ConstStr(s.clone()),
            HirExpr::Null           => MirVal::Null,

            HirExpr::Var(name, _) => {
                let t = self.fresh_temp();
                self.cur_instrs.push(MirInstr::Load(t, name.clone()));
                MirVal::Temp(t)
            }

            HirExpr::BinOp { op, left, right, .. } => {
                let lv = self.lower_expr(left);
                let rv = self.lower_expr(right);
                let t  = self.fresh_temp();
                self.cur_instrs.push(MirInstr::BinOp(t, op.clone(), lv, rv));
                MirVal::Temp(t)
            }

            HirExpr::UnaryOp { op, operand, .. } => {
                let v = self.lower_expr(operand);
                let t = self.fresh_temp();
                self.cur_instrs.push(MirInstr::UnaryOp(t, op.clone(), v));
                MirVal::Temp(t)
            }

            HirExpr::Call { name, args, .. } => {
                let arg_vals: Vec<MirVal> = args.iter().map(|a| self.lower_expr(a)).collect();
                let t = self.fresh_temp();
                self.cur_instrs.push(MirInstr::Call(Some(t), name.clone(), arg_vals));
                MirVal::Temp(t)
            }

            HirExpr::MethodCall { object, method, args, .. } => {
                let obj_val  = self.lower_expr(object);
                let arg_vals: Vec<MirVal> = args.iter().map(|a| self.lower_expr(a)).collect();
                let t = self.fresh_temp();
                self.cur_instrs.push(MirInstr::MethodCall(Some(t), obj_val, method.clone(), arg_vals));
                MirVal::Temp(t)
            }

            HirExpr::Index { array, index, .. } => {
                let arr = self.lower_expr(array);
                let idx = self.lower_expr(index);
                let t   = self.fresh_temp();
                self.cur_instrs.push(MirInstr::IndexLoad(t, arr, idx));
                MirVal::Temp(t)
            }

            HirExpr::Field { object, name, .. } => {
                let obj = self.lower_expr(object);
                let t   = self.fresh_temp();
                self.cur_instrs.push(MirInstr::FieldLoad(t, obj, name.clone()));
                MirVal::Temp(t)
            }

            HirExpr::New { class, args } => {
                let arg_vals: Vec<MirVal> = args.iter().map(|a| self.lower_expr(a)).collect();
                let t = self.fresh_temp();
                self.cur_instrs.push(MirInstr::New(t, class.clone(), arg_vals));
                MirVal::Temp(t)
            }

            HirExpr::Array { elements, .. } => {
                let vals: Vec<MirVal> = elements.iter().map(|e| self.lower_expr(e)).collect();
                let t = self.fresh_temp();
                self.cur_instrs.push(MirInstr::Call(Some(t), "__sz_array_new".to_string(), vals));
                MirVal::Temp(t)
            }

            // Conditional expression: emit then/else blocks, merge with a result temp
            HirExpr::If { cond, then_expr, else_expr, .. } => {
                let cond_val  = self.lower_expr(cond);
                let then_lbl  = self.fresh_label("if_then");
                let else_lbl  = self.fresh_label("if_else");
                let merge_lbl = self.fresh_label("if_merge");
                let result_t  = self.fresh_temp();
                let result_var = format!("__cond_{}", result_t);

                self.seal(Terminator::Branch(cond_val, then_lbl.clone(), else_lbl.clone()));

                // then
                self.start(then_lbl);
                let then_val = self.lower_expr(then_expr);
                self.cur_instrs.push(MirInstr::Store(result_var.clone(), then_val));
                self.seal(Terminator::Jump(merge_lbl.clone()));

                // else
                self.start(else_lbl);
                let else_val = self.lower_expr(else_expr);
                self.cur_instrs.push(MirInstr::Store(result_var.clone(), else_val));
                self.seal(Terminator::Jump(merge_lbl.clone()));

                // merge — load the result
                self.start(merge_lbl);
                self.cur_instrs.push(MirInstr::Load(result_t, result_var));
                MirVal::Temp(result_t)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::hir::*;
    use crate::compiler::mir::*;
    use crate::compiler::types::SzType;

    // ── Builders ─────────────────────────────────────────────────────────────

    fn lower(body: Vec<HirStmt>) -> MirFunction {
        let prog = HirProgram { functions: vec![HirFunction {
            name: "test".into(), params: vec![], ret_type: SzType::Void, body,
        }]};
        MirLowerer::new().lower_program(&prog).functions.remove(0)
    }

    fn int_var(name: &str) -> HirExpr { HirExpr::Var(name.to_string(), SzType::Int) }

    fn add(a: HirExpr, b: HirExpr) -> HirExpr {
        HirExpr::BinOp { op: HirBinOp::Add, left: Box::new(a), right: Box::new(b), ty: SzType::Int }
    }

    fn lt(a: HirExpr, b: HirExpr) -> HirExpr {
        HirExpr::BinOp { op: HirBinOp::Lt, left: Box::new(a), right: Box::new(b), ty: SzType::Bool }
    }

    // ── Basic block structure ─────────────────────────────────────────────────

    #[test]
    fn empty_function_has_entry_block_with_void_return() {
        let f = lower(vec![]);
        assert!(!f.blocks.is_empty());
        assert_eq!(f.blocks[0].label, "entry");
        assert!(matches!(f.blocks[0].term, Terminator::Return(None)));
    }

    #[test]
    fn every_block_has_exactly_one_terminator() {
        let f = lower(vec![
            HirStmt::If {
                cond: HirExpr::LitBool(true),
                then_body: vec![HirStmt::Out(HirExpr::LitInt(1))],
                else_body: vec![HirStmt::Out(HirExpr::LitInt(2))],
            },
        ]);
        for block in &f.blocks {
            // verifying the block has a terminator (just by type-checking the enum)
            let _ = &block.term;
        }
        assert!(f.blocks.len() >= 4); // entry, then, else, merge
    }

    // ── Let / Store ───────────────────────────────────────────────────────────

    #[test]
    fn let_const_emits_store_instruction() {
        let f = lower(vec![
            HirStmt::Let { name: "x".into(), ty: SzType::Int, value: HirExpr::LitInt(42), is_const: false },
        ]);
        let entry = &f.blocks[0];
        assert!(entry.instrs.iter().any(|i| {
            matches!(i, MirInstr::Store(n, MirVal::ConstInt(42)) if n == "x")
        }));
    }

    #[test]
    fn let_bool_emits_store_const_bool() {
        let f = lower(vec![
            HirStmt::Let { name: "flag".into(), ty: SzType::Bool, value: HirExpr::LitBool(true), is_const: false },
        ]);
        assert!(f.blocks[0].instrs.iter().any(|i| {
            matches!(i, MirInstr::Store(n, MirVal::ConstBool(true)) if n == "flag")
        }));
    }

    // ── BinOp ─────────────────────────────────────────────────────────────────

    #[test]
    fn arithmetic_binop_emits_binop_and_store() {
        let expr = add(HirExpr::LitInt(3), HirExpr::LitInt(4));
        let f = lower(vec![
            HirStmt::Let { name: "r".into(), ty: SzType::Int, value: expr, is_const: false },
        ]);
        let instrs = &f.blocks[0].instrs;
        // BinOp(t, Add, ConstInt(3), ConstInt(4))
        assert!(instrs.iter().any(|i| matches!(i, MirInstr::BinOp(_, HirBinOp::Add, MirVal::ConstInt(3), MirVal::ConstInt(4)))));
        // Store("r", Temp(t))
        assert!(instrs.iter().any(|i| matches!(i, MirInstr::Store(n, MirVal::Temp(_)) if n == "r")));
    }

    #[test]
    fn load_var_emits_load_instruction() {
        let f = lower(vec![
            HirStmt::Let { name: "x".into(), ty: SzType::Int, value: HirExpr::LitInt(5), is_const: false },
            HirStmt::Let { name: "y".into(), ty: SzType::Int, value: int_var("x"), is_const: false },
        ]);
        let instrs = &f.blocks[0].instrs;
        assert!(instrs.iter().any(|i| matches!(i, MirInstr::Load(_, n) if n == "x")));
    }

    // ── Return ────────────────────────────────────────────────────────────────

    #[test]
    fn return_value_emits_return_terminator() {
        let f = lower(vec![HirStmt::Return(Some(HirExpr::LitInt(7)))]);
        assert!(f.blocks.iter().any(|b| {
            matches!(&b.term, Terminator::Return(Some(MirVal::ConstInt(7))))
        }));
    }

    #[test]
    fn return_none_emits_void_return() {
        let prog = HirProgram { functions: vec![HirFunction {
            name: "void_fn".into(), params: vec![], ret_type: SzType::Void,
            body: vec![HirStmt::Return(None)],
        }]};
        let f = MirLowerer::new().lower_program(&prog).functions.remove(0);
        assert!(f.blocks.iter().any(|b| matches!(&b.term, Terminator::Return(None))));
    }

    // ── If / Else ─────────────────────────────────────────────────────────────

    #[test]
    fn if_else_creates_then_else_merge_blocks() {
        let f = lower(vec![HirStmt::If {
            cond: HirExpr::LitBool(true),
            then_body: vec![HirStmt::Out(HirExpr::LitInt(1))],
            else_body: vec![HirStmt::Out(HirExpr::LitInt(2))],
        }]);
        assert!(f.blocks.iter().any(|b| b.label.starts_with("then")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("else")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("merge")));
    }

    #[test]
    fn if_without_else_has_no_else_block() {
        let f = lower(vec![HirStmt::If {
            cond: HirExpr::LitBool(true),
            then_body: vec![HirStmt::Out(HirExpr::LitInt(0))],
            else_body: vec![],
        }]);
        assert!(!f.blocks.iter().any(|b| b.label.starts_with("else")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("merge")));
    }

    #[test]
    fn if_entry_block_ends_with_branch() {
        let f = lower(vec![HirStmt::If {
            cond: HirExpr::LitBool(true),
            then_body: vec![],
            else_body: vec![],
        }]);
        assert!(matches!(f.blocks[0].term, Terminator::Branch(_, _, _)));
    }

    // ── While ─────────────────────────────────────────────────────────────────

    #[test]
    fn while_loop_produces_cond_body_exit_blocks() {
        let f = lower(vec![HirStmt::While {
            cond: HirExpr::LitBool(true),
            body: vec![HirStmt::Out(HirExpr::LitInt(0))],
        }]);
        assert!(f.blocks.iter().any(|b| b.label.starts_with("while_cond")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("while_body")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("while_exit")));
    }

    #[test]
    fn while_cond_block_ends_with_branch() {
        let f = lower(vec![HirStmt::While {
            cond: HirExpr::LitBool(true),
            body: vec![],
        }]);
        let cond_block = f.blocks.iter().find(|b| b.label.starts_with("while_cond")).unwrap();
        assert!(matches!(cond_block.term, Terminator::Branch(_, _, _)));
    }

    // ── Break / Continue ──────────────────────────────────────────────────────

    #[test]
    fn break_terminates_body_block_with_jump_to_exit() {
        let f = lower(vec![HirStmt::While {
            cond: HirExpr::LitBool(true),
            body: vec![HirStmt::Break],
        }]);
        let exit_lbl = f.blocks.iter()
            .find(|b| b.label.starts_with("while_exit"))
            .map(|b| b.label.clone()).unwrap();
        let body = f.blocks.iter().find(|b| b.label.starts_with("while_body")).unwrap();
        assert!(matches!(&body.term, Terminator::Jump(lbl) if *lbl == exit_lbl));
    }

    #[test]
    fn continue_terminates_body_block_with_jump_to_cond() {
        let f = lower(vec![HirStmt::While {
            cond: HirExpr::LitBool(true),
            body: vec![HirStmt::Continue],
        }]);
        let cond_lbl = f.blocks.iter()
            .find(|b| b.label.starts_with("while_cond"))
            .map(|b| b.label.clone()).unwrap();
        let body = f.blocks.iter().find(|b| b.label.starts_with("while_body")).unwrap();
        assert!(matches!(&body.term, Terminator::Jump(lbl) if *lbl == cond_lbl));
    }

    // ── For ───────────────────────────────────────────────────────────────────

    #[test]
    fn for_loop_produces_cond_body_update_exit_blocks() {
        let f = lower(vec![HirStmt::For {
            init: Box::new(HirStmt::Let {
                name: "i".into(), ty: SzType::Int,
                value: HirExpr::LitInt(0), is_const: false,
            }),
            cond: lt(int_var("i"), HirExpr::LitInt(10)),
            update: Box::new(HirStmt::Assign(
                HirLValue::Var("i".into()),
                add(int_var("i"), HirExpr::LitInt(1)),
            )),
            body: vec![],
        }]);
        assert!(f.blocks.iter().any(|b| b.label.starts_with("for_cond")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("for_body")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("for_update")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("for_exit")));
    }

    #[test]
    fn for_update_block_jumps_back_to_cond() {
        let f = lower(vec![HirStmt::For {
            init: Box::new(HirStmt::Let {
                name: "i".into(), ty: SzType::Int, value: HirExpr::LitInt(0), is_const: false,
            }),
            cond: lt(int_var("i"), HirExpr::LitInt(5)),
            update: Box::new(HirStmt::Assign(
                HirLValue::Var("i".into()),
                add(int_var("i"), HirExpr::LitInt(1)),
            )),
            body: vec![],
        }]);
        let cond_lbl = f.blocks.iter()
            .find(|b| b.label.starts_with("for_cond"))
            .map(|b| b.label.clone()).unwrap();
        let update = f.blocks.iter().find(|b| b.label.starts_with("for_update")).unwrap();
        assert!(matches!(&update.term, Terminator::Jump(lbl) if *lbl == cond_lbl));
    }

    // ── Out ───────────────────────────────────────────────────────────────────

    #[test]
    fn out_statement_emits_out_instruction() {
        let f = lower(vec![HirStmt::Out(HirExpr::LitStr("hello".into()))]);
        assert!(f.blocks[0].instrs.iter().any(|i| {
            matches!(i, MirInstr::Out(MirVal::ConstStr(s)) if s == "hello")
        }));
    }

    // ── Function params ───────────────────────────────────────────────────────

    #[test]
    fn function_params_preserved_in_mir() {
        let prog = HirProgram { functions: vec![HirFunction {
            name: "add".into(),
            params: vec![
                HirParam { name: "a".into(), ty: SzType::Int },
                HirParam { name: "b".into(), ty: SzType::Int },
            ],
            ret_type: SzType::Int,
            body: vec![HirStmt::Return(Some(int_var("a")))],
        }]};
        let f = MirLowerer::new().lower_program(&prog).functions.remove(0);
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0], ("a".to_string(), SzType::Int));
        assert_eq!(f.params[1], ("b".to_string(), SzType::Int));
        assert_eq!(f.ret_type, SzType::Int);
    }

    // ── Conditional expression (HirExpr::If) ──────────────────────────────────

    #[test]
    fn conditional_expr_creates_if_then_else_merge_blocks() {
        let cond_expr = HirExpr::If {
            cond:      Box::new(HirExpr::LitBool(true)),
            then_expr: Box::new(HirExpr::LitInt(1)),
            else_expr: Box::new(HirExpr::LitInt(2)),
            ty:        SzType::Int,
        };
        let f = lower(vec![HirStmt::Let { name: "v".into(), ty: SzType::Int, value: cond_expr, is_const: false }]);
        assert!(f.blocks.iter().any(|b| b.label.starts_with("if_then")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("if_else")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("if_merge")));
        // merge block stores result into the named variable
        let merge = f.blocks.iter().find(|b| b.label.starts_with("if_merge")).unwrap();
        assert!(merge.instrs.iter().any(|i| matches!(i, MirInstr::Store(n, _) if n == "v")));
    }

    // ── App: fibonacci ────────────────────────────────────────────────────────

    #[test]
    fn fibonacci_program_lowers_to_valid_mir() {
        // fn int fib(int n) { if n <= 1 { return n; } return fib(n-1) + fib(n-2); }
        let prog = HirProgram { functions: vec![HirFunction {
            name: "fib".into(),
            params: vec![HirParam { name: "n".into(), ty: SzType::Int }],
            ret_type: SzType::Int,
            body: vec![
                HirStmt::If {
                    cond: HirExpr::BinOp {
                        op: HirBinOp::Le,
                        left:  Box::new(int_var("n")),
                        right: Box::new(HirExpr::LitInt(1)),
                        ty: SzType::Bool,
                    },
                    then_body: vec![HirStmt::Return(Some(int_var("n")))],
                    else_body: vec![],
                },
                HirStmt::Return(Some(add(
                    HirExpr::Call { name: "fib".into(), args: vec![
                        HirExpr::BinOp { op: HirBinOp::Sub, left: Box::new(int_var("n")), right: Box::new(HirExpr::LitInt(1)), ty: SzType::Int }
                    ], ty: SzType::Int },
                    HirExpr::Call { name: "fib".into(), args: vec![
                        HirExpr::BinOp { op: HirBinOp::Sub, left: Box::new(int_var("n")), right: Box::new(HirExpr::LitInt(2)), ty: SzType::Int }
                    ], ty: SzType::Int },
                ))),
            ],
        }]};
        let f = MirLowerer::new().lower_program(&prog).functions.remove(0);
        assert_eq!(f.name, "fib");
        // must have the base-case if blocks
        assert!(f.blocks.iter().any(|b| b.label.starts_with("then")));
        // must emit recursive Call instructions
        assert!(f.blocks.iter().any(|b|
            b.instrs.iter().any(|i| matches!(i, MirInstr::Call(_, nm, _) if nm == "fib"))
        ));
    }

    // ── App: sum 0..n (for loop + accumulator) ────────────────────────────────

    #[test]
    fn sum_n_for_loop_program() {
        // let sum = 0;
        // for (let i = 0; i < 10; i = i + 1) { sum = sum + i; }
        // return sum;
        let prog = HirProgram { functions: vec![HirFunction {
            name: "sum_n".into(),
            params: vec![],
            ret_type: SzType::Int,
            body: vec![
                HirStmt::Let { name: "sum".into(), ty: SzType::Int, value: HirExpr::LitInt(0), is_const: false },
                HirStmt::For {
                    init: Box::new(HirStmt::Let { name: "i".into(), ty: SzType::Int, value: HirExpr::LitInt(0), is_const: false }),
                    cond: lt(int_var("i"), HirExpr::LitInt(10)),
                    update: Box::new(HirStmt::Assign(HirLValue::Var("i".into()), add(int_var("i"), HirExpr::LitInt(1)))),
                    body: vec![
                        HirStmt::Assign(HirLValue::Var("sum".into()), add(int_var("sum"), int_var("i"))),
                    ],
                },
                HirStmt::Return(Some(int_var("sum"))),
            ],
        }]};
        let f = MirLowerer::new().lower_program(&prog).functions.remove(0);
        // structural checks
        assert!(f.blocks.iter().any(|b| b.label.starts_with("for_cond")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("for_body")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("for_update")));
        // body has Store for "sum"
        let body = f.blocks.iter().find(|b| b.label.starts_with("for_body")).unwrap();
        assert!(body.instrs.iter().any(|i| matches!(i, MirInstr::Store(n, _) if n == "sum")));
    }

    // ── App: count_down (while + break) ───────────────────────────────────────

    #[test]
    fn countdown_while_with_break() {
        // let n = 5;
        // while (n > 0) { n = n - 1; if n == 2 { break; } }
        let prog = HirProgram { functions: vec![HirFunction {
            name: "countdown".into(),
            params: vec![],
            ret_type: SzType::Void,
            body: vec![
                HirStmt::Let { name: "n".into(), ty: SzType::Int, value: HirExpr::LitInt(5), is_const: false },
                HirStmt::While {
                    cond: HirExpr::BinOp {
                        op: HirBinOp::Gt,
                        left: Box::new(int_var("n")), right: Box::new(HirExpr::LitInt(0)),
                        ty: SzType::Bool,
                    },
                    body: vec![
                        HirStmt::Assign(
                            HirLValue::Var("n".into()),
                            HirExpr::BinOp { op: HirBinOp::Sub, left: Box::new(int_var("n")), right: Box::new(HirExpr::LitInt(1)), ty: SzType::Int },
                        ),
                        HirStmt::If {
                            cond: HirExpr::BinOp {
                                op: HirBinOp::Eq,
                                left: Box::new(int_var("n")), right: Box::new(HirExpr::LitInt(2)),
                                ty: SzType::Bool,
                            },
                            then_body: vec![HirStmt::Break],
                            else_body: vec![],
                        },
                    ],
                },
            ],
        }]};
        let f = MirLowerer::new().lower_program(&prog).functions.remove(0);
        assert!(f.blocks.iter().any(|b| b.label.starts_with("while_cond")));
        assert!(f.blocks.iter().any(|b| b.label.starts_with("while_exit")));
        // break → some block jumps to while_exit
        let exit_lbl = f.blocks.iter()
            .find(|b| b.label.starts_with("while_exit"))
            .map(|b| b.label.clone()).unwrap();
        assert!(f.blocks.iter().any(|b| matches!(&b.term, Terminator::Jump(lbl) if *lbl == exit_lbl)));
    }
}
