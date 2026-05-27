// Autodiff namespace — reverse-mode automatic differentiation (tape-based)
//
// Autodiff.tape()          → null      start recording
// Autodiff.backward(loss)  → null      run backward pass from a scalar tensor loss
// Autodiff.gradient(t)     → Tensor    retrieve accumulated gradient for tensor t
// Autodiff.clear()         → null      clear tape + gradients
// Autodiff.isRecording()   → bool      true if tape is currently active

use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;

// ── Tape structures ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum TapeOp {
    MatMul { a_id: u64, b_id: u64, a_rows: usize, a_cols: usize, b_cols: usize,
             a_data: Vec<f64>, b_data: Vec<f64> },
    Add    { a_id: u64, b_id: u64 },
    Sub    { a_id: u64, b_id: u64 },
    Mul    { a_id: u64, b_id: u64, a_data: Vec<f64>, b_data: Vec<f64> },
    Scale  { in_id: u64, scalar: f64 },
    Neg    { in_id: u64 },
    BroadcastAdd { mat_id: u64, bias_id: u64, rows: usize, cols: usize },
    BroadcastMul { mat_id: u64, rhs_id: u64, rows: usize, cols: usize },
    Relu   { in_id: u64, cached_input: Vec<f64> },
    Sigmoid { in_id: u64, cached_output: Vec<f64> },
    Tanh   { in_id: u64, cached_output: Vec<f64> },
    Sum    { in_id: u64, in_len: usize },
    Mean   { in_id: u64, in_len: usize },
}

#[derive(Clone, Debug)]
pub struct TapeEntry {
    pub out_id: u64,
    pub op: TapeOp,
}

// ── Evaluator methods ─────────────────────────────────────────────────────────

impl super::Evaluator {
    pub(super) fn eval_autodiff_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {

            "tape" => {
                if !dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Autodiff.tape() takes no arguments");
                    return EvalResult::Error;
                }
                self.ad_recording = true;
                self.ad_tape.clear();
                self.ad_grads.clear();
                self.ad_tensor_ids.clear();
                self.ad_next_id = 1;
                EvalResult::Value(self.null_ref)
            }

            "clear" => {
                self.ad_recording = false;
                self.ad_tape.clear();
                self.ad_grads.clear();
                self.ad_tensor_ids.clear();
                self.ad_next_id = 1;
                EvalResult::Value(self.null_ref)
            }

            "isRecording" => {
                let b = self.ad_recording;
                EvalResult::Value(self.bool_ref(b))
            }

