use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;

impl super::Evaluator {
    pub(super) fn eval_memory_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            // Memory.sizeof(type_name) → int — size in bytes of the given type name
            "sizeof" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Memory.sizeof(type) requires 1 argument");
                }
                let type_name = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { return self.rt_err_kind("TypeError", "Memory.sizeof() argument must be a string type name"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let size: i64 = match type_name.as_str() {
                    "bool"    => 1,
                    "byte"    => 1,
                    "int8"    => 1,
                    "int16"   => 2,
                    "int32"   => 4,
                    "int64"   => 8,
                    "int"     => 8,
                    "uint8"   => 1,
                    "uint16"  => 2,
                    "uint32"  => 4,
                    "uint64"  => 8,
                    "float32" => 4,
                    "float64" => 8,
                    "decimal" => 8,
                    "ptr"     => 8,
                    "str"     => 8,
                    other => {
                        return self.rt_err_kind("MemoryError", format!("Memory.sizeof() — unknown type '{}'", other));
                    }
                };
                EvalResult::Value(self.alloc(ObjectData::Integer(size)))
            }

            // Memory.alloc(n) → int — allocate n bytes, return an opaque handle
            "alloc" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.alloc() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Memory.alloc(n) requires 1 argument");
                }
                let n = match self.eval_memory_usize(&dot_call.arguments[0]) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if n == 0 || n > 256 * 1024 * 1024 {
                    return self.rt_err_kind("TypeError", format!("Memory.alloc() size must be between 1 and 256 MiB, got {}", n));
                }
                let id = self.memory_heap_next_id;
                self.memory_heap_next_id += 1;
                self.memory_heap.insert(id, vec![0u8; n]);
                EvalResult::Value(self.alloc(ObjectData::Integer(id)))
            }

            // Memory.free(handle) — deallocate a raw allocation
            "free" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.free() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Memory.free(handle) requires 1 argument");
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if self.memory_heap.remove(&id).is_none() {
                    return self.rt_err_kind("MemoryError", format!("Memory.free() — no allocation with handle {}", id));
                }
                EvalResult::Value(self.null_ref)
            }

            // Memory.size(handle) → int — size in bytes of an existing allocation
            "size" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Memory.size(handle) requires 1 argument");
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                match self.memory_heap.get(&id) {
                    Some(buf) => EvalResult::Value(self.alloc(ObjectData::Integer(buf.len() as i64))),
                    None => {
                        self.rt_err_kind("MemoryError", format!("Memory.size() — no allocation with handle {}", id))
                    }
                }
            }

            // Memory.read(handle, offset, type) → value — read a typed value from raw memory
            "read" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.read() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Memory.read(handle, offset, type) requires 3 arguments");
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let offset = match self.eval_memory_usize(&dot_call.arguments[1]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let type_name = match self.eval_expression(&dot_call.arguments[2]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { return self.rt_err_kind("TypeError", "Memory.read() type must be a string"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let buf = match self.memory_heap.get(&id) {
                    Some(b) => b.clone(),
                    None => { return self.rt_err_kind("MemoryError", format!("Memory.read() — no allocation with handle {}", id)); }
                };
                match type_name.as_str() {
                    "bool" | "int8" | "byte" | "uint8" => {
                        if offset >= buf.len() { return self.rt_err_kind("MemoryError", format!("Memory.read() offset {} out of bounds (size {})", offset, buf.len())); }
                        EvalResult::Value(self.alloc(ObjectData::Integer(buf[offset] as i64)))
                    }
                    "int16" | "uint16" => {
                        if offset + 2 > buf.len() { return self.rt_err_kind("MemoryError", format!("Memory.read() offset {} out of bounds for int16 (size {})", offset, buf.len())); }
                        let v = i16::from_le_bytes([buf[offset], buf[offset+1]]);
                        EvalResult::Value(self.alloc(ObjectData::Integer(v as i64)))
                    }
                    "int32" | "uint32" => {
                        if offset + 4 > buf.len() { return self.rt_err_kind("MemoryError", format!("Memory.read() offset {} out of bounds for int32 (size {})", offset, buf.len())); }
                        let v = i32::from_le_bytes([buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]]);
                        EvalResult::Value(self.alloc(ObjectData::Integer(v as i64)))
                    }
                    "int64" | "int" | "uint64" => {
                        if offset + 8 > buf.len() { return self.rt_err_kind("MemoryError", format!("Memory.read() offset {} out of bounds for int64 (size {})", offset, buf.len())); }
                        let bytes: [u8; 8] = buf[offset..offset+8].try_into().unwrap();
                        let v = i64::from_le_bytes(bytes);
                        EvalResult::Value(self.alloc(ObjectData::Integer(v)))
                    }
                    "float32" => {
                        if offset + 4 > buf.len() { return self.rt_err_kind("MemoryError", format!("Memory.read() offset {} out of bounds for float32 (size {})", offset, buf.len())); }
                        let bits = u32::from_le_bytes([buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]]);
                        EvalResult::Value(self.alloc(ObjectData::Decimal(f32::from_bits(bits) as f64)))
                    }
                    "float64" | "decimal" => {
                        if offset + 8 > buf.len() { return self.rt_err_kind("MemoryError", format!("Memory.read() offset {} out of bounds for float64 (size {})", offset, buf.len())); }
                        let bytes: [u8; 8] = buf[offset..offset+8].try_into().unwrap();
                        let v = f64::from_le_bytes(bytes);
                        EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
                    }
                    other => { self.rt_err_kind("MemoryError", format!("Memory.read() — unknown type '{}'", other)) }
                }
            }

            // Memory.write(handle, offset, type, value) — write a typed value into raw memory
            "write" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.write() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 4 {
                    return self.rt_err_kind("TypeError", "Memory.write(handle, offset, type, value) requires 4 arguments");
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let offset = match self.eval_memory_usize(&dot_call.arguments[1]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let type_name = match self.eval_expression(&dot_call.arguments[2]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { return self.rt_err_kind("TypeError", "Memory.write() type must be a string"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let val_ref = match self.eval_expression(&dot_call.arguments[3]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let val_data = match self.resolve(val_ref).cloned() {
                    Some(d) => d,
                    None => { return self.rt_err_kind("MemoryError", "Memory.write() — null value"); }
                };
                let buf = match self.memory_heap.get_mut(&id) {
                    Some(b) => b,
                    None => { return self.rt_err_kind("MemoryError", format!("Memory.write() — no allocation with handle {}", id)); }
                };
                let write_result = Self::memory_write_typed(buf, offset, &type_name, &val_data);
                if let Err(msg) = write_result {
                    return self.rt_err_kind("MemoryError", format!("Memory.write() — {}", msg));
                }
                EvalResult::Value(self.null_ref)
            }

            // Memory.copy(src, dst, n) — copy n bytes from src allocation to dst allocation
            "copy" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.copy() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Memory.copy(src, dst, n) requires 3 arguments");
                }
                let src_id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let dst_id = match self.eval_memory_id(&dot_call.arguments[1]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let n = match self.eval_memory_usize(&dot_call.arguments[2]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let src_bytes: Vec<u8> = match self.memory_heap.get(&src_id) {
                    Some(b) => {
                        if n > b.len() {
                            return self.rt_err_kind("MemoryError", format!("Memory.copy() — src size {} < n {}", b.len(), n));
                        }
                        b[..n].to_vec()
                    }
                    None => { return self.rt_err_kind("MemoryError", format!("Memory.copy() — no src allocation {}", src_id)); }
                };
                // Validate size BEFORE taking the &mut borrow — rt_err_kind needs &mut self.
                let dst_len = match self.memory_heap.get(&dst_id) {
                    Some(b) => b.len(),
                    None => { return self.rt_err_kind("MemoryError", format!("Memory.copy() — no dst allocation {}", dst_id)); }
                };
                if n > dst_len {
                    return self.rt_err_kind("MemoryError", format!("Memory.copy() — dst size {} < n {}", dst_len, n));
                }
                if let Some(dst) = self.memory_heap.get_mut(&dst_id) {
                    dst[..n].copy_from_slice(&src_bytes);
                }
                EvalResult::Value(self.null_ref)
            }

            // Memory.fill(handle, value) — fill entire allocation with a byte value
            "fill" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.fill() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Memory.fill(handle, byte_value) requires 2 arguments");
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let byte_val = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(n)) => {
                            if n < 0 || n > 255 { return self.rt_err_kind("TypeError", "Memory.fill() byte value must be 0–255"); }
                            n as u8
                        }
                        _ => { return self.rt_err_kind("TypeError", "Memory.fill() byte value must be an integer 0–255"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.memory_heap.get_mut(&id) {
                    Some(buf) => { buf.iter_mut().for_each(|b| *b = byte_val); }
                    None => { return self.rt_err_kind("MemoryError", format!("Memory.fill() — no allocation with handle {}", id)); }
                }
                EvalResult::Value(self.null_ref)
            }

            // Memory.offsetOf(class_name, field_name) → int — simulated word-aligned field offset
            "offsetOf" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "Memory.offsetOf(class, field) requires 2 arguments");
                }
                let class_name = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { return self.rt_err_kind("TypeError", "Memory.offsetOf() class must be a string"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let field_name = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { return self.rt_err_kind("TypeError", "Memory.offsetOf() field must be a string"); }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let class = match self.class_registry.get(&class_name).cloned() {
                    Some(c) => c,
                    None => { return self.rt_err_kind("MemoryError", format!("Memory.offsetOf() — unknown class '{}'", class_name)); }
                };
                // Compute word-aligned offset for the named field
                let field_idx = class.fields.iter().position(|f| f.name == field_name);
                match field_idx {
                    Some(idx) => EvalResult::Value(self.alloc(ObjectData::Integer((idx * 8) as i64))),
                    None => { self.rt_err_kind("MemoryError", format!("Memory.offsetOf() — class '{}' has no field '{}'", class_name, field_name)) }
                }
            }

            other => {
                self.rt_err_kind("TypeError", format!("Unknown Memory method '{}'", other))
            }
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn eval_memory_id(&mut self, expr: &ast::Expression) -> Result<i64, EvalResult> {
        match self.eval_expression(expr) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Integer(n)) => Ok(n),
                _ => { eprintln!("❌ ERROR: Memory handle must be an integer"); Err(EvalResult::Error) }
            },
            EvalResult::Throw(v) => Err(EvalResult::Throw(v)),
            other => Err(other),
        }
    }

    fn eval_memory_usize(&mut self, expr: &ast::Expression) -> Result<usize, EvalResult> {
        match self.eval_expression(expr) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Integer(n)) if n >= 0 => Ok(n as usize),
                Some(ObjectData::Integer(n)) => { eprintln!("❌ ERROR: Memory size/offset cannot be negative, got {}", n); Err(EvalResult::Error) }
                _ => { eprintln!("❌ ERROR: Memory size/offset must be an integer"); Err(EvalResult::Error) }
            },
            EvalResult::Throw(v) => Err(EvalResult::Throw(v)),
            other => Err(other),
        }
    }

    fn memory_write_typed(buf: &mut Vec<u8>, offset: usize, type_name: &str, val: &ObjectData) -> Result<(), String> {
        let as_i64 = || match val {
            ObjectData::Integer(n) => Ok(*n),
            ObjectData::Decimal(d) => Ok(*d as i64),
            _ => Err("value must be a number".to_string()),
        };
        let as_f64 = || match val {
            ObjectData::Integer(n) => Ok(*n as f64),
            ObjectData::Decimal(d) => Ok(*d),
            _ => Err("value must be a number".to_string()),
        };
        match type_name {
            "bool" | "byte" | "int8" | "uint8" => {
                if offset >= buf.len() { return Err(format!("offset {} out of bounds (size {})", offset, buf.len())); }
                buf[offset] = as_i64()? as u8;
            }
            "int16" | "uint16" => {
                if offset + 2 > buf.len() { return Err(format!("offset {} out of bounds for int16 (size {})", offset, buf.len())); }
                let bytes = (as_i64()? as i16).to_le_bytes();
                buf[offset..offset+2].copy_from_slice(&bytes);
            }
            "int32" | "uint32" => {
                if offset + 4 > buf.len() { return Err(format!("offset {} out of bounds for int32 (size {})", offset, buf.len())); }
                let bytes = (as_i64()? as i32).to_le_bytes();
                buf[offset..offset+4].copy_from_slice(&bytes);
            }
            "int64" | "int" | "uint64" => {
                if offset + 8 > buf.len() { return Err(format!("offset {} out of bounds for int64 (size {})", offset, buf.len())); }
                let bytes = as_i64()?.to_le_bytes();
                buf[offset..offset+8].copy_from_slice(&bytes);
            }
            "float32" => {
                if offset + 4 > buf.len() { return Err(format!("offset {} out of bounds for float32 (size {})", offset, buf.len())); }
                let bytes = (as_f64()? as f32).to_bits().to_le_bytes();
                buf[offset..offset+4].copy_from_slice(&bytes);
            }
            "float64" | "decimal" => {
                if offset + 8 > buf.len() { return Err(format!("offset {} out of bounds for float64 (size {})", offset, buf.len())); }
                let bytes = as_f64()?.to_le_bytes();
                buf[offset..offset+8].copy_from_slice(&bytes);
            }
            other => return Err(format!("unknown type '{}'", other)),
        }
        Ok(())
    }
}
