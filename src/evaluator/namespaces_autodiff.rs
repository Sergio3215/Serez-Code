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
                        TapeOp::LayerNorm { in_id, g_id, b_id, eps, x_norm, stds, x_mu, gamma_data, rows, cols } => {
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