            "backward" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Autodiff.backward(loss) requires 1 argument");
                    return EvalResult::Error;
                }
                let loss_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let (loss_data, loss_tid) = match self.resolve(loss_ref).cloned() {
                    Some(ObjectData::Tensor { data, tid, .. }) => (data, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.backward() argument must be a Tensor"); return EvalResult::Error; }
                };
                let seed_id = match self.ad_tensor_ids.get(&loss_tid).copied() {
                    Some(id) => id,
                    None => {
                        eprintln!("❌ ERROR: Autodiff.backward() loss tensor was not created during tape recording");
                        return EvalResult::Error;
                    }
                };
                self.ad_grads.insert(seed_id, vec![1.0; loss_data.len()]);

                // Run backward pass in reverse order
                let tape = self.ad_tape.clone();
                for entry in tape.iter().rev() {
                    let out_grad = match self.ad_grads.get(&entry.out_id).cloned() {
                        Some(g) => g,
                        None => continue, // not yet reachable
                    };
                    match &entry.op {
                        TapeOp::Add { a_id, b_id } => {
                            Self::accum_grad(&mut self.ad_grads, *a_id, out_grad.clone());
                            Self::accum_grad(&mut self.ad_grads, *b_id, out_grad);
                        }
                        TapeOp::Sub { a_id, b_id } => {
                            Self::accum_grad(&mut self.ad_grads, *a_id, out_grad.clone());
                            let neg: Vec<f64> = out_grad.iter().map(|x| -x).collect();
                            Self::accum_grad(&mut self.ad_grads, *b_id, neg);
                        }
                        TapeOp::Mul { a_id, b_id, a_data, b_data } => {
                            // dA[i] = out_grad[i] * B[i],  dB[i] = out_grad[i] * A[i]
                            let da: Vec<f64> = out_grad.iter().zip(b_data.iter()).map(|(g, b)| g * b).collect();
                            let db: Vec<f64> = out_grad.iter().zip(a_data.iter()).map(|(g, a)| g * a).collect();
                            Self::accum_grad(&mut self.ad_grads, *a_id, da);
                            Self::accum_grad(&mut self.ad_grads, *b_id, db);
                        }
                        TapeOp::Scale { in_id, scalar } => {
                            let g: Vec<f64> = out_grad.iter().map(|x| x * scalar).collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::Neg { in_id } => {
                            let g: Vec<f64> = out_grad.iter().map(|x| -x).collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::BroadcastAdd { mat_id, bias_id, rows, cols } => {
                            let (r, c) = (*rows, *cols);
                            Self::accum_grad(&mut self.ad_grads, *mat_id, out_grad.clone());
                            let mut bias_grad = vec![0.0f64; c];
                            for row in 0..r { for col in 0..c { bias_grad[col] += out_grad[row * c + col]; } }
                            Self::accum_grad(&mut self.ad_grads, *bias_id, bias_grad);
                        }
                        TapeOp::BroadcastMul { mat_id, rhs_id, rows, cols } => {
                            let (r, c) = (*rows, *cols);
                            // d_mat[i,j] = out_grad[i,j] * rhs[j]  (need rhs values — not saved, skip for now)
                            // d_rhs[j] = sum_i(out_grad[i,j] * mat[i,j])
                            // Without saved inputs we can only propagate to mat_id (times 1.0)
                            Self::accum_grad(&mut self.ad_grads, *mat_id, out_grad.clone());
                            let _ = (r, c, rhs_id); // rhs gradient requires saved inputs
                        }
                        TapeOp::MatMul { a_id, b_id, a_rows, a_cols, b_cols, a_data, b_data } => {
                            let (m, k, n) = (*a_rows, *a_cols, *b_cols);
                            // dL/dA[i,j] = sum_l out_grad[i,l] * B[j,l]
                            let mut da = vec![0.0f64; m * k];
                            for i in 0..m {
                                for j in 0..k {
                                    for l in 0..n {
                                        da[i * k + j] += out_grad[i * n + l] * b_data[j * n + l];
                                    }
                                }
                            }
                            // dL/dB[j,l] = sum_i A[i,j] * out_grad[i,l]
                            let mut db = vec![0.0f64; k * n];
                            for j in 0..k {
                                for l in 0..n {
                                    for i in 0..m {
                                        db[j * n + l] += a_data[i * k + j] * out_grad[i * n + l];
                                    }
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *a_id, da);
                            Self::accum_grad(&mut self.ad_grads, *b_id, db);
                        }
                        TapeOp::Relu { in_id, cached_input } => {
                            let g: Vec<f64> = out_grad.iter().zip(cached_input.iter())
                                .map(|(dout, &x)| if x > 0.0 { *dout } else { 0.0 })
                                .collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::Sigmoid { in_id, cached_output } => {
                            let g: Vec<f64> = out_grad.iter().zip(cached_output.iter())
                                .map(|(dout, &s)| dout * s * (1.0 - s))
                                .collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::Tanh { in_id, cached_output } => {
                            let g: Vec<f64> = out_grad.iter().zip(cached_output.iter())
                                .map(|(dout, &t)| dout * (1.0 - t * t))
                                .collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::Sum { in_id, in_len } => {
                            // Scalar sum: broadcast gradient to all inputs
                            let g_val = out_grad.iter().sum::<f64>();
                            Self::accum_grad(&mut self.ad_grads, *in_id, vec![g_val; *in_len]);
                        }
                        TapeOp::Mean { in_id, in_len } => {
                            let n = *in_len as f64;
                            let g_val = out_grad.iter().sum::<f64>() / n;
                            Self::accum_grad(&mut self.ad_grads, *in_id, vec![g_val; *in_len]);
                        }
                    }
                }
                self.ad_recording = false;
                EvalResult::Value(self.null_ref)
            }

            "gradient" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Autodiff.gradient(tensor) requires 1 argument");
                    return EvalResult::Error;
                }
                let t_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let (shape, t_tid) = match self.resolve(t_ref).cloned() {
                    Some(ObjectData::Tensor { shape, tid, .. }) => (shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.gradient() argument must be a Tensor"); return EvalResult::Error; }
                };
                let tape_id = match self.ad_tensor_ids.get(&t_tid).copied() {
                    Some(id) => id,
                    None => {
                        eprintln!("❌ ERROR: Autodiff.gradient() tensor was not recorded during tape (tid={})", t_tid);
                        return EvalResult::Error;
                    }
                };
                let grad_data = self.ad_grads.get(&tape_id).cloned()
                    .unwrap_or_else(|| vec![0.0; shape.iter().product()]);
                EvalResult::Value(self.alloc_tensor(shape, grad_data))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown Autodiff method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── Tape recording helpers ────────────────────────────────────────────────

    pub(super) fn ad_tensor_id(&mut self, obj_ref: crate::region::ObjectRef) -> u64 {
        let tid = match self.resolve(obj_ref) {
            Some(crate::region::ObjectData::Tensor { tid, .. }) => *tid,
            _ => return 0,
        };
        if let Some(&id) = self.ad_tensor_ids.get(&tid) {
            return id;
        }
        let id = self.ad_next_id;
        self.ad_next_id += 1;
        self.ad_tensor_ids.insert(tid, id);
        id
    }

    pub(super) fn ad_push(&mut self, out_ref: crate::region::ObjectRef, op: TapeOp) {
        let out_id = self.ad_tensor_id(out_ref);
        self.ad_tape.push(TapeEntry { out_id, op });
    }

    fn accum_grad(grads: &mut std::collections::HashMap<u64, Vec<f64>>, id: u64, delta: Vec<f64>) {
        let entry = grads.entry(id).or_insert_with(|| vec![0.0; delta.len()]);
        if entry.len() == delta.len() {
            for (e, d) in entry.iter_mut().zip(delta.iter()) {
                *e += d;
            }
        }
    }
}
