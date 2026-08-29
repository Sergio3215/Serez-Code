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
        let dir = if dir == std::path::Path::new("") {
            std::path::Path::new(".")
        } else {
            dir
        };
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

/// Where to write the translated `.sz`: next to the source, so the app's
/// relative imports still resolve, and unique per process and per call.
///
/// This used to be `szx.with_extension("szx.sz")` — a fixed name derived from
/// the source, so `sz app.szx` overwrote and then deleted any `app.szx.sz` the
/// user already had, with no prompt and no warning, on both the success and the
/// failure path. Two concurrent runs of the same file also raced for it.
/// `translate_szx_to_string` below had always made its own temp name unique
/// with the pid and a counter, for exactly this reason.
fn translated_path(szx: &std::path::Path) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);

    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let stem = szx
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "app".to_string());
    // The `.szx.sz` tail is kept so an existing ignore rule for the old name
    // still covers this one.
    let name = format!("{stem}.{}.{n}.szx.sz", std::process::id());
    match szx.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name),
        _ => std::path::PathBuf::from(name),
    }
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
    let out_sz = translated_path(szx);
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
    if code != 0 {
        // Diagnostics from a translated program carry the translated file's
        // name, line numbers and source snippet, and that file is removed a
        // line below — so the message named a path the reader could not open
        // and quoted a line they never wrote. Say so.
        eprintln!(
            "\u{2139}\u{fe0f}  the diagnostics above refer to the translated form of '{szx_path}', not to the source as written."
        );
    }
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
    let translated = if ok {
        std::fs::read_to_string(&out_sz).ok()
    } else {
        None
    };
    let _ = std::fs::remove_file(&out_sz); // best-effort cleanup
    translated
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_translated_file_never_takes_a_name_the_user_could_own() {
        // The whole defect in one line: this used to be
        // `szx.with_extension("szx.sz")`, so running `sz app.szx` overwrote and
        // then deleted an existing `app.szx.sz` — on the success path and on the
        // failure path — with no prompt and no warning. Measured against the
        // built binary before the fix: a file holding user text was gone
        // afterwards.
        let szx = Path::new("proj/app.szx");
        let out = translated_path(szx);
        assert_ne!(out, Path::new("proj/app.szx.sz"));
        assert_ne!(out, szx.with_extension("szx.sz"));
    }

    #[test]
    fn the_translated_file_sits_beside_the_source() {
        // Not a detail: the translator's output has the app's relative imports
        // in it, so it only resolves from the source's own directory. A temp
        // directory would break every `import "comp/Chip"`.
        let out = translated_path(Path::new("proj/sub/app.szx"));
        assert_eq!(out.parent(), Some(Path::new("proj/sub")));

        // A bare filename has an empty parent, which must not become an
        // absolute-looking join.
        let bare = translated_path(Path::new("app.szx"));
        assert_eq!(bare.parent(), Some(Path::new("")));
    }

    #[test]
    fn two_runs_of_the_same_source_do_not_share_a_path() {
        // A fixed name meant two concurrent runs raced to write and delete the
        // same file. The import path in this module had always been unique for
        // this reason; the run path had not.
        let a = translated_path(Path::new("app.szx"));
        let b = translated_path(Path::new("app.szx"));
        assert_ne!(a, b);
    }

    #[test]
    fn the_name_still_ends_in_the_extension_an_ignore_rule_would_match() {
        // `.szx.sz` is kept so a project that already ignores the old artifact
        // keeps ignoring this one, and so `run_file` accepts it as `.sz`.
        let out = translated_path(Path::new("app.szx"));
        let name = out.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.ends_with(".szx.sz"), "{name}");
        assert!(name.starts_with("app."), "{name}");
    }

    #[test]
    fn a_missing_translator_is_reported_not_guessed() {
        // find_szx_translator returns None rather than a path that does not
        // exist, so the caller can print the install hint instead of failing to
        // spawn something.
        let found = find_szx_translator(Path::new("no/such/place/app.szx"));
        if let Some(path) = found {
            assert!(
                path.exists(),
                "returned a translator that is not there: {path:?}"
            );
        }
    }
}
