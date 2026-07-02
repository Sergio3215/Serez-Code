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
    BroadcastMul { mat_id: u64, rhs_id: u64, rows: usize, cols: usize, mat_data: Vec<f64>, rhs_data: Vec<f64> },
    Relu   { in_id: u64, cached_input: Vec<f64> },
    Sigmoid { in_id: u64, cached_output: Vec<f64> },
    Tanh   { in_id: u64, cached_output: Vec<f64> },
    Gelu   { in_id: u64, cached_input: Vec<f64> },
    Sum    { in_id: u64, in_len: usize },
    Mean   { in_id: u64, in_len: usize },
    Conv2d {
        in_id: u64, w_id: u64, b_id: u64,
        kernel: usize, stride: usize,
        in_shape: Vec<usize>,
        w_data: Vec<f64>,
        col_mat: Vec<f64>,
        col_rows: usize, col_cols: usize, c_out: usize,
    },
    MaxPool2d {
        in_id: u64,
        kernel: usize, stride: usize,
        in_shape: Vec<usize>,
        out_shape: Vec<usize>,
        max_indices: Vec<usize>,
    },
    Lstm {
        x_id: u64, wx_id: u64, wh_id: u64, b_id: u64, h0_id: u64, c0_id: u64,
        x_data: Vec<f64>,
        wx_data: Vec<f64>,
        wh_data: Vec<f64>,
        h_all: Vec<f64>,    // [seq_len+1, hidden_size] — h0 at index 0
        c_all: Vec<f64>,    // [seq_len+1, hidden_size] — c0 at index 0
        gates_i: Vec<f64>,  // [seq_len, hidden_size]
        gates_f: Vec<f64>,
        gates_g: Vec<f64>,
        gates_o: Vec<f64>,
        seq_len: usize, input_size: usize, hidden_size: usize,
    },
    Gru {
        x_id: u64, wx_id: u64, wh_id: u64, b_id: u64, h0_id: u64,
        x_data: Vec<f64>,
        wx_data: Vec<f64>,
        wh_data: Vec<f64>,
        h_all: Vec<f64>,    // [seq_len+1, hidden_size]
        gates_r: Vec<f64>,  // [seq_len, hidden_size] — reset gate output
        gates_z: Vec<f64>,  // [seq_len, hidden_size] — update gate output
        gates_n: Vec<f64>,  // [seq_len, hidden_size] — new gate output
        seq_len: usize, input_size: usize, hidden_size: usize,
    },
    Transpose { in_id: u64, rows: usize, cols: usize },
    Softmax { in_id: u64, rows: usize, cols: usize, cached_out: Vec<f64> },
    LayerNorm {
        in_id: u64, g_id: u64, b_id: u64,
        eps: f64,
        x_norm: Vec<f64>,      // [rows, cols] normalized x (before scale/shift)
        stds: Vec<f64>,        // [rows] sqrt(var+eps) per row
        x_mu: Vec<f64>,        // [rows, cols] x - mean per row
        gamma_data: Vec<f64>,  // [cols]
        rows: usize, cols: usize,
    },
    Mha {
        x_id: u64, wq_id: u64, wk_id: u64, wv_id: u64, wo_id: u64,
        x_data: Vec<f64>,
        wq_data: Vec<f64>, wk_data: Vec<f64>, wv_data: Vec<f64>, wo_data: Vec<f64>,
        q_proj: Vec<f64>, k_proj: Vec<f64>, v_proj: Vec<f64>,
        attn_weights: Vec<f64>, // [n_heads * sl * sl]
        concat_heads: Vec<f64>, // [sl, dm]
        seq_len: usize, d_model: usize, n_heads: usize, dh: usize,
    },
    // ── Phase 1: Loss functions ───────────────────────────────────────────────
    MseLoss { pred_id: u64, target_data: Vec<f64>, n: usize },
    MaeLoss { pred_id: u64, signs: Vec<f64>, n: usize },
    BceLoss { pred_id: u64, pred_data: Vec<f64>, target_data: Vec<f64>, n: usize },
    CrossEntropyLoss { logits_id: u64, probs: Vec<f64>, target_indices: Vec<usize>, batch: usize, classes: usize },
    // ── Phase 2: Activations ─────────────────────────────────────────────────
    Elu   { in_id: u64, cached_input: Vec<f64>, alpha: f64 },
    Swish { in_id: u64, cached_input: Vec<f64>, cached_sigmoid: Vec<f64> },
    Mish  { in_id: u64, cached_input: Vec<f64> },
    // ── Phase 2: Normalization & Regularization ───────────────────────────────
    BatchNorm {
        in_id: u64, g_id: u64, b_id: u64,
        eps: f64,
        x_norm: Vec<f64>,   // [N, C] normalized values
        stds:   Vec<f64>,   // [C] per-feature std
        x_mu:   Vec<f64>,   // [N, C] x - mean
        gamma_data: Vec<f64>,
        rows: usize, cols: usize,
    },
    Dropout { in_id: u64, mask: Vec<f64>, keep_prob: f64 },
    // ── Phase 2: Embedding ───────────────────────────────────────────────────
    Embedding { w_id: u64, indices: Vec<usize>, seq_len: usize, emb_dim: usize, vocab_size: usize },
    // ── Phase 2: Pooling ─────────────────────────────────────────────────────
    AvgPool2d {
        in_id: u64,
        kernel: usize, stride: usize,
        in_shape: Vec<usize>,
        out_shape: Vec<usize>,
    },
    // ── Phase 2: Leaky relu (tracked) ────────────────────────────────────────
    LeakyRelu { in_id: u64, cached_input: Vec<f64>, alpha: f64 },
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
                        TapeOp::BroadcastMul { mat_id, rhs_id, rows, cols, mat_data, rhs_data } => {
                            let (r, c) = (*rows, *cols);
                            // d_mat[i,j] = out_grad[i,j] * rhs[j]
                            let mut dmat = vec![0.0f64; r * c];
                            for i in 0..r { for j in 0..c { dmat[i*c+j] = out_grad[i*c+j] * rhs_data[j]; } }
                            Self::accum_grad(&mut self.ad_grads, *mat_id, dmat);
                            // d_rhs[j] = sum_i(out_grad[i,j] * mat[i,j])
                            let mut drhs = vec![0.0f64; c];
                            for i in 0..r { for j in 0..c { drhs[j] += out_grad[i*c+j] * mat_data[i*c+j]; } }
                            Self::accum_grad(&mut self.ad_grads, *rhs_id, drhs);
                        }
                        TapeOp::MatMul { a_id, b_id, a_rows, a_cols, b_cols, a_data, b_data } => {
                            let (m, k, n) = (*a_rows, *a_cols, *b_cols);
                            // dL/dA = out_grad (m×n) · Bᵀ (n×k). Transpose B (k×n) → Bᵀ (n×k),
                            // then reuse the shared matmul kernel (same accumulation order → identical result).
                            let mut bt = vec![0.0f64; n * k];
                            for j in 0..k { for l in 0..n { bt[l * k + j] = b_data[j * n + l]; } }
                            let mut da = vec![0.0f64; m * k];
                            super::methods_tensor::matmul_kernel(&out_grad, &bt, m, n, k, &mut da);
                            // dL/dB = Aᵀ (k×m) · out_grad (m×n). Transpose A (m×k) → Aᵀ (k×m).
                            let mut at = vec![0.0f64; k * m];
                            for i in 0..m { for j in 0..k { at[j * m + i] = a_data[i * k + j]; } }
                            let mut db = vec![0.0f64; k * n];
                            super::methods_tensor::matmul_kernel(&at, &out_grad, k, m, n, &mut db);
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
                        TapeOp::Conv2d { in_id, w_id, b_id, kernel, stride, in_shape, w_data, col_mat, col_rows, col_cols, c_out } => {
                            let (cr, cc, co) = (*col_rows, *col_cols, *c_out);
                            // dWeights = col_mat^T @ grad_out  [cc x cr] @ [cr x co] → [cc x co]
                            let mut dw = vec![0.0f64; cc * co];
                            for l in 0..cc {
                                for j in 0..co {
                                    for i in 0..cr {
                                        dw[l * co + j] += col_mat[i * cc + l] * out_grad[i * co + j];
                                    }
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *w_id, dw);
                            // dBias = row-sum of grad_out → [co]
                            let mut db = vec![0.0f64; co];
                            for i in 0..cr {
                                for j in 0..co {
                                    db[j] += out_grad[i * co + j];
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *b_id, db);
                            // dCol = grad_out @ W^T  [cr x co] @ [co x cc] → [cr x cc]
                            let mut dcol = vec![0.0f64; cr * cc];
                            for i in 0..cr {
                                for l in 0..cc {
                                    for j in 0..co {
                                        dcol[i * cc + l] += out_grad[i * co + j] * w_data[l * co + j];
                                    }
                                }
                            }
                            // col2im: scatter dcol back to dinput [N, H, W, C_in]
                            let (n, h, w_in, c_in) = (in_shape[0], in_shape[1], in_shape[2], in_shape[3]);
                            let ks = *kernel;
                            let st = *stride;
                            let out_h = (h - ks) / st + 1;
                            let out_w = (w_in - ks) / st + 1;
                            let mut dinput = vec![0.0f64; n * h * w_in * c_in];
                            for nb in 0..n {
                                for oh in 0..out_h {
                                    for ow in 0..out_w {
                                        let row = nb * out_h * out_w + oh * out_w + ow;
                                        for kh in 0..ks {
                                            for kw in 0..ks {
                                                for ci in 0..c_in {
                                                    let ih = oh * st + kh;
                                                    let iw = ow * st + kw;
                                                    let in_idx = nb*h*w_in*c_in + ih*w_in*c_in + iw*c_in + ci;
                                                    let col_idx = row * cc + kh * ks * c_in + kw * c_in + ci;
                                                    dinput[in_idx] += dcol[col_idx];
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *in_id, dinput);
                        }
                        TapeOp::MaxPool2d { in_id, in_shape, max_indices, .. } => {
                            let in_total: usize = in_shape.iter().product();
                            let mut dinput = vec![0.0f64; in_total];
                            for (i, &idx) in max_indices.iter().enumerate() {
                                if i < out_grad.len() {
                                    dinput[idx] += out_grad[i];
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *in_id, dinput);
                        }
                        TapeOp::Lstm { x_id, wx_id, wh_id, b_id, h0_id, c0_id,
                                       x_data, wx_data, wh_data, h_all, c_all,
                                       gates_i, gates_f, gates_g, gates_o,
                                       seq_len, input_size, hidden_size } => {
                            let (sl, is, hs) = (*seq_len, *input_size, *hidden_size);
                            let four_h = 4 * hs;
                            let mut dx  = vec![0.0f64; sl * is];
                            let mut dwx = vec![0.0f64; is * four_h];
                            let mut dwh = vec![0.0f64; hs * four_h];
                            let mut db  = vec![0.0f64; four_h];
                            let mut dh_next = vec![0.0f64; hs];
                            let mut dc_next = vec![0.0f64; hs];
                            // out_grad: [1, hs] — gradient of loss w.r.t. last hidden state
                            for t in (0..sl).rev() {
                                let mut dh = dh_next.clone();
                                if t == sl - 1 {
                                    for j in 0..hs { dh[j] += out_grad[j]; }
                                }
                                let i_t = &gates_i[t*hs..(t+1)*hs];
                                let f_t = &gates_f[t*hs..(t+1)*hs];
                                let g_t = &gates_g[t*hs..(t+1)*hs];
                                let o_t = &gates_o[t*hs..(t+1)*hs];
                                let c_t   = &c_all[(t+1)*hs..(t+2)*hs];
                                let c_prev = &c_all[t*hs..(t+1)*hs];
                                let h_prev = &h_all[t*hs..(t+1)*hs];
                                let x_t   = &x_data[t*is..(t+1)*is];
                                // do_t = dh * tanh(c_t)
                                let do_t: Vec<f64> = (0..hs).map(|j| dh[j] * c_t[j].tanh()).collect();
                                // dc = dh * o_t * (1 - tanh(c_t)^2) + dc_next
                                let dc: Vec<f64> = (0..hs).map(|j| {
                                    let tc = c_t[j].tanh();
                                    dh[j] * o_t[j] * (1.0 - tc*tc) + dc_next[j]
                                }).collect();
                                let df: Vec<f64> = (0..hs).map(|j| dc[j] * c_prev[j]).collect();
                                let di: Vec<f64> = (0..hs).map(|j| dc[j] * g_t[j]).collect();
                                let dg: Vec<f64> = (0..hs).map(|j| dc[j] * i_t[j]).collect();
                                // Gate pre-activation gradients
                                let mut dz = vec![0.0f64; four_h];
                                for j in 0..hs {
                                    dz[j]        = di[j] * i_t[j] * (1.0 - i_t[j]);
                                    dz[hs+j]     = df[j] * f_t[j] * (1.0 - f_t[j]);
                                    dz[2*hs+j]   = dg[j] * (1.0 - g_t[j]*g_t[j]);
                                    dz[3*hs+j]   = do_t[j] * o_t[j] * (1.0 - o_t[j]);
                                }
                                // dx_t = dz @ Wx^T
                                for k in 0..is {
                                    for j in 0..four_h {
                                        dx[t*is+k] += dz[j] * wx_data[k*four_h+j];
                                    }
                                }
                                // dWx += x_t^T ⊗ dz
                                for k in 0..is {
                                    for j in 0..four_h { dwx[k*four_h+j] += x_t[k] * dz[j]; }
                                }
                                // dWh += h_prev^T ⊗ dz
                                for k in 0..hs {
                                    for j in 0..four_h { dwh[k*four_h+j] += h_prev[k] * dz[j]; }
                                }
                                for j in 0..four_h { db[j] += dz[j]; }
                                // dh_next = dz @ Wh^T
                                let mut new_dh_next = vec![0.0f64; hs];
                                for k in 0..hs {
                                    for j in 0..four_h {
                                        new_dh_next[k] += dz[j] * wh_data[k*four_h+j];
                                    }
                                }
                                dh_next = new_dh_next;
                                dc_next = (0..hs).map(|j| dc[j] * f_t[j]).collect();
                            }
                            Self::accum_grad(&mut self.ad_grads, *x_id,  dx);
                            Self::accum_grad(&mut self.ad_grads, *wx_id, dwx);
                            Self::accum_grad(&mut self.ad_grads, *wh_id, dwh);
                            Self::accum_grad(&mut self.ad_grads, *b_id,  db);
                            Self::accum_grad(&mut self.ad_grads, *h0_id, dh_next);
                            Self::accum_grad(&mut self.ad_grads, *c0_id, dc_next);
                        }
                        TapeOp::Transpose { in_id, rows, cols } => {
                            // Gradient of transpose is transpose of gradient
                            let (r, c) = (*rows, *cols);
                            let mut din = vec![0.0f64; r * c];
                            for i in 0..r { for j in 0..c { din[i * c + j] = out_grad[j * r + i]; } }
                            Self::accum_grad(&mut self.ad_grads, *in_id, din);
                        }
                        TapeOp::Softmax { in_id, rows, cols, cached_out } => {
                            let (r, c) = (*rows, *cols);
                            let mut dx = vec![0.0f64; r * c];
                            for row in 0..r {
                                let s = &cached_out[row*c..(row+1)*c];
                                let g = &out_grad[row*c..(row+1)*c];
                                let dot: f64 = g.iter().zip(s.iter()).map(|(gi, si)| gi * si).sum();
                                for col in 0..c { dx[row*c+col] = s[col] * (g[col] - dot); }
                            }
                            Self::accum_grad(&mut self.ad_grads, *in_id, dx);
                        }
                        TapeOp::LayerNorm { in_id, g_id, b_id, eps: _, x_norm, stds, x_mu, gamma_data, rows, cols } => {
                            let (r, c) = (*rows, *cols);
                            // d_gamma = sum_rows(d_out * x_norm)
                            let mut dgamma = vec![0.0f64; c];
                            // d_beta = sum_rows(d_out)
                            let mut dbeta = vec![0.0f64; c];
                            let mut dx = vec![0.0f64; r * c];
                            let n = c as f64;
                            for row in 0..r {
                                let std_i = stds[row];
                                let xn = &x_norm[row*c..(row+1)*c];
                                let xm = &x_mu[row*c..(row+1)*c];
                                let g = &out_grad[row*c..(row+1)*c];
                                for col in 0..c {
                                    dgamma[col] += g[col] * xn[col];
                                    dbeta[col]  += g[col];
                                }
                                // dx: standard layer norm backward
                                let d_xnorm: Vec<f64> = (0..c).map(|j| g[j] * gamma_data[j]).collect();
                                let sum_d_xnorm: f64 = d_xnorm.iter().sum();
                                let sum_d_xnorm_xn: f64 = d_xnorm.iter().zip(xn.iter()).map(|(a, b)| a * b).sum();
                                for col in 0..c {
                                    dx[row*c+col] = (d_xnorm[col] - sum_d_xnorm / n
                                        - xm[col] / (std_i * std_i) * sum_d_xnorm_xn / n) / std_i;
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *in_id, dx);
                            Self::accum_grad(&mut self.ad_grads, *g_id, dgamma);
                            Self::accum_grad(&mut self.ad_grads, *b_id, dbeta);
                        }
                        TapeOp::Mha { x_id, wq_id, wk_id, wv_id, wo_id,
                                      x_data, wq_data, wk_data, wv_data, wo_data,
                                      q_proj, k_proj, v_proj,
                                      attn_weights, concat_heads,
                                      seq_len, d_model, n_heads, dh } => {
                            let (sl, dm, nh, d) = (*seq_len, *d_model, *n_heads, *dh);
                            // Helper: [m,k] @ [k,n] → [m,n]
                            let mm = |a: &[f64], m: usize, k: usize, b: &[f64], n: usize| -> Vec<f64> {
                                let mut c = vec![0.0f64; m * n];
                                for i in 0..m { for l in 0..k { for j in 0..n { c[i*n+j] += a[i*k+l] * b[l*n+j]; } } }
                                c
                            };
                            // dconcat = d_out @ Wo^T  [sl, dm]
                            let d_concat = mm(&out_grad, sl, dm, wo_data, dm);   // out_grad@Wo not right — need Wo^T
                            // d_out @ Wo^T: [sl,dm]@[dm,dm]^T — Wo is [dm,dm], Wo^T is [dm,dm]
                            let mut d_concat2 = vec![0.0f64; sl * dm];
                            for i in 0..sl { for k in 0..dm { for j in 0..dm { d_concat2[i*dm+k] += out_grad[i*dm+j] * wo_data[k*dm+j]; } } }
                            // d_Wo = concat^T @ d_out  [dm,dm]
                            let mut dwo = vec![0.0f64; dm * dm];
                            for k in 0..dm { for j in 0..dm { for i in 0..sl { dwo[k*dm+j] += concat_heads[i*dm+k] * out_grad[i*dm+j]; } } }
                            let _ = d_concat; // drop incorrect version
                            // Per-head backward
                            let mut dq_proj = vec![0.0f64; sl * dm];
                            let mut dk_proj = vec![0.0f64; sl * dm];
                            let mut dv_proj = vec![0.0f64; sl * dm];
                            let scale = 1.0 / (d as f64).sqrt();
                            for h in 0..nh {
                                let attn_h = &attn_weights[h*sl*sl..(h+1)*sl*sl];
                                // Kh [sl,dh], Qh [sl,dh], Vh [sl,dh]
                                let mut kh = vec![0.0f64; sl * d];
                                let mut qh = vec![0.0f64; sl * d];
                                let mut vh = vec![0.0f64; sl * d];
                                for row in 0..sl {
                                    kh[row*d..row*d+d].copy_from_slice(&k_proj[row*dm+h*d..row*dm+h*d+d]);
                                    qh[row*d..row*d+d].copy_from_slice(&q_proj[row*dm+h*d..row*dm+h*d+d]);
                                    vh[row*d..row*d+d].copy_from_slice(&v_proj[row*dm+h*d..row*dm+h*d+d]);
                                }
                                // d_head_h = d_concat[:, h*d:(h+1)*d]  [sl, dh]
                                let mut dhead = vec![0.0f64; sl * d];
                                for row in 0..sl { dhead[row*d..row*d+d].copy_from_slice(&d_concat2[row*dm+h*d..row*dm+h*d+d]); }
                                // d_attn_h = dhead @ Vh^T  [sl,sl]
                                let mut dattn = vec![0.0f64; sl * sl];
                                for i in 0..sl { for j in 0..sl { for k in 0..d { dattn[i*sl+j] += dhead[i*d+k] * vh[j*d+k]; } } }
                                // d_Vh = attn_h^T @ dhead  [sl,dh]
                                let mut dvh = vec![0.0f64; sl * d];
                                for j in 0..sl { for k in 0..d { for i in 0..sl { dvh[j*d+k] += attn_h[i*sl+j] * dhead[i*d+k]; } } }
                                // Softmax backward on dattn with attn_h → d_scores_h [sl,sl]
                                let mut dscores = vec![0.0f64; sl * sl];
                                for row in 0..sl {
                                    let s = &attn_h[row*sl..(row+1)*sl];
                                    let g = &dattn[row*sl..(row+1)*sl];
                                    let dot: f64 = g.iter().zip(s.iter()).map(|(gi, si)| gi * si).sum();
                                    for col in 0..sl { dscores[row*sl+col] = s[col] * (g[col] - dot) * scale; }
                                }
                                // d_Qh = d_scores @ Kh  [sl,dh]
                                let dqh = mm(&dscores, sl, sl, &kh, d);
                                // d_Kh = d_scores^T @ Qh  [sl,dh]
                                let mut dkh = vec![0.0f64; sl * d];
                                for j in 0..sl { for k in 0..d { for i in 0..sl { dkh[j*d+k] += dscores[i*sl+j] * qh[i*d+k]; } } }
                                // Scatter back to full d_Q, d_K, d_V
                                for row in 0..sl {
                                    for col in 0..d {
                                        dq_proj[row*dm+h*d+col] += dqh[row*d+col];
                                        dk_proj[row*dm+h*d+col] += dkh[row*d+col];
                                        dv_proj[row*dm+h*d+col] += dvh[row*d+col];
                                    }
                                }
                            }
                            // d_Wq = x^T @ d_Q_proj  [dm,dm]
                            let mut dwq = vec![0.0f64; dm * dm];
                            let mut dwk = vec![0.0f64; dm * dm];
                            let mut dwv = vec![0.0f64; dm * dm];
                            for k in 0..dm { for j in 0..dm { for i in 0..sl {
                                dwq[k*dm+j] += x_data[i*dm+k] * dq_proj[i*dm+j];
                                dwk[k*dm+j] += x_data[i*dm+k] * dk_proj[i*dm+j];
                                dwv[k*dm+j] += x_data[i*dm+k] * dv_proj[i*dm+j];
                            }}}
                            // d_x = d_Q @ Wq^T + d_K @ Wk^T + d_V @ Wv^T  [sl,dm]
                            let mut dxd = vec![0.0f64; sl * dm];
                            for i in 0..sl { for k in 0..dm { for j in 0..dm {
                                dxd[i*dm+k] += dq_proj[i*dm+j] * wq_data[k*dm+j]
                                             + dk_proj[i*dm+j] * wk_data[k*dm+j]
                                             + dv_proj[i*dm+j] * wv_data[k*dm+j];
                            }}}
                            Self::accum_grad(&mut self.ad_grads, *x_id,  dxd);
                            Self::accum_grad(&mut self.ad_grads, *wq_id, dwq);
                            Self::accum_grad(&mut self.ad_grads, *wk_id, dwk);
                            Self::accum_grad(&mut self.ad_grads, *wv_id, dwv);
                            Self::accum_grad(&mut self.ad_grads, *wo_id, dwo);
                        }
                        // ── Phase 1: Loss function backwards ─────────────────
                        TapeOp::MseLoss { pred_id, target_data, n } => {
                            // d/d_pred = 2*(pred - target) / n
                            let g_scalar = out_grad.iter().sum::<f64>();
                            // We stored target; grad w.r.t. pred requires pred values.
                            // pred values are not stored — only the target.
                            // Workaround: The tape doesn't store pred_data, so we recover
                            // the gradient shape from target_data.
                            let scale = 2.0 * g_scalar / (*n as f64);
                            // We emit a "ones * scale" gradient — the real per-element
                            // gradient requires pred-target which is only available if we
                            // store it at record time (done via a separate step below).
                            // NOTE: MseLoss TapeOp now stores pred-target diff. See forward.
                            let dpred: Vec<f64> = target_data.iter()
                                .map(|diff| diff * scale).collect();
                            Self::accum_grad(&mut self.ad_grads, *pred_id, dpred);
                        }
                        TapeOp::MaeLoss { pred_id, signs, n } => {
                            let g_scalar = out_grad.iter().sum::<f64>();
                            let scale = g_scalar / (*n as f64);
                            let dpred: Vec<f64> = signs.iter().map(|s| s * scale).collect();
                            Self::accum_grad(&mut self.ad_grads, *pred_id, dpred);
                        }
                        TapeOp::BceLoss { pred_id, pred_data, target_data, n } => {
                            let g_scalar = out_grad.iter().sum::<f64>();
                            let scale = g_scalar / (*n as f64);
                            let eps = 1e-12_f64;
                            let dpred: Vec<f64> = pred_data.iter().zip(target_data.iter())
                                .map(|(&p, &t)| {
                                    let p = p.clamp(eps, 1.0 - eps);
                                    (-t / p + (1.0 - t) / (1.0 - p)) * scale
                                }).collect();
                            Self::accum_grad(&mut self.ad_grads, *pred_id, dpred);
                        }
                        TapeOp::CrossEntropyLoss { logits_id, probs, target_indices, batch, classes } => {
                            let g_scalar = out_grad.iter().sum::<f64>();
                            let scale = g_scalar / (*batch as f64);
                            let mut dlogits = vec![0.0f64; batch * classes];
                            for b in 0..*batch {
                                for c in 0..*classes {
                                    let mut d = probs[b * classes + c];
                                    if c == target_indices[b] { d -= 1.0; }
                                    dlogits[b * classes + c] = d * scale;
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *logits_id, dlogits);
                        }
                        // ── Phase 2: Activation backwards ────────────────────
                        TapeOp::Elu { in_id, cached_input, alpha } => {
                            let g: Vec<f64> = out_grad.iter().zip(cached_input.iter())
                                .map(|(dout, &x)| {
                                    if x > 0.0 { *dout }
                                    else { dout * alpha * x.exp() }
                                }).collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::Swish { in_id, cached_input, cached_sigmoid } => {
                            // swish(x) = x * sigmoid(x)
                            // d/dx = sigmoid(x) + x * sigmoid(x) * (1 - sigmoid(x))
                            //      = sigmoid(x) * (1 + x * (1 - sigmoid(x)))
                            let g: Vec<f64> = out_grad.iter()
                                .zip(cached_input.iter())
                                .zip(cached_sigmoid.iter())
                                .map(|((dout, &x), &s)| {
                                    let d = s * (1.0 + x * (1.0 - s));
                                    dout * d
                                }).collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::Gelu { in_id, cached_input } => {
                            // GELU(x) = 0.5*x*(1 + tanh(c*(x + 0.044715*x³)))  where c = sqrt(2/π)
                            // d/dx = 0.5*(1 + tanh(inner)) + 0.5*x*(1-tanh²(inner))*c*(1 + 3*0.044715*x²)
                            let c = (2.0_f64 / std::f64::consts::PI).sqrt();
                            let g: Vec<f64> = out_grad.iter().zip(cached_input.iter())
                                .map(|(dout, &x)| {
                                    let inner = c * (x + 0.044715 * x * x * x);
                                    let t = inner.tanh();
                                    let sech2 = 1.0 - t * t;
                                    let d = 0.5 * (1.0 + t) + 0.5 * x * sech2 * c * (1.0 + 3.0 * 0.044715 * x * x);
                                    dout * d
                                }).collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::Mish { in_id, cached_input } => {
                            // mish(x) = x * tanh(softplus(x))
                            // softplus(x) = log(1 + exp(x))
                            // d/dx = tanh(sp) + x * sech^2(sp) * sigmoid(x)
                            let g: Vec<f64> = out_grad.iter().zip(cached_input.iter())
                                .map(|(dout, &x)| {
                                    let sp = (1.0 + x.exp()).ln();
                                    let ts = sp.tanh();
                                    let sg = 1.0 / (1.0 + (-x).exp());
                                    let sech2 = 1.0 - ts * ts;
                                    let d = ts + x * sech2 * sg;
                                    dout * d
                                }).collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        TapeOp::LeakyRelu { in_id, cached_input, alpha } => {
                            let g: Vec<f64> = out_grad.iter().zip(cached_input.iter())
                                .map(|(dout, &x)| dout * if x > 0.0 { 1.0 } else { *alpha })
                                .collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        // ── Phase 2: BatchNorm backward ───────────────────────
                        TapeOp::BatchNorm { in_id, g_id, b_id, eps: _, x_norm, stds, x_mu, gamma_data, rows, cols } => {
                            let (n, c) = (*rows, *cols);
                            let nf = n as f64;
                            let mut dgamma = vec![0.0f64; c];
                            let mut dbeta  = vec![0.0f64; c];
                            let mut dx     = vec![0.0f64; n * c];
                            for j in 0..c {
                                let std_j = stds[j];
                                // dgamma[j] = sum_i(d_out[i,j] * x_norm[i,j])
                                // dbeta[j]  = sum_i(d_out[i,j])
                                for i in 0..n {
                                    dgamma[j] += out_grad[i*c+j] * x_norm[i*c+j];
                                    dbeta[j]  += out_grad[i*c+j];
                                }
                                // Standard batch-norm backward for each feature j
                                // dx[i,j] = (1/std_j/N) * (N*d_xnorm[i,j] - sum_d_xnorm[j]
                                //            - x_norm[i,j] * sum_d_xnorm_xn[j])
                                let mut sum_dxn = 0.0_f64;
                                let mut sum_dxn_xn = 0.0_f64;
                                for i in 0..n {
                                    let d_xn = out_grad[i*c+j] * gamma_data[j];
                                    sum_dxn    += d_xn;
                                    sum_dxn_xn += d_xn * x_norm[i*c+j];
                                }
                                for i in 0..n {
                                    let d_xn = out_grad[i*c+j] * gamma_data[j];
                                    dx[i*c+j] = (d_xn - sum_dxn / nf
                                        - x_norm[i*c+j] * sum_dxn_xn / nf) / std_j;
                                }
                                let _ = x_mu; // used during forward; not needed in backward
                            }
                            Self::accum_grad(&mut self.ad_grads, *in_id, dx);
                            Self::accum_grad(&mut self.ad_grads, *g_id,  dgamma);
                            Self::accum_grad(&mut self.ad_grads, *b_id,  dbeta);
                        }
                        // ── Phase 2: Dropout backward ─────────────────────────
                        TapeOp::Dropout { in_id, mask, keep_prob } => {
                            let kp = *keep_prob;
                            let g: Vec<f64> = out_grad.iter().zip(mask.iter())
                                .map(|(dout, &m)| dout * m / kp)
                                .collect();
                            Self::accum_grad(&mut self.ad_grads, *in_id, g);
                        }
                        // ── Phase 2: Embedding backward ───────────────────────
                        TapeOp::Embedding { w_id, indices, seq_len, emb_dim, vocab_size } => {
                            let (sl, ed, vs) = (*seq_len, *emb_dim, *vocab_size);
                            let mut dw = vec![0.0f64; vs * ed];
                            for (i, &idx) in indices.iter().enumerate() {
                                if i >= sl { break; }
                                if idx < vs {
                                    for k in 0..ed {
                                        dw[idx * ed + k] += out_grad[i * ed + k];
                                    }
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *w_id, dw);
                        }
                        // ── Phase 2: AvgPool2d backward ───────────────────────
                        TapeOp::AvgPool2d { in_id, kernel, stride, in_shape, out_shape } => {
                            let (n, in_h, in_w, c) = (in_shape[0], in_shape[1], in_shape[2], in_shape[3]);
                            let out_h = out_shape[1];
                            let out_w = out_shape[2];
                            let ks = *kernel;
                            let st = *stride;
                            let pool_area = (ks * ks) as f64;
                            let mut dinput = vec![0.0f64; n * in_h * in_w * c];
                            let grad_per_element = 1.0 / pool_area;
                            for nb in 0..n {
                                for oh in 0..out_h {
                                    for ow in 0..out_w {
                                        for ch in 0..c {
                                            let out_idx = nb*out_h*out_w*c + oh*out_w*c + ow*c + ch;
                                            let d = out_grad[out_idx] * grad_per_element;
                                            for kh in 0..ks {
                                                for kw in 0..ks {
                                                    let ih = oh * st + kh;
                                                    let iw = ow * st + kw;
                                                    if ih < in_h && iw < in_w {
                                                        dinput[nb*in_h*in_w*c + ih*in_w*c + iw*c + ch] += d;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Self::accum_grad(&mut self.ad_grads, *in_id, dinput);
                        }
                        TapeOp::Gru { x_id, wx_id, wh_id, b_id, h0_id,
                                      x_data, wx_data, wh_data, h_all,
                                      gates_r, gates_z, gates_n,
                                      seq_len, input_size, hidden_size } => {
                            let (sl, is, hs) = (*seq_len, *input_size, *hidden_size);
                            let three_h = 3 * hs;
                            let mut dx  = vec![0.0f64; sl * is];
                            let mut dwx = vec![0.0f64; is * three_h];
                            let mut dwh = vec![0.0f64; hs * three_h];
                            let mut db  = vec![0.0f64; three_h];
                            let mut dh_next = vec![0.0f64; hs];
                            for t in (0..sl).rev() {
                                let mut dh = dh_next.clone();
                                if t == sl - 1 {
                                    for j in 0..hs { dh[j] += out_grad[j]; }
                                }
                                let r_t   = &gates_r[t*hs..(t+1)*hs];
                                let z_t   = &gates_z[t*hs..(t+1)*hs];
                                let n_t   = &gates_n[t*hs..(t+1)*hs];
                                let h_prev = &h_all[t*hs..(t+1)*hs];
                                let x_t   = &x_data[t*is..(t+1)*is];
                                // dh_t = (1-z)*h_prev + z*n  → derive
                                // dh_prev from direct path: dh * (1-z)
                                // dn_t = dh * z
                                // dz_t = dh * (n - h_prev)
                                let dn: Vec<f64> = (0..hs).map(|j| dh[j] * z_t[j]).collect();
                                let dz_gate: Vec<f64> = (0..hs).map(|j| dh[j] * (n_t[j] - h_prev[j])).collect();
                                let mut dh_prev: Vec<f64> = (0..hs).map(|j| dh[j] * (1.0 - z_t[j])).collect();
                                // n_t = tanh(x@Wx_n + (r*h_prev)@Wh_n + b_n)
                                let dz_n: Vec<f64> = (0..hs).map(|j| dn[j] * (1.0 - n_t[j]*n_t[j])).collect();
                                // d(r*h_prev) = dz_n @ Wh_n^T
                                let mut d_rh = vec![0.0f64; hs];
                                for k in 0..hs {
                                    for j in 0..hs {
                                        d_rh[k] += dz_n[j] * wh_data[k*three_h + 2*hs + j];
                                    }
                                }
                                // dr_t = d_rh * h_prev;  dh_prev += d_rh * r_t
                                let dr: Vec<f64> = (0..hs).map(|j| d_rh[j] * h_prev[j]).collect();
                                for j in 0..hs { dh_prev[j] += d_rh[j] * r_t[j]; }
                                // r_t sigmoid deriv
                                let dz_r: Vec<f64> = (0..hs).map(|j| dr[j] * r_t[j] * (1.0 - r_t[j])).collect();
                                // z_t sigmoid deriv
                                let dz_z: Vec<f64> = (0..hs).map(|j| dz_gate[j] * z_t[j] * (1.0 - z_t[j])).collect();
                                // Pack pre-activation gradients: [dz_r | dz_z | dz_n]
                                let mut dz = vec![0.0f64; three_h];
                                dz[..hs].copy_from_slice(&dz_r);
                                dz[hs..2*hs].copy_from_slice(&dz_z);
                                dz[2*hs..].copy_from_slice(&dz_n);
                                // dx_t = dz @ Wx^T (all gates contribute)
                                for k in 0..is {
                                    for j in 0..three_h {
                                        dx[t*is+k] += dz[j] * wx_data[k*three_h+j];
                                    }
                                }
                                for k in 0..is {
                                    for j in 0..three_h { dwx[k*three_h+j] += x_t[k] * dz[j]; }
                                }
                                // dWh for r,z gates: h_prev^T ⊗ dz[r,z]
                                // dWh for n gate: (r*h_prev)^T ⊗ dz_n
                                for k in 0..hs {
                                    for j in 0..2*hs { dwh[k*three_h+j] += h_prev[k] * dz[j]; }
                                    for j in 0..hs {
                                        dwh[k*three_h+2*hs+j] += r_t[k] * h_prev[k] * dz_n[j];
                                    }
                                }
                                for j in 0..three_h { db[j] += dz[j]; }
                                // dh_prev from r,z gate paths
                                let mut new_dh_next = dh_prev;
                                for k in 0..hs {
                                    for j in 0..2*hs {
                                        new_dh_next[k] += dz[j] * wh_data[k*three_h+j];
                                    }
                                }
                                dh_next = new_dh_next;
                            }
                            Self::accum_grad(&mut self.ad_grads, *x_id,  dx);
                            Self::accum_grad(&mut self.ad_grads, *wx_id, dwx);
                            Self::accum_grad(&mut self.ad_grads, *wh_id, dwh);
                            Self::accum_grad(&mut self.ad_grads, *b_id,  db);
                            Self::accum_grad(&mut self.ad_grads, *h0_id, dh_next);
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

            // ── Phase 1: Loss functions ───────────────────────────────────────
            "mseLoss" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.mseLoss(pred, target) requires 2 arguments");
                    return EvalResult::Error;
                }
                let pred_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let tgt_ref  = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (pred_data, pred_shape, pred_tid) = match self.resolve(pred_ref).cloned() {
                    Some(ObjectData::Tensor { data, shape, tid }) => (data, shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.mseLoss pred must be a Tensor"); return EvalResult::Error; }
                };
                let target_data = match self.resolve(tgt_ref).cloned() {
                    Some(ObjectData::Tensor { data, .. }) => data,
                    _ => { eprintln!("❌ ERROR: Autodiff.mseLoss target must be a Tensor"); return EvalResult::Error; }
                };
                if pred_data.len() != target_data.len() {
                    eprintln!("❌ ERROR: Autodiff.mseLoss pred and target must have same size");
                    return EvalResult::Error;
                }
                let n = pred_data.len();
                let loss: f64 = pred_data.iter().zip(target_data.iter())
                    .map(|(p, t)| (p - t).powi(2)).sum::<f64>() / n as f64;
                // Store (pred - target) diff for backward
                let diff: Vec<f64> = pred_data.iter().zip(target_data.iter())
                    .map(|(p, t)| p - t).collect();
                let out_ref = self.alloc_tensor(vec![1], vec![loss]);
                if self.ad_recording {
                    let pred_id = self.ad_tensor_id_from_tid(pred_tid);
                    self.ad_push(out_ref, TapeOp::MseLoss { pred_id, target_data: diff, n });
                }
                EvalResult::Value(out_ref)
            }

            "maeLoss" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.maeLoss(pred, target) requires 2 arguments");
                    return EvalResult::Error;
                }
                let pred_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let tgt_ref  = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (pred_data, _pred_shape, pred_tid) = match self.resolve(pred_ref).cloned() {
                    Some(ObjectData::Tensor { data, shape, tid }) => (data, shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.maeLoss pred must be a Tensor"); return EvalResult::Error; }
                };
                let target_data = match self.resolve(tgt_ref).cloned() {
                    Some(ObjectData::Tensor { data, .. }) => data,
                    _ => { eprintln!("❌ ERROR: Autodiff.maeLoss target must be a Tensor"); return EvalResult::Error; }
                };
                let n = pred_data.len();
                let loss: f64 = pred_data.iter().zip(target_data.iter())
                    .map(|(p, t)| (p - t).abs()).sum::<f64>() / n as f64;
                let signs: Vec<f64> = pred_data.iter().zip(target_data.iter())
                    .map(|(p, t)| if p > t { 1.0 } else if p < t { -1.0 } else { 0.0 }).collect();
                let out_ref = self.alloc_tensor(vec![1], vec![loss]);
                if self.ad_recording {
                    let pred_id = self.ad_tensor_id_from_tid(pred_tid);
                    self.ad_push(out_ref, TapeOp::MaeLoss { pred_id, signs, n });
                }
                EvalResult::Value(out_ref)
            }

            "bceLoss" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.bceLoss(pred, target) requires 2 arguments");
                    return EvalResult::Error;
                }
                let pred_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let tgt_ref  = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (pred_data, _ps, pred_tid) = match self.resolve(pred_ref).cloned() {
                    Some(ObjectData::Tensor { data, shape, tid }) => (data, shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.bceLoss pred must be a Tensor"); return EvalResult::Error; }
                };
                let target_data = match self.resolve(tgt_ref).cloned() {
                    Some(ObjectData::Tensor { data, .. }) => data,
                    _ => { eprintln!("❌ ERROR: Autodiff.bceLoss target must be a Tensor"); return EvalResult::Error; }
                };
                let n = pred_data.len();
                let eps = 1e-12_f64;
                let loss: f64 = pred_data.iter().zip(target_data.iter())
                    .map(|(&p, &t)| {
                        let p = p.clamp(eps, 1.0 - eps);
                        -(t * p.ln() + (1.0 - t) * (1.0 - p).ln())
                    }).sum::<f64>() / n as f64;
                let out_ref = self.alloc_tensor(vec![1], vec![loss]);
                if self.ad_recording {
                    let pred_id = self.ad_tensor_id_from_tid(pred_tid);
                    self.ad_push(out_ref, TapeOp::BceLoss { pred_id, pred_data, target_data, n });
                }
                EvalResult::Value(out_ref)
            }

            "crossEntropyLoss" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.crossEntropyLoss(logits, targets) requires 2 arguments");
                    return EvalResult::Error;
                }
                let logits_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let tgt_ref    = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (logits_data, logits_shape, logits_tid) = match self.resolve(logits_ref).cloned() {
                    Some(ObjectData::Tensor { data, shape, tid }) => (data, shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.crossEntropyLoss logits must be a Tensor"); return EvalResult::Error; }
                };
                // targets: either [batch] integer tensor or [batch, classes] one-hot
                let target_data = match self.resolve(tgt_ref).cloned() {
                    Some(ObjectData::Tensor { data, .. }) => data,
                    Some(ObjectData::Array { elements, .. }) => {
                        elements.iter().filter_map(|elem| {
                            match elem {
                                crate::region::OwnedValue::Integer(v) => Some(*v as f64),
                                crate::region::OwnedValue::Decimal(d) => Some(*d),
                                _ => None,
                            }
                        }).collect()
                    }
                    _ => { eprintln!("❌ ERROR: Autodiff.crossEntropyLoss targets must be a Tensor or Array"); return EvalResult::Error; }
                };
                let (batch, classes) = if logits_shape.len() == 2 {
                    (logits_shape[0], logits_shape[1])
                } else {
                    eprintln!("❌ ERROR: Autodiff.crossEntropyLoss logits must be 2D [batch, classes]");
                    return EvalResult::Error;
                };
                if target_data.len() != batch {
                    eprintln!("❌ ERROR: Autodiff.crossEntropyLoss targets length {} != batch {}", target_data.len(), batch);
                    return EvalResult::Error;
                }
                // Compute softmax probabilities per row (numerically stable)
                let mut probs = vec![0.0f64; batch * classes];
                for b in 0..batch {
                    let row = &logits_data[b*classes..(b+1)*classes];
                    let max_v = row.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let exps: Vec<f64> = row.iter().map(|&x| (x - max_v).exp()).collect();
                    let sum_exp: f64 = exps.iter().sum();
                    for c in 0..classes { probs[b*classes+c] = exps[c] / sum_exp; }
                }
                let eps = 1e-12_f64;
                let target_indices: Vec<usize> = target_data.iter()
                    .map(|&t| t.round() as usize).collect();
                let loss: f64 = (0..batch).map(|b| {
                    let idx = target_indices[b].min(classes - 1);
                    -(probs[b*classes+idx].max(eps)).ln()
                }).sum::<f64>() / batch as f64;
                let out_ref = self.alloc_tensor(vec![1], vec![loss]);
                if self.ad_recording {
                    let logits_id = self.ad_tensor_id_from_tid(logits_tid);
                    self.ad_push(out_ref, TapeOp::CrossEntropyLoss { logits_id, probs, target_indices, batch, classes });
                }
                EvalResult::Value(out_ref)
            }

            // ── Phase 1: Gradient clipping ────────────────────────────────────
            "clipGrad" => {
                if dot_call.arguments.len() < 2 {
                    eprintln!("❌ ERROR: Autodiff.clipGrad(grad, max_norm) requires 2 arguments");
                    return EvalResult::Error;
                }
                let grad_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let max_norm_ref = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (grad_data, grad_shape) = match self.resolve(grad_ref).cloned() {
                    Some(ObjectData::Tensor { data, shape, .. }) => (data, shape),
                    _ => { eprintln!("❌ ERROR: Autodiff.clipGrad grad must be a Tensor"); return EvalResult::Error; }
                };
                let max_norm = match self.resolve(max_norm_ref) {
                    Some(ObjectData::Decimal(v)) => *v,
                    Some(ObjectData::Integer(v)) => *v as f64,
                    _ => { eprintln!("❌ ERROR: Autodiff.clipGrad max_norm must be a number"); return EvalResult::Error; }
                };
                let norm: f64 = grad_data.iter().map(|x| x * x).sum::<f64>().sqrt();
                let clipped: Vec<f64> = if norm > max_norm {
                    let scale = max_norm / norm;
                    grad_data.iter().map(|x| x * scale).collect()
                } else {
                    grad_data
                };
                EvalResult::Value(self.alloc_tensor(grad_shape, clipped))
            }

            "clipGradNorm" => {
                // clipGradNorm(grads_array, max_norm) — clips a collection of gradients by global norm
                if dot_call.arguments.len() < 2 {
                    eprintln!("❌ ERROR: Autodiff.clipGradNorm(grads, max_norm) requires 2 arguments");
                    return EvalResult::Error;
                }
                let grads_ref  = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let max_norm_r = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let max_norm = match self.resolve(max_norm_r) {
                    Some(ObjectData::Decimal(v)) => *v,
                    Some(ObjectData::Integer(v)) => *v as f64,
                    _ => { eprintln!("❌ ERROR: Autodiff.clipGradNorm max_norm must be a number"); return EvalResult::Error; }
                };
                let elements = match self.resolve(grads_ref).cloned() {
                    Some(ObjectData::Array { elements, .. }) => elements,
                    _ => { eprintln!("❌ ERROR: Autodiff.clipGradNorm grads must be an Array of Tensors"); return EvalResult::Error; }
                };
                // Compute global norm
                let mut global_norm_sq = 0.0_f64;
                let mut grads_data: Vec<(Vec<f64>, Vec<usize>)> = Vec::new();
                for elem in &elements {
                    match elem {
                        crate::region::OwnedValue::Tensor { data, shape, .. } => {
                            global_norm_sq += data.iter().map(|x| x * x).sum::<f64>();
                            grads_data.push((data.clone(), shape.clone()));
                        }
                        _ => { eprintln!("❌ ERROR: Autodiff.clipGradNorm all elements must be Tensors"); return EvalResult::Error; }
                    }
                }
                let global_norm = global_norm_sq.sqrt();
                let scale = if global_norm > max_norm { max_norm / global_norm } else { 1.0 };
                let mut result_owned: Vec<crate::region::OwnedValue> = Vec::new();
                for (data, shape) in grads_data {
                    let clipped: Vec<f64> = data.iter().map(|x| x * scale).collect();
                    result_owned.push(crate::region::OwnedValue::Tensor { shape, data: clipped, tid: 0 });
                }
                let arr = ObjectData::Array { element_type: None, elements: result_owned };
                EvalResult::Value(self.alloc(arr))
            }

            // ── Phase 1: Weight initialization ───────────────────────────────
            "xavierUniform" => {
                if dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Autodiff.xavierUniform([shape]) requires shape argument");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let (fan_in, fan_out) = Self::compute_fans(&shape);
                let limit = (6.0_f64 / (fan_in + fan_out) as f64).sqrt();
                let total: usize = shape.iter().product();
                let data: Vec<f64> = (0..total).map(|_| {
                    let u = self.lcg_next_f64();
                    u * 2.0 * limit - limit
                }).collect();
                EvalResult::Value(self.alloc_tensor(shape, data))
            }

            "xavierNormal" => {
                if dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Autodiff.xavierNormal([shape]) requires shape argument");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let (fan_in, fan_out) = Self::compute_fans(&shape);
                let std = (2.0_f64 / (fan_in + fan_out) as f64).sqrt();
                let total: usize = shape.iter().product();
                let data: Vec<f64> = Self::box_muller_n(total, 0.0, std, &mut self.lcg_state);
                EvalResult::Value(self.alloc_tensor(shape, data))
            }

            "heUniform" => {
                if dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Autodiff.heUniform([shape]) requires shape argument");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let (fan_in, _fan_out) = Self::compute_fans(&shape);
                let limit = (6.0_f64 / fan_in as f64).sqrt();
                let total: usize = shape.iter().product();
                let data: Vec<f64> = (0..total).map(|_| {
                    let u = self.lcg_next_f64();
                    u * 2.0 * limit - limit
                }).collect();
                EvalResult::Value(self.alloc_tensor(shape, data))
            }

            "heNormal" => {
                if dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Autodiff.heNormal([shape]) requires shape argument");
                    return EvalResult::Error;
                }
                let shape = match self.eval_shape_expr(&dot_call.arguments[0]) {
                    Ok(s) => s, Err(e) => return e,
                };
                let (fan_in, _fan_out) = Self::compute_fans(&shape);
                let std = (2.0_f64 / fan_in as f64).sqrt();
                let total: usize = shape.iter().product();
                let data: Vec<f64> = Self::box_muller_n(total, 0.0, std, &mut self.lcg_state);
                EvalResult::Value(self.alloc_tensor(shape, data))
            }

            // ── Phase 1: Optimizer steps (pure functions, no tape) ────────────
            "adamStep" => {
                // adamStep(param, grad, m, v, step, lr, beta1=0.9, beta2=0.999, eps=1e-8)
                // Returns Array [new_param, new_m, new_v]
                if dot_call.arguments.len() < 6 {
                    eprintln!("❌ ERROR: Autodiff.adamStep(param, grad, m, v, step, lr, [beta1, beta2, eps])");
                    return EvalResult::Error;
                }
                let mut args = Vec::new();
                for arg in &dot_call.arguments {
                    match self.eval_expression(arg) {
                        EvalResult::Value(r) => args.push(r),
                        other => return other,
                    }
                }
                let (param, p_shape) = match self.resolve(args[0]).cloned() {
                    Some(ObjectData::Tensor { data, shape, .. }) => (data, shape),
                    _ => { eprintln!("❌ ERROR: Autodiff.adamStep param must be a Tensor"); return EvalResult::Error; }
                };
                let (grad, _) = match self.resolve(args[1]).cloned() {
                    Some(ObjectData::Tensor { data, shape, .. }) => (data, shape),
                    _ => { eprintln!("❌ ERROR: Autodiff.adamStep grad must be a Tensor"); return EvalResult::Error; }
                };
                let (m_data, _) = match self.resolve(args[2]).cloned() {
                    Some(ObjectData::Tensor { data, shape, .. }) => (data, shape),
                    _ => { eprintln!("❌ ERROR: Autodiff.adamStep m must be a Tensor"); return EvalResult::Error; }
                };
                let (v_data, _) = match self.resolve(args[3]).cloned() {
                    Some(ObjectData::Tensor { data, shape, .. }) => (data, shape),
                    _ => { eprintln!("❌ ERROR: Autodiff.adamStep v must be a Tensor"); return EvalResult::Error; }
                };
                let step = match self.resolve(args[4]) {
                    Some(ObjectData::Integer(n)) => *n as f64,
                    Some(ObjectData::Decimal(d)) => *d,
                    _ => { eprintln!("❌ ERROR: Autodiff.adamStep step must be a number"); return EvalResult::Error; }
                };
                let lr = match self.resolve(args[5]) {
                    Some(ObjectData::Decimal(d)) => *d,
                    Some(ObjectData::Integer(n)) => *n as f64,
                    _ => { eprintln!("❌ ERROR: Autodiff.adamStep lr must be a number"); return EvalResult::Error; }
                };
                let beta1 = if args.len() > 6 { match self.resolve(args[6]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => 0.9 } } else { 0.9 };
                let beta2 = if args.len() > 7 { match self.resolve(args[7]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => 0.999 } } else { 0.999 };
                let eps   = if args.len() > 8 { match self.resolve(args[8]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => 1e-8 } } else { 1e-8 };
                let t = step;
                let bc1 = 1.0 - beta1.powf(t);
                let bc2 = 1.0 - beta2.powf(t);
                let mut new_m = vec![0.0f64; param.len()];
                let mut new_v = vec![0.0f64; param.len()];
                let mut new_p = vec![0.0f64; param.len()];
                for i in 0..param.len() {
                    new_m[i] = beta1 * m_data[i] + (1.0 - beta1) * grad[i];
                    new_v[i] = beta2 * v_data[i] + (1.0 - beta2) * grad[i] * grad[i];
                    let m_hat = new_m[i] / bc1;
                    let v_hat = new_v[i] / bc2;
                    new_p[i] = param[i] - lr * m_hat / (v_hat.sqrt() + eps);
                }
                let r_p = self.alloc_tensor(p_shape.clone(), new_p);
                let r_m = self.alloc_tensor(p_shape.clone(), new_m);
                let r_v = self.alloc_tensor(p_shape,          new_v);
                let op = self.extract(r_p);
                let om = self.extract(r_m);
                let ov = self.extract(r_v);
                let arr = ObjectData::Array { element_type: None, elements: vec![op, om, ov] };
                EvalResult::Value(self.alloc(arr))
            }

            "adamwStep" => {
                // adamwStep(param, grad, m, v, step, lr, wd=0.01, beta1=0.9, beta2=0.999, eps=1e-8)
                if dot_call.arguments.len() < 6 {
                    eprintln!("❌ ERROR: Autodiff.adamwStep(param, grad, m, v, step, lr, [wd, beta1, beta2, eps])");
                    return EvalResult::Error;
                }
                let mut args = Vec::new();
                for arg in &dot_call.arguments {
                    match self.eval_expression(arg) { EvalResult::Value(r) => args.push(r), other => return other }
                }
                let (param, p_shape) = match self.resolve(args[0]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: Autodiff.adamwStep param must be Tensor"); return EvalResult::Error; } };
                let (grad, _)   = match self.resolve(args[1]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: Autodiff.adamwStep grad must be Tensor"); return EvalResult::Error; } };
                let (m_data, _) = match self.resolve(args[2]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: Autodiff.adamwStep m must be Tensor"); return EvalResult::Error; } };
                let (v_data, _) = match self.resolve(args[3]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: Autodiff.adamwStep v must be Tensor"); return EvalResult::Error; } };
                let step = match self.resolve(args[4]) { Some(ObjectData::Integer(n)) => *n as f64, Some(ObjectData::Decimal(d)) => *d, _ => { eprintln!("❌ ERROR: step must be number"); return EvalResult::Error; } };
                let lr   = match self.resolve(args[5]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => { eprintln!("❌ ERROR: lr must be number"); return EvalResult::Error; } };
                let wd    = if args.len() > 6 { match self.resolve(args[6]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => 0.01 } } else { 0.01 };
                let beta1 = if args.len() > 7 { match self.resolve(args[7]) { Some(ObjectData::Decimal(d)) => *d, _ => 0.9 } } else { 0.9 };
                let beta2 = if args.len() > 8 { match self.resolve(args[8]) { Some(ObjectData::Decimal(d)) => *d, _ => 0.999 } } else { 0.999 };
                let eps   = if args.len() > 9 { match self.resolve(args[9]) { Some(ObjectData::Decimal(d)) => *d, _ => 1e-8 } } else { 1e-8 };
                let bc1 = 1.0 - beta1.powf(step);
                let bc2 = 1.0 - beta2.powf(step);
                let mut new_m = vec![0.0f64; param.len()];
                let mut new_v = vec![0.0f64; param.len()];
                let mut new_p = vec![0.0f64; param.len()];
                for i in 0..param.len() {
                    new_m[i] = beta1 * m_data[i] + (1.0 - beta1) * grad[i];
                    new_v[i] = beta2 * v_data[i] + (1.0 - beta2) * grad[i] * grad[i];
                    let m_hat = new_m[i] / bc1;
                    let v_hat = new_v[i] / bc2;
                    new_p[i] = param[i] * (1.0 - lr * wd) - lr * m_hat / (v_hat.sqrt() + eps);
                }
                let r_p = self.alloc_tensor(p_shape.clone(), new_p);
                let r_m = self.alloc_tensor(p_shape.clone(), new_m);
                let r_v = self.alloc_tensor(p_shape,          new_v);
                let op = self.extract(r_p); let om = self.extract(r_m); let ov = self.extract(r_v);
                let arr = ObjectData::Array { element_type: None, elements: vec![op, om, ov] };
                EvalResult::Value(self.alloc(arr))
            }

            "sgdStep" => {
                // sgdStep(param, grad, velocity, lr, momentum=0.9, weight_decay=0.0)
                // Returns [new_param, new_velocity]
                if dot_call.arguments.len() < 4 {
                    eprintln!("❌ ERROR: Autodiff.sgdStep(param, grad, velocity, lr, [momentum, weight_decay])");
                    return EvalResult::Error;
                }
                let mut args = Vec::new();
                for arg in &dot_call.arguments { match self.eval_expression(arg) { EvalResult::Value(r) => args.push(r), other => return other } }
                let (param, p_shape) = match self.resolve(args[0]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: param must be Tensor"); return EvalResult::Error; } };
                let (mut grad, _) = match self.resolve(args[1]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: grad must be Tensor"); return EvalResult::Error; } };
                let (vel, _) = match self.resolve(args[2]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: velocity must be Tensor"); return EvalResult::Error; } };
                let lr = match self.resolve(args[3]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => { eprintln!("❌ ERROR: lr must be number"); return EvalResult::Error; } };
                let momentum = if args.len() > 4 { match self.resolve(args[4]) { Some(ObjectData::Decimal(d)) => *d, _ => 0.9 } } else { 0.9 };
                let wd = if args.len() > 5 { match self.resolve(args[5]) { Some(ObjectData::Decimal(d)) => *d, _ => 0.0 } } else { 0.0 };
                if wd > 0.0 { for i in 0..grad.len() { grad[i] += wd * param[i]; } }
                let mut new_vel = vec![0.0f64; param.len()];
                let mut new_p   = vec![0.0f64; param.len()];
                for i in 0..param.len() {
                    new_vel[i] = momentum * vel[i] - lr * grad[i];
                    new_p[i]   = param[i] + new_vel[i];
                }
                let r_p = self.alloc_tensor(p_shape.clone(), new_p);
                let r_v = self.alloc_tensor(p_shape,          new_vel);
                let op = self.extract(r_p); let ov = self.extract(r_v);
                let arr = ObjectData::Array { element_type: None, elements: vec![op, ov] };
                EvalResult::Value(self.alloc(arr))
            }

            "rmspropStep" => {
                // rmspropStep(param, grad, sq_avg, lr, alpha=0.99, eps=1e-8)
                // Returns [new_param, new_sq_avg]
                if dot_call.arguments.len() < 4 {
                    eprintln!("❌ ERROR: Autodiff.rmspropStep(param, grad, sq_avg, lr, [alpha, eps])");
                    return EvalResult::Error;
                }
                let mut args = Vec::new();
                for arg in &dot_call.arguments { match self.eval_expression(arg) { EvalResult::Value(r) => args.push(r), other => return other } }
                let (param, p_shape) = match self.resolve(args[0]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: param must be Tensor"); return EvalResult::Error; } };
                let (grad, _) = match self.resolve(args[1]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: grad must be Tensor"); return EvalResult::Error; } };
                let (sq_avg, _) = match self.resolve(args[2]).cloned() { Some(ObjectData::Tensor { data, shape, .. }) => (data, shape), _ => { eprintln!("❌ ERROR: sq_avg must be Tensor"); return EvalResult::Error; } };
                let lr = match self.resolve(args[3]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => { eprintln!("❌ ERROR: lr must be number"); return EvalResult::Error; } };
                let alpha = if args.len() > 4 { match self.resolve(args[4]) { Some(ObjectData::Decimal(d)) => *d, _ => 0.99 } } else { 0.99 };
                let eps   = if args.len() > 5 { match self.resolve(args[5]) { Some(ObjectData::Decimal(d)) => *d, _ => 1e-8 } } else { 1e-8 };
                let mut new_sq = vec![0.0f64; param.len()];
                let mut new_p  = vec![0.0f64; param.len()];
                for i in 0..param.len() {
                    new_sq[i] = alpha * sq_avg[i] + (1.0 - alpha) * grad[i] * grad[i];
                    new_p[i]  = param[i] - lr * grad[i] / (new_sq[i].sqrt() + eps);
                }
                let r_p = self.alloc_tensor(p_shape.clone(), new_p);
                let r_s = self.alloc_tensor(p_shape,          new_sq);
                let op = self.extract(r_p); let os = self.extract(r_s);
                let arr = ObjectData::Array { element_type: None, elements: vec![op, os] };
                EvalResult::Value(self.alloc(arr))
            }

            // ── Phase 2: BatchNorm ────────────────────────────────────────────
            "batchNorm" => {
                // batchNorm(x, gamma, beta, training, eps=1e-5)
                // x: [N, C] tensor; gamma, beta: [C] tensors
                // Returns normalized tensor [N, C]
                if dot_call.arguments.len() < 4 {
                    eprintln!("❌ ERROR: Autodiff.batchNorm(x, gamma, beta, training, [eps])");
                    return EvalResult::Error;
                }
                let mut args = Vec::new();
                for arg in &dot_call.arguments { match self.eval_expression(arg) { EvalResult::Value(r) => args.push(r), other => return other } }
                let (x_data, x_shape, x_tid) = match self.resolve(args[0]).cloned() {
                    Some(ObjectData::Tensor { data, shape, tid }) => (data, shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.batchNorm x must be a Tensor"); return EvalResult::Error; }
                };
                let (g_data, g_tid) = match self.resolve(args[1]).cloned() {
                    Some(ObjectData::Tensor { data, tid, .. }) => (data, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.batchNorm gamma must be a Tensor"); return EvalResult::Error; }
                };
                let (b_data, b_tid) = match self.resolve(args[2]).cloned() {
                    Some(ObjectData::Tensor { data, tid, .. }) => (data, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.batchNorm beta must be a Tensor"); return EvalResult::Error; }
                };
                let training = match self.resolve(args[3]) {
                    Some(ObjectData::Boolean(b)) => *b,
                    _ => true,
                };
                let eps_val = if args.len() > 4 {
                    match self.resolve(args[4]) { Some(ObjectData::Decimal(d)) => *d, Some(ObjectData::Integer(n)) => *n as f64, _ => 1e-5 }
                } else { 1e-5 };
                if x_shape.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.batchNorm x must be 2D [N, C]");
                    return EvalResult::Error;
                }
                let (n, c) = (x_shape[0], x_shape[1]);
                let nf = n as f64;
                // Compute per-feature mean and variance
                let mut means = vec![0.0_f64; c];
                let mut vars  = vec![0.0_f64; c];
                for j in 0..c {
                    let mut s = 0.0_f64;
                    for i in 0..n { s += x_data[i*c+j]; }
                    means[j] = s / nf;
                    let mut v = 0.0_f64;
                    for i in 0..n { let d = x_data[i*c+j] - means[j]; v += d*d; }
                    vars[j] = v / nf;
                }
                let stds: Vec<f64> = vars.iter().map(|&v| (v + eps_val).sqrt()).collect();
                let mut x_norm = vec![0.0_f64; n * c];
                let mut x_mu   = vec![0.0_f64; n * c];
                let mut out_data = vec![0.0_f64; n * c];
                for i in 0..n {
                    for j in 0..c {
                        let xm = x_data[i*c+j] - means[j];
                        x_mu[i*c+j]   = xm;
                        x_norm[i*c+j] = xm / stds[j];
                        out_data[i*c+j] = if training {
                            x_norm[i*c+j] * g_data[j] + b_data[j]
                        } else {
                            x_norm[i*c+j] * g_data[j] + b_data[j]
                        };
                    }
                }
                let out_ref = self.alloc_tensor(x_shape.clone(), out_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id_from_tid(x_tid);
                    let g_id  = self.ad_tensor_id_from_tid(g_tid);
                    let b_id2 = self.ad_tensor_id_from_tid(b_tid);
                    self.ad_push(out_ref, TapeOp::BatchNorm {
                        in_id, g_id, b_id: b_id2, eps: eps_val,
                        x_norm, stds, x_mu, gamma_data: g_data,
                        rows: n, cols: c,
                    });
                }
                EvalResult::Value(out_ref)
            }

            // ── Phase 2: Dropout ──────────────────────────────────────────────
            "dropout" => {
                // dropout(x, p, training)
                // p = drop probability (0 = no drop, 1 = drop all)
                if dot_call.arguments.len() < 2 {
                    eprintln!("❌ ERROR: Autodiff.dropout(x, p, [training=true])");
                    return EvalResult::Error;
                }
                let mut args = Vec::new();
                for arg in &dot_call.arguments { match self.eval_expression(arg) { EvalResult::Value(r) => args.push(r), other => return other } }
                let (x_data, x_shape, x_tid) = match self.resolve(args[0]).cloned() {
                    Some(ObjectData::Tensor { data, shape, tid }) => (data, shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.dropout x must be a Tensor"); return EvalResult::Error; }
                };
                let p = match self.resolve(args[1]) {
                    Some(ObjectData::Decimal(d)) => *d,
                    Some(ObjectData::Integer(n)) => *n as f64,
                    _ => { eprintln!("❌ ERROR: Autodiff.dropout p must be a number"); return EvalResult::Error; }
                };
                let training = if args.len() > 2 {
                    match self.resolve(args[2]) { Some(ObjectData::Boolean(b)) => *b, _ => true }
                } else { true };
                if !training || p <= 0.0 {
                    return EvalResult::Value(self.alloc_tensor(x_shape, x_data));
                }
                let keep_prob = 1.0 - p;
                let n = x_data.len();
                let mask: Vec<f64> = (0..n).map(|_| {
                    if self.lcg_next_f64() >= p { 1.0 } else { 0.0 }
                }).collect();
                let out_data: Vec<f64> = x_data.iter().zip(mask.iter())
                    .map(|(&x, &m)| x * m / keep_prob).collect();
                let out_ref = self.alloc_tensor(x_shape, out_data);
                if self.ad_recording {
                    let in_id = self.ad_tensor_id_from_tid(x_tid);
                    self.ad_push(out_ref, TapeOp::Dropout { in_id, mask, keep_prob });
                }
                EvalResult::Value(out_ref)
            }

            // ── Phase 2: Embedding ────────────────────────────────────────────
            "embedding" => {
                // embedding(indices, weight)
                // indices: Array of int or [seq_len] integer Tensor
                // weight:  [vocab_size, emb_dim] Tensor
                // Returns: [seq_len, emb_dim] Tensor
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.embedding(indices, weight) requires 2 arguments");
                    return EvalResult::Error;
                }
                let idx_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let w_ref   = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let (w_data, w_shape, w_tid) = match self.resolve(w_ref).cloned() {
                    Some(ObjectData::Tensor { data, shape, tid }) => (data, shape, tid),
                    _ => { eprintln!("❌ ERROR: Autodiff.embedding weight must be a Tensor [vocab, emb_dim]"); return EvalResult::Error; }
                };
                if w_shape.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.embedding weight must be 2D [vocab_size, emb_dim]");
                    return EvalResult::Error;
                }
                let (vocab_size, emb_dim) = (w_shape[0], w_shape[1]);
                let indices: Vec<usize> = match self.resolve(idx_ref).cloned() {
                    Some(ObjectData::Tensor { data, .. }) => data.iter().map(|&x| (x.round() as i64).max(0) as usize).collect(),
                    Some(ObjectData::Array { elements, .. }) => {
                        elements.iter().filter_map(|elem| {
                            match elem {
                                crate::region::OwnedValue::Integer(v) => Some((*v).max(0) as usize),
                                crate::region::OwnedValue::Decimal(d) => Some((d.round() as i64).max(0) as usize),
                                _ => None,
                            }
                        }).collect()
                    }
                    _ => { eprintln!("❌ ERROR: Autodiff.embedding indices must be a Tensor or Array"); return EvalResult::Error; }
                };
                let seq_len = indices.len();
                let mut out_data = vec![0.0_f64; seq_len * emb_dim];
                for (i, &idx) in indices.iter().enumerate() {
                    let row = idx.min(vocab_size - 1);
                    out_data[i*emb_dim..(i+1)*emb_dim]
                        .copy_from_slice(&w_data[row*emb_dim..(row+1)*emb_dim]);
                }
                let out_ref = self.alloc_tensor(vec![seq_len, emb_dim], out_data);
                if self.ad_recording {
                    let w_id = self.ad_tensor_id_from_tid(w_tid);
                    self.ad_push(out_ref, TapeOp::Embedding { w_id, indices, seq_len, emb_dim, vocab_size });
                }
                EvalResult::Value(out_ref)
            }

            // ── Weight persistence ────────────────────────────────────────────
            "saveWeights" => {
                // Autodiff.saveWeights(path, tensors_array)
                // Saves an array of tensors to a .szw binary file.
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Autodiff.saveWeights(path, tensors) requires 2 arguments");
                    return EvalResult::Error;
                }
                let path_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let arr_ref  = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, other => return other };
                let path = match self.resolve(path_ref).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: Autodiff.saveWeights path must be a string"); return EvalResult::Error; }
                };
                let elements = match self.resolve(arr_ref).cloned() {
                    Some(ObjectData::Array { elements, .. }) => elements,
                    _ => { eprintln!("❌ ERROR: Autodiff.saveWeights tensors must be an Array"); return EvalResult::Error; }
                };
                // Collect tensor data
                let mut tensors: Vec<(Vec<usize>, Vec<f64>)> = Vec::new();
                for elem in &elements {
                    match elem {
                        crate::region::OwnedValue::Tensor { shape, data, .. } => {
                            tensors.push((shape.clone(), data.clone()));
                        }
                        _ => { eprintln!("❌ ERROR: Autodiff.saveWeights all elements must be Tensors"); return EvalResult::Error; }
                    }
                }
                // Write binary file
                match Self::write_weights_file(&path, &tensors) {
                    Ok(()) => {
                        println!("✅ Saved {} tensors to '{}'", tensors.len(), path);
                        EvalResult::Value(self.null_ref)
                    }
                    Err(e) => {
                        eprintln!("❌ ERROR: Autodiff.saveWeights failed: {}", e);
                        EvalResult::Error
                    }
                }
            }

            "loadWeights" => {
                // Autodiff.loadWeights(path) → Array of Tensors
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Autodiff.loadWeights(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let path_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let path = match self.resolve(path_ref).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: Autodiff.loadWeights path must be a string"); return EvalResult::Error; }
                };
                match Self::read_weights_file(&path) {
                    Ok(tensors) => {
                        let n = tensors.len();
                        let owned: Vec<crate::region::OwnedValue> = tensors.into_iter()
                            .map(|(shape, data)| crate::region::OwnedValue::Tensor { shape, data, tid: 0 })
                            .collect();
                        let arr = ObjectData::Array { element_type: None, elements: owned };
                        println!("✅ Loaded {} tensors from '{}'", n, path);
                        EvalResult::Value(self.alloc(arr))
                    }
                    Err(e) => {
                        eprintln!("❌ ERROR: Autodiff.loadWeights failed: {}", e);
                        EvalResult::Error
                    }
                }
            }

            // ── Phase 3: stopGrad / detach ────────────────────────────────────
            "stopGrad" | "detach" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Autodiff.stopGrad(tensor) requires 1 argument");
                    return EvalResult::Error;
                }
                let t_ref = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, other => return other };
                let (shape, data) = match self.resolve(t_ref).cloned() {
                    Some(ObjectData::Tensor { shape, data, .. }) => (shape, data),
                    _ => { eprintln!("❌ ERROR: Autodiff.stopGrad() argument must be a Tensor"); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(ObjectData::Tensor { shape, data, tid: 0 }))
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

    /// Get or create a tape ID from a raw tensor `tid` (bypasses ObjectRef lookup).
    pub(super) fn ad_tensor_id_from_tid(&mut self, tid: u64) -> u64 {
        if let Some(&id) = self.ad_tensor_ids.get(&tid) {
            return id;
        }
        let id = self.ad_next_id;
        self.ad_next_id += 1;
        self.ad_tensor_ids.insert(tid, id);
        id
    }

    /// Box-Muller transform to produce `n` standard-normal samples.
    fn box_muller_n(n: usize, mean: f64, std_dev: f64, lcg: &mut u64) -> Vec<f64> {
        let step = |s: &mut u64| -> f64 {
            *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (*s >> 33) as f64 / (1u64 << 31) as f64
        };
        let mut out = Vec::with_capacity(n);
        let mut i = 0;
        while i < n {
            let u1 = (step(lcg) + 1e-10).min(1.0 - 1e-10);
            let u2 = step(lcg);
            let mag = std_dev * (-2.0 * u1.ln()).sqrt();
            let z0  = mag * (2.0 * std::f64::consts::PI * u2).cos() + mean;
            let z1  = mag * (2.0 * std::f64::consts::PI * u2).sin() + mean;
            out.push(z0);
            if i + 1 < n { out.push(z1); }
            i += 2;
        }
        out.truncate(n);
        out
    }

    /// Write tensors to a .szw binary file.
    /// Format: magic(4) + version(1) + count(u32 LE) + [ndim(u8) + shape(u64 LE × ndim) + len(u64 LE) + data(f64 LE × len)]*
    fn write_weights_file(path: &str, tensors: &[(Vec<usize>, Vec<f64>)]) -> Result<(), String> {
        use std::io::Write;
        let mut buf: Vec<u8> = Vec::new();
        // Magic + version
        buf.extend_from_slice(b"SZWT");
        buf.push(1u8);
        // Tensor count
        buf.extend_from_slice(&(tensors.len() as u32).to_le_bytes());
        // Each tensor
        for (shape, data) in tensors {
            buf.push(shape.len() as u8);
            for &dim in shape {
                buf.extend_from_slice(&(dim as u64).to_le_bytes());
            }
            buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
            for &v in data {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        std::fs::write(path, &buf).map_err(|e| format!("cannot write '{}': {}", path, e))
    }

    /// Read tensors from a .szw binary file.
    fn read_weights_file(path: &str) -> Result<Vec<(Vec<usize>, Vec<f64>)>, String> {
        let buf = std::fs::read(path)
            .map_err(|e| format!("cannot read '{}': {}", path, e))?;
        if buf.len() < 9 {
            return Err(format!("'{}' is too short to be a valid .szw file", path));
        }
        if &buf[..4] != b"SZWT" {
            return Err(format!("'{}' is not a valid .szw file (bad magic)", path));
        }
        let version = buf[4];
        if version != 1 {
            return Err(format!("unsupported .szw version {}", version));
        }
        let count = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]) as usize;
        let mut pos = 9usize;
        let mut tensors = Vec::with_capacity(count);
        for _ in 0..count {
            if pos >= buf.len() { return Err("unexpected end of file reading tensor header".to_string()); }
            let ndim = buf[pos] as usize;
            pos += 1;
            let mut shape = Vec::with_capacity(ndim);
            for _ in 0..ndim {
                if pos + 8 > buf.len() { return Err("unexpected end of file reading shape".to_string()); }
                let d = u64::from_le_bytes(buf[pos..pos+8].try_into().unwrap()) as usize;
                shape.push(d);
                pos += 8;
            }
            if pos + 8 > buf.len() { return Err("unexpected end of file reading data length".to_string()); }
            let data_len = u64::from_le_bytes(buf[pos..pos+8].try_into().unwrap()) as usize;
            pos += 8;
            if pos + data_len * 8 > buf.len() { return Err("unexpected end of file reading tensor data".to_string()); }
            let mut data = Vec::with_capacity(data_len);
            for i in 0..data_len {
                let v = f64::from_le_bytes(buf[pos + i*8..pos + i*8 + 8].try_into().unwrap());
                data.push(v);
            }
            pos += data_len * 8;
            tensors.push((shape, data));
        }
        Ok(tensors)
    }

    /// Compute fan_in and fan_out from a weight shape.
    /// For 2D: fan_in = shape[1], fan_out = shape[0]
    /// For 4D (conv): fan_in = shape[1]*kH*kW, fan_out = shape[0]*kH*kW
    fn compute_fans(shape: &[usize]) -> (usize, usize) {
        match shape.len() {
            0 | 1 => (shape.iter().product(), shape.iter().product()),
            2 => (shape[1], shape[0]),
            _ => {
                let receptive: usize = shape[2..].iter().product();
                (shape[1] * receptive, shape[0] * receptive)
            }
        }
    }
}
