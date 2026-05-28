#![allow(unused_imports)]
use crate::ast::{self};
use crate::region::{ObjectData, ObjectRef, OwnedValue};
use std::io::{Read, Write};
use super::EvalResult;

// ── Permission helpers ────────────────────────────────────────────────────────

macro_rules! require_perm {
    ($self:expr, $ns:expr) => {
        if !$self.permissions.contains($ns) {
            eprintln!(
                "❌ ERROR: '{}' requires permission '{}' — declare it in serez.json \
                 (\"permissions\": [\"{}\", ...]) or with `use permissions {{ {} }}`",
                $ns, $ns, $ns, $ns
            );
            return EvalResult::Error;
        }
    };
}

macro_rules! require_unsafe {
    ($self:expr, $method:expr) => {
        if !$self.in_unsafe_block {
            eprintln!(
                "❌ ERROR: '{}' requires an `unsafe {{ }}` block — it modifies OS state",
                $method
            );
            return EvalResult::Error;
        }
    };
}

// ── Platform helpers (no external deps) ──────────────────────────────────────

#[cfg(windows)]
fn os_total_memory() -> i64 {
    #[repr(C)]
    struct MEMORYSTATUSEX {
        dw_length: u32, dw_memory_load: u32,
        ull_total_phys: u64, ull_avail_phys: u64,
        ull_total_page_file: u64, ull_avail_page_file: u64,
        ull_total_virtual: u64, ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }
    unsafe extern "system" { fn GlobalMemoryStatusEx(lp: *mut MEMORYSTATUSEX) -> i32; }
    let mut info: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    info.dw_length = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut info) } != 0 { info.ull_total_phys as i64 } else { -1 }
}

#[cfg(windows)]
fn os_free_memory() -> i64 {
    #[repr(C)]
    struct MEMORYSTATUSEX {
        dw_length: u32, dw_memory_load: u32,
        ull_total_phys: u64, ull_avail_phys: u64,
        ull_total_page_file: u64, ull_avail_page_file: u64,
        ull_total_virtual: u64, ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }
    unsafe extern "system" { fn GlobalMemoryStatusEx(lp: *mut MEMORYSTATUSEX) -> i32; }
    let mut info: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    info.dw_length = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut info) } != 0 { info.ull_avail_phys as i64 } else { -1 }
}

#[cfg(windows)]
fn os_uptime_secs() -> i64 {
    unsafe extern "system" { fn GetTickCount64() -> u64; }
    unsafe { GetTickCount64() as i64 / 1000 }
}

#[cfg(windows)]
fn os_hostname() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(not(windows))]
fn os_total_memory() -> i64 {
    if let Ok(c) = std::fs::read_to_string("/proc/meminfo") {
        for line in c.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb) = line.split_whitespace().nth(1) {
                    if let Ok(v) = kb.parse::<u64>() { return (v * 1024) as i64; }
                }
            }
        }
    }
    -1
}

#[cfg(not(windows))]
fn os_free_memory() -> i64 {
    if let Ok(c) = std::fs::read_to_string("/proc/meminfo") {
        for line in c.lines() {
            if line.starts_with("MemAvailable:") {
                if let Some(kb) = line.split_whitespace().nth(1) {
                    if let Ok(v) = kb.parse::<u64>() { return (v * 1024) as i64; }
                }
            }
        }
    }
    -1
}

#[cfg(target_os = "linux")]
fn os_uptime_secs() -> i64 {
    if let Ok(c) = std::fs::read_to_string("/proc/uptime") {
        if let Some(s) = c.split_whitespace().next() {
            if let Ok(f) = s.parse::<f64>() { return f as i64; }
        }
    }
    -1
}

#[cfg(not(any(windows, target_os = "linux")))]
fn os_uptime_secs() -> i64 { -1 }

#[cfg(not(windows))]
fn os_hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "unknown".to_string())
}

// ── Namespace implementations ─────────────────────────────────────────────────

impl super::Evaluator {

    // ── Terminal ──────────────────────────────────────────────────────────────

    pub(super) fn eval_terminal_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        require_perm!(self, "Terminal");
        match dot_call.method.as_str() {

            "getSize" => {
                match crossterm::terminal::size() {
                    Ok((cols, rows)) => {
                        let cr = self.alloc(ObjectData::Integer(cols as i64));
                        let rr = self.alloc(ObjectData::Integer(rows as i64));
                        EvalResult::Value(self.alloc(ObjectData::Array {
                            element_type: Some("int".to_string()),
                            elements: vec![cr, rr],
                        }))
                    }
                    Err(e) => { eprintln!("❌ ERROR: Terminal.getSize failed: {}", e); EvalResult::Error }
                }
            }

            "clear" => {
                use crossterm::{ExecutableCommand, terminal::{Clear, ClearType}};
                match std::io::stdout().execute(Clear(ClearType::All)) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: Terminal.clear failed: {}", e); EvalResult::Error }
                }
            }

