#![allow(unused_imports)]
use super::EvalResult;
use crate::ast::{self};
use crate::region::{ObjectData, ObjectRef, OwnedValue};
use std::io::{Read, Write};

const PROTECTED_PROCESS_TARGET_FRAGMENTS: &[&str] = &[
    "C:\\Windows\\System32",
    "/etc/",
    "/bin/",
    "/sbin/",
    "/usr/bin/",
];

/// Proceso lanzado con OS.spawn (no bloqueante). OS.tick() lo cosecha (try_wait) y
/// devuelve [pid, code, errMsg] como DATO (no callbacks: guardar refs de closures .sz
/// es use-after-free en el modelo de valor/regiones). stderr queda piped para el msg.
pub(super) struct SpawnedJob {
    pub child: std::process::Child,
    pub pid: i64,
    /// The child's stderr, drained **while it runs** by the thread below.
    ///
    /// It used to be read from `child.stderr` inside `OS.tick`, *after*
    /// `try_wait()` reported the child had exited. That deadlocks: a child that
    /// writes more than the pipe buffer holds blocks on the write, so it never
    /// exits, so `try_wait` never reports it, so nothing ever drains the pipe.
    /// Measured before the fix — a child writing ~600 KB to stderr was never
    /// harvested across 200 polls over 10 seconds, while the same command
    /// writing three lines was harvested on the second poll.
    ///
    /// A reader thread per child is the portable answer; the alternative is
    /// non-blocking pipe reads, which are platform-specific in three different
    /// ways. The thread ends when the pipe closes, which is when the child exits.
    pub stderr: std::sync::Arc<std::sync::Mutex<String>>,
}

// ── Platform helpers (no external deps) ──────────────────────────────────────

#[cfg(windows)]
#[repr(C)]
struct MemoryStatusEx {
    dw_length: u32,
    dw_memory_load: u32,
    ull_total_phys: u64,
    ull_avail_phys: u64,
    ull_total_page_file: u64,
    ull_avail_page_file: u64,
    ull_total_virtual: u64,
    ull_avail_virtual: u64,
    ull_avail_extended_virtual: u64,
}

#[cfg(windows)]
unsafe extern "system" {
    fn GlobalMemoryStatusEx(lp: *mut MemoryStatusEx) -> i32;
}

#[cfg(windows)]
fn os_memory_status() -> Option<MemoryStatusEx> {
    let mut info: MemoryStatusEx = unsafe { std::mem::zeroed() };
    info.dw_length = std::mem::size_of::<MemoryStatusEx>() as u32;
    if unsafe { GlobalMemoryStatusEx(&mut info) } != 0 {
        Some(info)
    } else {
        None
    }
}

#[cfg(windows)]
fn os_total_memory() -> i64 {
    os_memory_status()
        .map(|s| s.ull_total_phys as i64)
        .unwrap_or(-1)
}

#[cfg(windows)]
fn os_free_memory() -> i64 {
    os_memory_status()
        .map(|s| s.ull_avail_phys as i64)
        .unwrap_or(-1)
}

#[cfg(windows)]
fn os_uptime_secs() -> i64 {
    unsafe extern "system" {
        fn GetTickCount64() -> u64;
    }
    unsafe { GetTickCount64() as i64 / 1000 }
}

#[cfg(windows)]
fn os_hostname() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string())
}

// ── macOS ────────────────────────────────────────────────────────────────────
// Darwin has no `/proc`, so the readers below returned -1 there and every
// `System` memory number was a lie on macOS. The same values come from
// `sysctl`, declared here rather than taking on `libc` for one symbol.
#[cfg(target_os = "macos")]
mod darwin {
    use std::ffi::{CString, c_char, c_int, c_void};

    unsafe extern "C" {
        fn sysctlbyname(
            name: *const c_char,
            oldp: *mut c_void,
            oldlenp: *mut usize,
            newp: *mut c_void,
            newlen: usize,
        ) -> c_int;
    }

