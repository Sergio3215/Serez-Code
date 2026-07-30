//! Running and importing `.szx` (serez-ui JSX) sources.
//!
//! Translation is delegated to serez-ui's own translator, which is written in `.sz`
//! — so this shells out to a second `sz` process rather than linking a translator
//! in. Native only: the wasm build has no process to spawn, and the playground runs
//! a single file with no packages.

/// Locate serez-ui's `.szx → .sz` translator (`tools/translate.sz`), searching
/// the local project packages, the source file's packages, the global store, and
/// the executable's directory (for packaged apps that bundle serez-ui).
pub fn find_szx_translator(szx: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("packages"));
    }
    if let Some(dir) = szx.parent() {
        let dir = if dir == std::path::Path::new("") { std::path::Path::new(".") } else { dir };
        roots.push(dir.join("packages"));
    }
    roots.push(crate::package_manager::packages_dir());
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            roots.push(d.to_path_buf());
        }
    }
    for r in roots {
        let cand = r.join("serez-ui").join("tools").join("translate.sz");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Run a `.szx` (serez-ui JSX) file directly: translate it to `.sz` with
/// serez-ui's translator, run the result, then clean up. This is what the old
/// `szx.ps1` / `szx.sh` wrappers did — now the runtime does it itself, so
/// `sz app.szx` just works (and opens the UI).
pub fn run_szx_file(szx_path: &str, is_check: bool) -> i32 {
    let szx = std::path::Path::new(szx_path);
    if !szx.exists() {
        eprintln!("❌ ERROR reading file '{}': not found", szx_path);
        return 1;
    }
    let translator = match find_szx_translator(szx) {
        Some(t) => t,
        None => {
            eprintln!(
                "❌ ERROR: cannot run '{}': serez-ui not found. Install it with `sz install serez-ui` to run .szx files.",
                szx_path
            );
            return 1;
        }
    };
    let sz_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ ERROR: cannot locate the sz executable: {}", e);
            return 1;
        }
    };
    // Translate next to the source so the app's relative imports still resolve.
    let out_sz = szx.with_extension("szx.sz");
    let mut cmd = std::process::Command::new(&sz_exe);
    cmd.arg(&translator)
        .arg(szx)
        .arg(&out_sz)
        .stdout(std::process::Stdio::null()); // hide the translator's own chatter
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW); // never pop a console for the translate step
    }
    // Capture stderr: the child runs detached from the console (CREATE_NO_WINDOW),
    // so the translator's own error (e.g. invalid JSX + how to fix it) would be
    // lost unless we pipe it and re-print it here.
    cmd.stderr(std::process::Stdio::piped());
    let output = cmd.output();
    let ok = matches!(output, Ok(ref o) if o.status.success()) && out_sz.exists();
    if !ok {
        let _ = std::fs::remove_file(&out_sz);
        if let Ok(ref o) = output {
            let msg = String::from_utf8_lossy(&o.stderr);
            let msg = msg.trim();
            if !msg.is_empty() {
                eprintln!("{}", msg.replace("UNCAUGHT EXCEPTION:", "TRANSLATE ERROR:"));
            }
        }
        eprintln!(
            "❌ ERROR: could not translate '{}' (is it valid .szx, and is serez-ui's translator present?)",
            szx_path
        );
        return 1;
    }
    let code = crate::run::run_file(out_sz.to_string_lossy().as_ref(), is_check);
    let _ = std::fs::remove_file(&out_sz); // best-effort cleanup
    code
}

/// Translate a `.szx` (serez-ui JSX) module to `.sz` source and return the
/// translated text. Used by the import resolver so `import "src/components"` can
/// pull in a `.szx` file (JSX components split into their own files), not just
/// plain `.sz`. Translates into a throwaway temp file (the caller controls the
/// module's directory for relative imports, so the temp's location is irrelevant),
/// reads it back, and deletes it. Returns None if serez-ui's translator isn't
/// found or the translation fails.
pub fn translate_szx_to_string(szx: &std::path::Path) -> Option<String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let translator = find_szx_translator(szx)?;
    let sz_exe = std::env::current_exe().ok()?;
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let out_sz = std::env::temp_dir().join(format!("szimport_{}_{}.sz", std::process::id(), n));

    let mut cmd = std::process::Command::new(&sz_exe);
    cmd.arg(&translator)
        .arg(szx)
        .arg(&out_sz)
        .stdout(std::process::Stdio::null()); // hide the translator's own chatter
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    // Piped stderr for the same reason as run_szx_file: surface the translator's
    // error (lost to CREATE_NO_WINDOW) before the import-level message.
    cmd.stderr(std::process::Stdio::piped());
    let output = cmd.output();
    let ok = matches!(output, Ok(ref o) if o.status.success()) && out_sz.exists();
    if !ok {
        if let Ok(ref o) = output {
            let msg = String::from_utf8_lossy(&o.stderr);
            let msg = msg.trim();
            if !msg.is_empty() {
                eprintln!("{}", msg.replace("UNCAUGHT EXCEPTION:", "TRANSLATE ERROR:"));
            }
        }
    }
    let translated = if ok { std::fs::read_to_string(&out_sz).ok() } else { None };
    let _ = std::fs::remove_file(&out_sz); // best-effort cleanup
    translated
}
