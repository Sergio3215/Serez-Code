use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;

impl super::Evaluator {
    pub(super) fn eval_memory_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            // Memory.sizeof(type_name) → int — size in bytes of the given type name
            "sizeof" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Memory.sizeof(type) requires 1 argument");
                    return EvalResult::Error;
                }
                let type_name = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { eprintln!("❌ ERROR: Memory.sizeof() argument must be a string type name"); return EvalResult::Error; }
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
                        eprintln!("❌ ERROR: Memory.sizeof() — unknown type '{}'", other);
                        return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Memory.alloc(n) requires 1 argument");
                    return EvalResult::Error;
                }
                let n = match self.eval_memory_usize(&dot_call.arguments[0]) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if n == 0 || n > 256 * 1024 * 1024 {
                    eprintln!("❌ ERROR: Memory.alloc() size must be between 1 and 256 MiB, got {}", n);
                    return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Memory.free(handle) requires 1 argument");
                    return EvalResult::Error;
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                if self.memory_heap.remove(&id).is_none() {
                    eprintln!("❌ ERROR: Memory.free() — no allocation with handle {}", id);
                    return EvalResult::Error;
                }
                EvalResult::Value(self.null_ref)
            }

            // Memory.size(handle) → int — size in bytes of an existing allocation
            "size" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Memory.size(handle) requires 1 argument");
                    return EvalResult::Error;
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                match self.memory_heap.get(&id) {
                    Some(buf) => EvalResult::Value(self.alloc(ObjectData::Integer(buf.len() as i64))),
                    None => {
                        eprintln!("❌ ERROR: Memory.size() — no allocation with handle {}", id);
                        EvalResult::Error
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
                    eprintln!("❌ ERROR: Memory.read(handle, offset, type) requires 3 arguments");
                    return EvalResult::Error;
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
                        _ => { eprintln!("❌ ERROR: Memory.read() type must be a string"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let buf = match self.memory_heap.get(&id) {
                    Some(b) => b.clone(),
                    None => { eprintln!("❌ ERROR: Memory.read() — no allocation with handle {}", id); return EvalResult::Error; }
                };
                match type_name.as_str() {
                    "bool" | "int8" | "byte" | "uint8" => {
                        if offset >= buf.len() { eprintln!("❌ ERROR: Memory.read() offset {} out of bounds (size {})", offset, buf.len()); return EvalResult::Error; }
                        EvalResult::Value(self.alloc(ObjectData::Integer(buf[offset] as i64)))
                    }
                    "int16" | "uint16" => {
                        if offset + 2 > buf.len() { eprintln!("❌ ERROR: Memory.read() offset {} out of bounds for int16 (size {})", offset, buf.len()); return EvalResult::Error; }
                        let v = i16::from_le_bytes([buf[offset], buf[offset+1]]);
                        EvalResult::Value(self.alloc(ObjectData::Integer(v as i64)))
                    }
                    "int32" | "uint32" => {
                        if offset + 4 > buf.len() { eprintln!("❌ ERROR: Memory.read() offset {} out of bounds for int32 (size {})", offset, buf.len()); return EvalResult::Error; }
                        let v = i32::from_le_bytes([buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]]);
                        EvalResult::Value(self.alloc(ObjectData::Integer(v as i64)))
                    }
                    "int64" | "int" | "uint64" => {
                        if offset + 8 > buf.len() { eprintln!("❌ ERROR: Memory.read() offset {} out of bounds for int64 (size {})", offset, buf.len()); return EvalResult::Error; }
                        let bytes: [u8; 8] = buf[offset..offset+8].try_into().unwrap();
                        let v = i64::from_le_bytes(bytes);
                        EvalResult::Value(self.alloc(ObjectData::Integer(v)))
                    }
                    "float32" => {
                        if offset + 4 > buf.len() { eprintln!("❌ ERROR: Memory.read() offset {} out of bounds for float32 (size {})", offset, buf.len()); return EvalResult::Error; }
                        let bits = u32::from_le_bytes([buf[offset], buf[offset+1], buf[offset+2], buf[offset+3]]);
                        EvalResult::Value(self.alloc(ObjectData::Decimal(f32::from_bits(bits) as f64)))
                    }
                    "float64" | "decimal" => {
                        if offset + 8 > buf.len() { eprintln!("❌ ERROR: Memory.read() offset {} out of bounds for float64 (size {})", offset, buf.len()); return EvalResult::Error; }
                        let bytes: [u8; 8] = buf[offset..offset+8].try_into().unwrap();
                        let v = f64::from_le_bytes(bytes);
                        EvalResult::Value(self.alloc(ObjectData::Decimal(v)))
                    }
                    other => { eprintln!("❌ ERROR: Memory.read() — unknown type '{}'", other); EvalResult::Error }
                }
            }

            // Memory.write(handle, offset, type, value) — write a typed value into raw memory
            "write" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.write() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 4 {
                    eprintln!("❌ ERROR: Memory.write(handle, offset, type, value) requires 4 arguments");
                    return EvalResult::Error;
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
                        _ => { eprintln!("❌ ERROR: Memory.write() type must be a string"); return EvalResult::Error; }
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
                    None => { eprintln!("❌ ERROR: Memory.write() — null value"); return EvalResult::Error; }
                };
                let buf = match self.memory_heap.get_mut(&id) {
                    Some(b) => b,
                    None => { eprintln!("❌ ERROR: Memory.write() — no allocation with handle {}", id); return EvalResult::Error; }
                };
                let write_result = Self::memory_write_typed(buf, offset, &type_name, &val_data);
                if let Err(msg) = write_result {
                    eprintln!("❌ ERROR: Memory.write() — {}", msg);
                    return EvalResult::Error;
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
                    eprintln!("❌ ERROR: Memory.copy(src, dst, n) requires 3 arguments");
                    return EvalResult::Error;
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
                            eprintln!("❌ ERROR: Memory.copy() — src size {} < n {}", b.len(), n);
                            return EvalResult::Error;
                        }
                        b[..n].to_vec()
                    }
                    None => { eprintln!("❌ ERROR: Memory.copy() — no src allocation {}", src_id); return EvalResult::Error; }
                };
                let dst = match self.memory_heap.get_mut(&dst_id) {
                    Some(b) => b,
                    None => { eprintln!("❌ ERROR: Memory.copy() — no dst allocation {}", dst_id); return EvalResult::Error; }
                };
                if n > dst.len() {
                    eprintln!("❌ ERROR: Memory.copy() — dst size {} < n {}", dst.len(), n);
                    return EvalResult::Error;
                }
                dst[..n].copy_from_slice(&src_bytes);
                EvalResult::Value(self.null_ref)
            }

            // Memory.fill(handle, value) — fill entire allocation with a byte value
            "fill" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Memory.fill() requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Memory.fill(handle, byte_value) requires 2 arguments");
                    return EvalResult::Error;
                }
                let id = match self.eval_memory_id(&dot_call.arguments[0]) {
                    Ok(v) => v, Err(e) => return e,
                };
                let byte_val = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(n)) => {
                            if n < 0 || n > 255 { eprintln!("❌ ERROR: Memory.fill() byte value must be 0–255"); return EvalResult::Error; }
                            n as u8
                        }
                        _ => { eprintln!("❌ ERROR: Memory.fill() byte value must be an integer 0–255"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.memory_heap.get_mut(&id) {
                    Some(buf) => { buf.iter_mut().for_each(|b| *b = byte_val); }
                    None => { eprintln!("❌ ERROR: Memory.fill() — no allocation with handle {}", id); return EvalResult::Error; }
                }
                EvalResult::Value(self.null_ref)
            }

            // Memory.offsetOf(class_name, field_name) → int — simulated word-aligned field offset
            "offsetOf" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Memory.offsetOf(class, field) requires 2 arguments");
                    return EvalResult::Error;
                }
                let class_name = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { eprintln!("❌ ERROR: Memory.offsetOf() class must be a string"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let field_name = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => { eprintln!("❌ ERROR: Memory.offsetOf() field must be a string"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let class = match self.class_registry.get(&class_name).cloned() {
                    Some(c) => c,
                    None => { eprintln!("❌ ERROR: Memory.offsetOf() — unknown class '{}'", class_name); return EvalResult::Error; }
                };
                // Compute word-aligned offset for the named field
                let field_idx = class.fields.iter().position(|f| f.name == field_name);
                match field_idx {
                    Some(idx) => EvalResult::Value(self.alloc(ObjectData::Integer((idx * 8) as i64))),
                    None => { eprintln!("❌ ERROR: Memory.offsetOf() — class '{}' has no field '{}'", class_name, field_name); EvalResult::Error }
                }
            }

            other => {
                eprintln!("❌ ERROR: Unknown Memory method '{}'", other);
                EvalResult::Error
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