    /// Reads a sysctl into `out` and reports how many bytes the kernel wrote.
    /// A name the kernel does not know is `None`, never a half-filled buffer.
    pub fn raw(name: &str, out: &mut [u8]) -> Option<usize> {
        let cname = CString::new(name).ok()?;
        let mut len = out.len();
        let rc = unsafe {
            sysctlbyname(
                cname.as_ptr(),
                out.as_mut_ptr() as *mut c_void,
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 && len <= out.len() {
            Some(len)
        } else {
            None
        }
    }

    /// The integer sysctls are 4 or 8 bytes wide depending on the name, so
    /// both widths are accepted and widened.
    pub fn integer(name: &str) -> Option<u64> {
        let mut buf = [0u8; 8];
        match raw(name, &mut buf)? {
            8 => Some(u64::from_ne_bytes(buf)),
            4 => Some(u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as u64),
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
fn os_total_memory() -> i64 {
    darwin::integer("hw.memsize")
        .map(|v| v as i64)
        .unwrap_or(-1)
}

#[cfg(target_os = "macos")]
fn os_free_memory() -> i64 {
    // Darwin publishes no `MemAvailable` equivalent. `vm.page_free_count` is
    // the honest reading: the pages the kernel holds free right now, counted
    // in `hw.pagesize` units. It sits below what a Linux caller would expect,
    // because Darwin keeps cached pages out of it.
    match (
        darwin::integer("vm.page_free_count"),
        darwin::integer("hw.pagesize"),
    ) {
        (Some(pages), Some(size)) => pages.saturating_mul(size) as i64,
        _ => -1,
    }
}

#[cfg(target_os = "macos")]
fn os_uptime_secs() -> i64 {
    // `kern.boottime` is a `struct timeval`; only `tv_sec`, its first eight
    // bytes, is wanted.
    let mut buf = [0u8; 16];
    if !matches!(darwin::raw("kern.boottime", &mut buf), Some(n) if n >= 8) {
        return -1;
    }
    let mut secs = [0u8; 8];
    secs.copy_from_slice(&buf[..8]);
    let boot = i64::from_ne_bytes(secs);
    let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(_) => return -1,
    };
    if boot <= 0 || now < boot {
        -1
    } else {
        now - boot
    }
}

#[cfg(target_os = "macos")]
fn os_hostname() -> String {
    // `/etc/hostname` does not exist on Darwin and `HOSTNAME` is not exported
    // by every shell, so the reader below answered "unknown" on every Mac.
    let mut buf = [0u8; 256];
    if let Some(n) = darwin::raw("kern.hostname", &mut buf) {
        let bytes = &buf[..n];
        let bytes = match bytes.iter().position(|&b| b == 0) {
            Some(nul) => &bytes[..nul],
            None => bytes,
        };
        if let Ok(name) = std::str::from_utf8(bytes) {
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_total_memory() -> i64 {
    if let Ok(c) = std::fs::read_to_string("/proc/meminfo") {
        for line in c.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb) = line.split_whitespace().nth(1) {
                    if let Ok(v) = kb.parse::<u64>() {
                        return (v * 1024) as i64;
                    }
                }
            }
        }
    }
    -1
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_free_memory() -> i64 {
    if let Ok(c) = std::fs::read_to_string("/proc/meminfo") {
        for line in c.lines() {
            if line.starts_with("MemAvailable:") {
                if let Some(kb) = line.split_whitespace().nth(1) {
                    if let Ok(v) = kb.parse::<u64>() {
                        return (v * 1024) as i64;
                    }
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
            if let Ok(f) = s.parse::<f64>() {
                return f as i64;
            }
        }
    }
    -1
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn os_uptime_secs() -> i64 {
    -1
}

#[cfg(not(any(windows, target_os = "macos")))]
fn os_hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "unknown".to_string())
}

// ── Namespace implementations ─────────────────────────────────────────────────

impl super::Evaluator {
    // ── Terminal ──────────────────────────────────────────────────────────────

    pub(super) fn eval_terminal_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        if let Some(error) = self.require_permission("Terminal", "Terminal") {
            return error;
        }
        match dot_call.method.as_str() {
            "getSize" => {
                if let Some(error) = self.reject_arguments(dot_call, "Terminal") {
                    return error;
                }
                match crossterm::terminal::size() {
                    Ok((cols, rows)) => {
                        let cr = self.alloc(ObjectData::Integer(cols as i64));
                        let rr = self.alloc(ObjectData::Integer(rows as i64));
                        EvalResult::Value(self.alloc(ObjectData::Array {
                            element_type: Some("int".to_string()),
                            elements: vec![self.extract(cr), self.extract(rr)],
                        }))
                    }
                    Err(e) => {
                        self.rt_err_kind("IOError", format!("Terminal.getSize failed: {}", e))
                    }
                }
            }

            "clear" => {
                if let Some(error) = self.reject_arguments(dot_call, "Terminal") {
                    return error;
                }
                use crossterm::{
                    ExecutableCommand,
                    terminal::{Clear, ClearType},
                };
                match std::io::stdout().execute(Clear(ClearType::All)) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => self.rt_err_kind("IOError", format!("Terminal.clear failed: {}", e)),
                }
            }

            "setCursor" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind(
                        "TypeError",
                        "Terminal.setCursor(row, col) requires 2 arguments",
                    );
                }
                let rr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let cr = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let row = match self.resolve(rr).cloned() {
                    Some(ObjectData::Integer(v)) => v as u16,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Terminal.setCursor row must be an integer");
                    }
                };
                let col = match self.resolve(cr).cloned() {
                    Some(ObjectData::Integer(v)) => v as u16,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Terminal.setCursor col must be an integer");
                    }
                };
                use crossterm::{ExecutableCommand, cursor::MoveTo};
                match std::io::stdout().execute(MoveTo(col, row)) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => {
                        self.rt_err_kind("IOError", format!("Terminal.setCursor failed: {}", e))
                    }
                }
            }

            "writeByte" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Terminal.writeByte(byte) requires 1 argument");
                }
                let br = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let byte = match self.resolve(br).cloned() {
                    Some(ObjectData::Integer(v)) if v >= 0 && v <= 255 => v as u8,
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            "Terminal.writeByte requires an integer 0-255",
                        );
                    }
                };
                let mut out = std::io::stdout();
                if out.write_all(&[byte]).is_err() || out.flush().is_err() {
                    return self.rt_err_kind("IOError", "Terminal.writeByte write failed");
                }
                EvalResult::Value(self.null_ref)
            }

            "setRawMode" => {
                if let Some(error) =
                    self.require_unsafe("Terminal.setRawMode", "it modifies OS state")
                {
                    return error;
                }
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "Terminal.setRawMode(bool) requires 1 argument");
                }
                let ar = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let enable = match self.resolve(ar).cloned() {
                    Some(ObjectData::Boolean(b)) => b,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Terminal.setRawMode requires a boolean");
                    }
                };
                let result = if enable {
                    crossterm::terminal::enable_raw_mode()
                } else {
                    crossterm::terminal::disable_raw_mode()
                };
                match result {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => {
                        self.rt_err_kind("IOError", format!("Terminal.setRawMode failed: {}", e))
                    }
                }
            }

            "readByte" => {
                if let Some(error) =
                    self.require_unsafe("Terminal.readByte", "it reads raw terminal input")
                {
                    return error;
                }
                if !dot_call.arguments.is_empty() {
                    return self.rt_err_kind("TypeError", "Terminal.readByte() takes no arguments");
                }
                let mut buf = [0u8; 1];
                match std::io::stdin().lock().read_exact(&mut buf) {
                    Ok(_) => EvalResult::Value(self.alloc(ObjectData::Integer(buf[0] as i64))),
                    Err(e) => {
                        self.rt_err_kind("IOError", format!("Terminal.readByte failed: {}", e))
                    }
                }
            }

            "enableMouse" => {
                if let Some(error) =
                    self.require_unsafe("Terminal.enableMouse", "it modifies OS input state")
                {
                    return error;
                }
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind(
                        "TypeError",
                        "Terminal.enableMouse(bool) requires 1 argument",
                    );
                }
                let ar = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let enable = match self.resolve(ar).cloned() {
                    Some(ObjectData::Boolean(b)) => b,
                    _ => {
                        return self
                            .rt_err_kind("TypeError", "Terminal.enableMouse requires a boolean");
                    }
                };
                use crossterm::{
                    ExecutableCommand,
                    event::{DisableMouseCapture, EnableMouseCapture},
                };
                let mut out = std::io::stdout();
                let result = if enable {
                    out.execute(EnableMouseCapture)
                } else {
                    out.execute(DisableMouseCapture)
                };
                match result {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => {
                        self.rt_err_kind("IOError", format!("Terminal.enableMouse failed: {}", e))
                    }
                }
            }

            // readEvent() → { type: "key"|"mouse"|"resize", ... }
            // key:    { type: "key",    code: string, modifiers: [string] }
            // mouse:  { type: "mouse",  kind: string, button: string, col: int, row: int, modifiers: [string] }
            // resize: { type: "resize", cols: int, rows: int }
            "readEvent" => {
                if let Some(error) =
                    self.require_unsafe("Terminal.readEvent", "it reads raw terminal input")
                {
                    return error;
                }
                if !dot_call.arguments.is_empty() {
                    return self
                        .rt_err_kind("TypeError", "Terminal.readEvent() takes no arguments");
                }
                use crossterm::event::{
                    self, Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind,
                };
                match event::read() {
                    Ok(Event::Key(key)) => {
                        let code = match key.code {
                            KeyCode::Char(c) => c.to_string(),
                            KeyCode::Enter => "Enter".to_string(),
                            KeyCode::Backspace => "Backspace".to_string(),
                            KeyCode::Left => "Left".to_string(),
                            KeyCode::Right => "Right".to_string(),
                            KeyCode::Up => "Up".to_string(),
                            KeyCode::Down => "Down".to_string(),
                            KeyCode::Tab => "Tab".to_string(),
                            KeyCode::Esc => "Esc".to_string(),
                            KeyCode::Delete => "Delete".to_string(),
                            KeyCode::Home => "Home".to_string(),
                            KeyCode::End => "End".to_string(),
                            KeyCode::PageUp => "PageUp".to_string(),
                            KeyCode::PageDown => "PageDown".to_string(),
                            KeyCode::F(n) => format!("F{}", n),
                            _ => "Unknown".to_string(),
                        };
                        let mut mods: Vec<OwnedValue> = Vec::new();
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            mods.push(OwnedValue::Str("ctrl".to_string()));
                        }
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            mods.push(OwnedValue::Str("alt".to_string()));
                        }
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            mods.push(OwnedValue::Str("shift".to_string()));
                        }
                        let mods_owned = OwnedValue::Array {
                            element_type: Some("string".to_string()),
                            elements: mods,
                        };
                        EvalResult::Value(self.alloc(ObjectData::Instance {
                            class_name: "KeyEvent".to_string(),
                            fields: vec![
                                ("type".to_string(), OwnedValue::Str("key".to_string())),
                                ("code".to_string(), OwnedValue::Str(code)),
                                ("modifiers".to_string(), mods_owned),
                            ],
                        }))
                    }
                    Ok(Event::Mouse(mouse)) => {
                        let kind = match mouse.kind {
                            MouseEventKind::Down(_) => "down",
                            MouseEventKind::Up(_) => "up",
                            MouseEventKind::Drag(_) => "drag",
                            MouseEventKind::Moved => "move",
                            MouseEventKind::ScrollDown => "scrollDown",
                            MouseEventKind::ScrollUp => "scrollUp",
                            _ => "unknown",
                        }
                        .to_string();
                        let button = match mouse.kind {
                            MouseEventKind::Down(b)
                            | MouseEventKind::Up(b)
                            | MouseEventKind::Drag(b) => match b {
                                MouseButton::Left => "left",
                                MouseButton::Right => "right",
                                MouseButton::Middle => "middle",
                            },
                            _ => "none",
                        }
                        .to_string();
                        let mut mods: Vec<OwnedValue> = Vec::new();
                        if mouse.modifiers.contains(KeyModifiers::CONTROL) {
                            mods.push(OwnedValue::Str("ctrl".to_string()));
                        }
                        if mouse.modifiers.contains(KeyModifiers::ALT) {
                            mods.push(OwnedValue::Str("alt".to_string()));
                        }
                        if mouse.modifiers.contains(KeyModifiers::SHIFT) {
                            mods.push(OwnedValue::Str("shift".to_string()));
                        }
                        let mods_owned = OwnedValue::Array {
                            element_type: Some("string".to_string()),
                            elements: mods,
                        };
                        EvalResult::Value(self.alloc(ObjectData::Instance {
                            class_name: "MouseEvent".to_string(),
                            fields: vec![
                                ("type".to_string(), OwnedValue::Str("mouse".to_string())),
                                ("kind".to_string(), OwnedValue::Str(kind)),
                                ("button".to_string(), OwnedValue::Str(button)),
                                ("col".to_string(), OwnedValue::Integer(mouse.column as i64)),
                                ("row".to_string(), OwnedValue::Integer(mouse.row as i64)),
                                ("modifiers".to_string(), mods_owned),
                            ],
                        }))
                    }
                    Ok(Event::Resize(cols, rows)) => {
                        EvalResult::Value(self.alloc(ObjectData::Instance {
                            class_name: "ResizeEvent".to_string(),
                            fields: vec![
                                ("type".to_string(), OwnedValue::Str("resize".to_string())),
                                ("cols".to_string(), OwnedValue::Integer(cols as i64)),
                                ("rows".to_string(), OwnedValue::Integer(rows as i64)),
                            ],
                        }))
                    }
                    Err(e) => {
                        self.rt_err_kind("IOError", format!("Terminal.readEvent failed: {}", e))
                    }
                    _ => EvalResult::Value(self.null_ref),
                }
            }

            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("ReferenceError", format!("Unknown Terminal method '{}'", m))
            }
        }
    }

    /// Collect the argument vector for `OS.exec` / `OS.spawn`.
    ///
    /// This was `if let Some(Array) = .. { for elem { if let Str(s) = elem { .. } } }`,
    /// with no `else` on either shape. A non-array `args` was ignored entirely
    /// and a non-string element was dropped, so the process ran with a
    /// different argument list than the caller wrote — and still reported
    /// success. `OS.exec("cmd", "/c echo hi")` launched a bare interactive
    /// shell; `OS.exec("cmd", ["/c", "echo", 42])` echoed nothing. Both now
    /// fail before anything is started.
    fn process_argument_list(
        &mut self,
        expr: &ast::Expression,
        operation: &str,
    ) -> Result<Vec<String>, EvalResult> {
        let arg_ref = match self.eval_expression(expr) {
            EvalResult::Value(v) => v,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            _ => return Err(EvalResult::Error),
        };
        let elements = match self.resolve(arg_ref).cloned() {
            Some(ObjectData::Array { elements, .. }) => elements,
            _ => {
                return Err(self.rt_err_kind(
                    "TypeError",
                    format!("{operation}: args must be an array of strings"),
                ));
            }
        };
        let mut collected = Vec::with_capacity(elements.len());
        for element in elements {
            match element {
                OwnedValue::Str(text) => collected.push(text),
                _ => {
                    return Err(self.rt_err_kind(
                        "TypeError",
                        format!("{operation}: every argument must be a string"),
                    ));
                }
            }
        }
        Ok(collected)
    }

    // ── OS ────────────────────────────────────────────────────────────────────

    pub(super) fn eval_os_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        if let Some(error) = self.require_permission("OS", "OS") {
            return error;
        }
        match dot_call.method.as_str() {
            "platform" => {
                if let Some(error) = self.reject_arguments(dot_call, "OS") {
                    return error;
                }
                EvalResult::Value(self.alloc(ObjectData::Str(std::env::consts::OS.to_string())))
            }

            "pid" => {
                if let Some(error) = self.reject_arguments(dot_call, "OS") {
                    return error;
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(std::process::id() as i64)))
            }

            "exec" => {
                if let Some(error) =
                    self.require_unsafe("OS.exec", "it executes an external process")
                {
                    return error;
                }
                if dot_call.arguments.is_empty() {
                    return self.rt_err_kind(
                        "TypeError",
                        "OS.exec(cmd, args) requires at least 1 argument",
                    );
                }
                let cr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let cmd = match self.resolve(cr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            "OS.exec: first argument must be a string command",
                        );
                    }
                };
                if let Some(error) = self.reject_protected_process_target("OS.exec", &cmd) {
                    return error;
                }
                let mut args_vec: Vec<String> = Vec::new();
                if dot_call.arguments.len() >= 2 {
                    args_vec = match self.process_argument_list(&dot_call.arguments[1], "OS.exec") {
                        Ok(list) => list,
                        Err(signal) => return signal,
                    };
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
                                ("code".to_string(), OwnedValue::Integer(code)),
                            ],
                        }))
                    }
                    Err(e) => {
                        self.rt_err_kind("OSError", format!("OS.exec '{}' failed: {}", cmd, e))
                    }
                }
            }

            "spawn" => {
                // No bloqueante: lanza el proceso y vuelve enseguida (a diferencia de
                // OS.exec que espera con .output()). Devuelve el PID (o -1 si no arrancó).
                //   OS.spawn(cmd, [args])
                // La notificación de fin/error se cosecha por OS.tick() (poll-based).
                if let Some(error) =
                    self.require_unsafe("OS.spawn", "it starts an external process")
                {
                    return error;
                }
                if dot_call.arguments.is_empty() {
                    return self.rt_err_kind(
                        "TypeError",
                        "OS.spawn(cmd, [args]) requires at least 1 argument",
                    );
                }
                let cr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let cmd = match self.resolve(cr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            "OS.spawn: first argument must be a string command",
                        );
                    }
                };
                if let Some(error) = self.reject_protected_process_target("OS.spawn", &cmd) {
                    return error;
                }
                let mut args_vec: Vec<String> = Vec::new();
                if dot_call.arguments.len() >= 2 {
                    args_vec = match self.process_argument_list(&dot_call.arguments[1], "OS.spawn")
                    {
                        Ok(list) => list,
                        Err(signal) => return signal,
                    };
                }
                // stderr piped (para el mensaje de error); sin ventana de consola en Windows.
                let mut command = std::process::Command::new(&cmd);
                command
                    .args(&args_vec)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::piped());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                    command.creation_flags(CREATE_NO_WINDOW);
                }
                match command.spawn() {
                    Ok(mut child) => {
                        let pid = child.id() as i64;
                        // Drain stderr concurrently. See `SpawnedJob::stderr`:
                        // reading it only after the child exits is a deadlock,
                        // because a full pipe is what stops it exiting.
                        let collected = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
                        if let Some(mut pipe) = child.stderr.take() {
                            let sink = std::sync::Arc::clone(&collected);
                            std::thread::spawn(move || {
                                let mut buffer = String::new();
                                // Errors are dropped deliberately: a child that
                                // writes invalid UTF-8 or closes early still has
                                // to be harvestable, and the exit code is the
                                // signal that matters.
                                let _ = pipe.read_to_string(&mut buffer);
                                if let Ok(mut sink) = sink.lock() {
                                    *sink = buffer;
                                }
                            });
                        }
                        self.spawned.push(SpawnedJob {
                            child,
                            pid,
                            stderr: collected,
                        });
                        EvalResult::Value(self.int_ref(pid))
                    }
                    Err(e) => {
                        // `-1` is the documented return, so the program keeps
                        // going — which means this is a warning, not an error.
                        // It printed the "❌ ERROR:" marker, the one the CLI and
                        // the conformance runner both read as "this program
                        // failed", while the program went on to exit 0.
                        eprintln!("⚠️  WARNING: OS.spawn '{}' failed: {}", cmd, e);
                        EvalResult::Value(self.int_ref(-1))
                    }
                }
            }

            "tick" => {
                if let Some(error) = self.reject_arguments(dot_call, "OS") {
                    return error;
                }
                // Cosecha (no bloqueante) los procesos de OS.spawn ya terminados y los
                // DEVUELVE como datos: array de [pid, code, errMsg]. La app reacciona
                // (p.ej. en onFrame). No requiere `unsafe` (no lanza nada nuevo).
                let mut finished: Vec<OwnedValue> = Vec::new();
                let mut i = 0;
                while i < self.spawned.len() {
                    let status = match self.spawned[i].child.try_wait() {
                        Ok(Some(st)) => Some(st.code().unwrap_or(-1)),
                        Ok(None) => None,   // sigue corriendo
                        Err(_) => Some(-1), // error al consultar → fallo
                    };
                    match status {
                        None => {
                            i += 1;
                        }
                        Some(code) => {
                            let job = self.spawned.remove(i);
                            let pid = job.pid;
                            // The reader thread may still hold the last few
                            // bytes when the child has only just exited. Join by
                            // dropping our handle and taking what it collected;
                            // a lock failure means the thread panicked, which
                            // costs the message and not the harvest.
                            let errbuf = job
                                .stderr
                                .lock()
                                .map(|guard| guard.clone())
                                .unwrap_or_default();
                            let msg = if code == 0 || errbuf.trim().is_empty() {
                                String::new()
                            } else {
                                errbuf.trim().to_string()
                            };
                            // [pid, code, errMsg] como array de valor anidado
                            finished.push(OwnedValue::Array {
                                element_type: Some("any".to_string()),
                                elements: vec![
                                    OwnedValue::Integer(pid),
                                    OwnedValue::Integer(code as i64),
                                    OwnedValue::Str(msg),
                                ],
                            });
                            // no incrementar i: remove() desplazó el resto
                        }
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("any".to_string()),
                    elements: finished,
                }))
            }

            "kill" => {
                if let Some(error) = self.require_unsafe("OS.kill", "it terminates an OS process") {
                    return error;
                }
                if dot_call.arguments.is_empty() {
                    return self.rt_err_kind("TypeError", "OS.kill(pid) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let pid = match self.resolve(pr).cloned() {
                    Some(ObjectData::Integer(v)) => v,
                    _ => return self.rt_err_kind("TypeError", "OS.kill: pid must be an integer"),
                };
                // `.status()` let the helper inherit stderr, so `taskkill`'s own
                // 'The process "999999" not found.' appeared on the program's
                // stderr while `OS.kill` returned the success value. The caller
                // was told the process was killed; the terminal said otherwise;
                // nothing in the language reported anything. `.output()` captures
                // that text so it can become the diagnostic it always was.
                #[cfg(windows)]
                let result = std::process::Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .output();
                #[cfg(not(windows))]
                let result = std::process::Command::new("kill")
                    .arg(pid.to_string())
                    .output();
                match result {
                    Ok(output) if output.status.success() => EvalResult::Value(self.null_ref),
                    Ok(output) => {
                        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                        let detail = if detail.is_empty() {
                            format!("the helper exited {}", output.status.code().unwrap_or(-1))
                        } else {
                            detail
                        };
                        self.rt_err_kind("OSError", format!("OS.kill {} failed: {}", pid, detail))
                    }
                    Err(e) => self.rt_err_kind("OSError", format!("OS.kill {} failed: {}", pid, e)),
                }
            }

            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("ReferenceError", format!("Unknown OS method '{}'", m))
            }
        }
    }

    // ── Env ───────────────────────────────────────────────────────────────────

    pub(super) fn eval_env_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        if let Some(error) = self.require_permission("Env", "Env") {
            return error;
        }
        match dot_call.method.as_str() {
            "get" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Env.get(key) requires 1 argument");
                }
                let kr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let key = match self.resolve(kr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "Env.get requires a string key"),
                };
                match std::env::var(&key) {
                    Ok(val) => EvalResult::Value(self.alloc(ObjectData::Str(val))),
                    Err(_) => EvalResult::Value(self.null_ref),
                }
            }

            "args" => {
                if let Some(error) = self.reject_arguments(dot_call, "Env") {
                    return error;
                }
                let owned: Vec<OwnedValue> = std::env::args().map(|a| OwnedValue::Str(a)).collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("string".to_string()),
                    elements: owned,
                }))
            }

            "set" => {
                if let Some(error) =
                    self.require_unsafe("Env.set", "it modifies process environment state")
                {
                    return error;
                }
                if dot_call.arguments.len() != 2 {
                    return self
                        .rt_err_kind("TypeError", "Env.set(key, value) requires 2 arguments");
                }
                let kr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let vr = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let key = match self.resolve(kr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "Env.set key must be a string"),
                };
                let val = self.display(vr);
                unsafe { std::env::set_var(&key, &val) };
                EvalResult::Value(self.null_ref)
            }

            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("ReferenceError", format!("Unknown Env method '{}'", m))
            }
        }
    }

    // ── Time ──────────────────────────────────────────────────────────────────

    pub(super) fn eval_time_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        if let Some(error) = self.require_permission("Time", "Time") {
            return error;
        }
        match dot_call.method.as_str() {
            "now" => {
                if let Some(error) = self.reject_arguments(dot_call, "Time") {
                    return error;
                }
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                EvalResult::Value(self.alloc(ObjectData::Integer(ms)))
            }

            "sleep" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Time.sleep(ms) requires 1 argument");
                }
                let mr = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let ms = match self.resolve(mr).cloned() {
                    Some(ObjectData::Integer(v)) => v.max(0) as u64,
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            "Time.sleep requires an integer (milliseconds)",
                        );
                    }
                };
                std::thread::sleep(std::time::Duration::from_millis(ms));
                EvalResult::Value(self.null_ref)
            }

            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("ReferenceError", format!("Unknown Time method '{}'", m))
            }
        }
    }

    fn reject_protected_process_target(
        &mut self,
        operation: &str,
        command: &str,
    ) -> Option<EvalResult> {
        if !PROTECTED_PROCESS_TARGET_FRAGMENTS
            .iter()
            .any(|fragment| command.contains(fragment))
        {
            return None;
        }
        Some(self.fatal_err_kind(
            "SecurityError",
            format!("{operation} blocked — target contains a protected system path"),
        ))
    }

    // ── System ────────────────────────────────────────────────────────────────

    pub(super) fn eval_system_namespace(
        &mut self,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        if let Some(error) = self.require_permission("System", "System") {
            return error;
        }
        match dot_call.method.as_str() {
            "cpuCount" => {
                if let Some(error) = self.reject_arguments(dot_call, "System") {
                    return error;
                }
                let n = std::thread::available_parallelism()
                    .map(|n| n.get() as i64)
                    .unwrap_or(1);
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "totalMemory" => {
                if let Some(error) = self.reject_arguments(dot_call, "System") {
                    return error;
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(os_total_memory())))
            }

            "freeMemory" => {
                if let Some(error) = self.reject_arguments(dot_call, "System") {
                    return error;
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(os_free_memory())))
            }

            "hostname" => {
                if let Some(error) = self.reject_arguments(dot_call, "System") {
                    return error;
                }
                EvalResult::Value(self.alloc(ObjectData::Str(os_hostname())))
            }

            "uptime" => {
                if let Some(error) = self.reject_arguments(dot_call, "System") {
                    return error;
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(os_uptime_secs())))
            }

            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("ReferenceError", format!("Unknown System method '{}'", m))
            }
        }
    }
}