            "setCursor" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Terminal.setCursor(row, col) requires 2 arguments");
                    return EvalResult::Error;
                }
                let rr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let cr = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let row = match self.resolve(rr).cloned() {
                    Some(ObjectData::Integer(v)) => v as u16,
                    _ => { eprintln!("❌ ERROR: Terminal.setCursor row must be an integer"); return EvalResult::Error; }
                };
                let col = match self.resolve(cr).cloned() {
                    Some(ObjectData::Integer(v)) => v as u16,
                    _ => { eprintln!("❌ ERROR: Terminal.setCursor col must be an integer"); return EvalResult::Error; }
                };
                use crossterm::{ExecutableCommand, cursor::MoveTo};
                match std::io::stdout().execute(MoveTo(col, row)) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: Terminal.setCursor failed: {}", e); EvalResult::Error }
                }
            }

            "writeByte" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Terminal.writeByte(byte) requires 1 argument");
                    return EvalResult::Error;
                }
                let br = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let byte = match self.resolve(br).cloned() {
                    Some(ObjectData::Integer(v)) if v >= 0 && v <= 255 => v as u8,
                    _ => { eprintln!("❌ ERROR: Terminal.writeByte requires an integer 0-255"); return EvalResult::Error; }
                };
                let mut out = std::io::stdout();
                if out.write_all(&[byte]).is_err() || out.flush().is_err() {
                    eprintln!("❌ ERROR: Terminal.writeByte write failed");
                    return EvalResult::Error;
                }
                EvalResult::Value(self.null_ref)
            }

            "setRawMode" => {
                require_unsafe!(self, "Terminal.setRawMode");
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Terminal.setRawMode(bool) requires 1 argument");
                    return EvalResult::Error;
                }
                let ar = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let enable = match self.resolve(ar).cloned() {
                    Some(ObjectData::Boolean(b)) => b,
                    _ => { eprintln!("❌ ERROR: Terminal.setRawMode requires a boolean"); return EvalResult::Error; }
                };
                let result = if enable {
                    crossterm::terminal::enable_raw_mode()
                } else {
                    crossterm::terminal::disable_raw_mode()
                };
                match result {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: Terminal.setRawMode failed: {}", e); EvalResult::Error }
                }
            }

            "readByte" => {
                require_unsafe!(self, "Terminal.readByte");
                if !dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Terminal.readByte() takes no arguments");
                    return EvalResult::Error;
                }
                let mut buf = [0u8; 1];
                match std::io::stdin().lock().read_exact(&mut buf) {
                    Ok(_) => EvalResult::Value(self.alloc(ObjectData::Integer(buf[0] as i64))),
                    Err(e) => { eprintln!("❌ ERROR: Terminal.readByte failed: {}", e); EvalResult::Error }
                }
            }

            _ => { eprintln!("❌ ERROR: Unknown Terminal method '{}'", dot_call.method); EvalResult::Error }
        }
    }

    // ── OS ────────────────────────────────────────────────────────────────────

    pub(super) fn eval_os_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        require_perm!(self, "OS");
        match dot_call.method.as_str() {

            "platform" => {
                EvalResult::Value(self.alloc(ObjectData::Str(std::env::consts::OS.to_string())))
            }

            "pid" => {
                EvalResult::Value(self.alloc(ObjectData::Integer(std::process::id() as i64)))
            }

            "exec" => {
                require_unsafe!(self, "OS.exec");
                if dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: OS.exec(cmd, args) requires at least 1 argument");
                    return EvalResult::Error;
                }
                let cr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let cmd = match self.resolve(cr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: OS.exec: first argument must be a string command"); return EvalResult::Error; }
                };
                // Block system paths
                let blocked = ["C:\\Windows\\System32", "/etc/", "/bin/", "/sbin/", "/usr/bin/"];
                if blocked.iter().any(|b| cmd.contains(b)) {
                    eprintln!("❌ SECURITY ERROR: OS.exec blocked — targets a protected system path");
                    return EvalResult::Error;
                }
                let mut args_vec: Vec<String> = Vec::new();
                if dot_call.arguments.len() >= 2 {
                    let ar = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                    if let Some(ObjectData::Array { elements, .. }) = self.resolve(ar).cloned() {
                        for r in elements {
                            if let Some(ObjectData::Str(s)) = self.resolve(r).cloned() {
                                args_vec.push(s);
                            }
                        }
                    }
                }
                match std::process::Command::new(&cmd).args(&args_vec).output() {
                    Ok(output) => {
                        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();
                        let code = output.status.code().unwrap_or(-1) as i64;
                        EvalResult::Value(self.alloc(ObjectData::Instance {
                            class_name: "ExecResult".to_string(),
                            fields: vec![
                                ("stdout".to_string(), OwnedValue::Str(stdout_str)),
                                ("stderr".to_string(), OwnedValue::Str(stderr_str)),
                                ("code".to_string(),   OwnedValue::Integer(code)),
                            ],
                        }))
                    }
                    Err(e) => { eprintln!("❌ ERROR: OS.exec '{}' failed: {}", cmd, e); EvalResult::Error }
                }
            }

            "kill" => {
                require_unsafe!(self, "OS.kill");
                if dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: OS.kill(pid) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let pid = match self.resolve(pr).cloned() {
                    Some(ObjectData::Integer(v)) => v,
                    _ => { eprintln!("❌ ERROR: OS.kill: pid must be an integer"); return EvalResult::Error; }
                };
                #[cfg(windows)]
                let result = std::process::Command::new("taskkill").args(["/PID", &pid.to_string(), "/F"]).status();
                #[cfg(not(windows))]
                let result = std::process::Command::new("kill").arg(pid.to_string()).status();
                match result {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: OS.kill {} failed: {}", pid, e); EvalResult::Error }
                }
            }

            _ => { eprintln!("❌ ERROR: Unknown OS method '{}'", dot_call.method); EvalResult::Error }
        }
    }

    // ── Env ───────────────────────────────────────────────────────────────────

    pub(super) fn eval_env_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        require_perm!(self, "Env");
        match dot_call.method.as_str() {

            "get" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Env.get(key) requires 1 argument");
                    return EvalResult::Error;
                }
                let kr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let key = match self.resolve(kr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: Env.get requires a string key"); return EvalResult::Error; }
                };
                match std::env::var(&key) {
                    Ok(val) => EvalResult::Value(self.alloc(ObjectData::Str(val))),
                    Err(_)  => EvalResult::Value(self.null_ref),
                }
            }

            "args" => {
                let refs: Vec<ObjectRef> = std::env::args()
                    .map(|a| self.alloc(ObjectData::Str(a)))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("string".to_string()),
                    elements: refs,
                }))
            }

            "set" => {
                require_unsafe!(self, "Env.set");
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Env.set(key, value) requires 2 arguments");
                    return EvalResult::Error;
                }
                let kr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let vr = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let key = match self.resolve(kr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: Env.set key must be a string"); return EvalResult::Error; }
                };
                let val = self.display(vr);
                unsafe { std::env::set_var(&key, &val) };
                EvalResult::Value(self.null_ref)
            }

            _ => { eprintln!("❌ ERROR: Unknown Env method '{}'", dot_call.method); EvalResult::Error }
        }
    }

    // ── Time ──────────────────────────────────────────────────────────────────

    pub(super) fn eval_time_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        require_perm!(self, "Time");
        match dot_call.method.as_str() {

            "now" => {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(ms)))
            }

            "sleep" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Time.sleep(ms) requires 1 argument");
                    return EvalResult::Error;
                }
                let mr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(v) => v, _ => return EvalResult::Error };
                let ms = match self.resolve(mr).cloned() {
                    Some(ObjectData::Integer(v)) => v.max(0) as u64,
                    _ => { eprintln!("❌ ERROR: Time.sleep requires an integer (milliseconds)"); return EvalResult::Error; }
                };
                std::thread::sleep(std::time::Duration::from_millis(ms));
                EvalResult::Value(self.null_ref)
            }

            _ => { eprintln!("❌ ERROR: Unknown Time method '{}'", dot_call.method); EvalResult::Error }
        }
    }

    // ── System ────────────────────────────────────────────────────────────────

    pub(super) fn eval_system_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        require_perm!(self, "System");
        match dot_call.method.as_str() {

            "cpuCount" => {
                let n = std::thread::available_parallelism().map(|n| n.get() as i64).unwrap_or(1);
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "totalMemory" => {
                EvalResult::Value(self.alloc(ObjectData::Integer(os_total_memory())))
            }

            "freeMemory" => {
                EvalResult::Value(self.alloc(ObjectData::Integer(os_free_memory())))
            }

            "hostname" => {
                EvalResult::Value(self.alloc(ObjectData::Str(os_hostname())))
            }

            "uptime" => {
                EvalResult::Value(self.alloc(ObjectData::Integer(os_uptime_secs())))
            }

            _ => { eprintln!("❌ ERROR: Unknown System method '{}'", dot_call.method); EvalResult::Error }
        }
    }
}
