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

        // Seal the last open block if it has no terminator yet
        if !fl.cur_instrs.is_empty() || fl.blocks.is_empty() {
            fl.seal(Terminator::Return(None));
        }

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
