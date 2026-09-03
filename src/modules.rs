//! Module resolution and load tracking.
//!
//! `import` does two separable things: it decides **which file** a path string
//! means, and it **executes** that file. Only the second needs the evaluator.
//! The first is a pure function of the path, the importing file's directory and
//! the environment — so it lives here, where it can be tested without building
//! an interpreter and where the search order is one readable list instead of a
//! `flat_map` buried in a 200-line statement handler.
//!
//! Execution stays in the evaluator, because running a module means evaluating
//! statements against its arenas, registries and export tracking. This is the
//! seam, not a rewrite: the evaluator still owns everything it owned before
//! except the two questions answered here.
//!
//! The contract these functions implement is frozen in `spec/modules.md`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Directories searched for an import, in order. The first hit wins.
///
/// 1. the directory of the **importing file** — a relative import;
/// 2. the process working directory;
/// 3. `<cwd>/packages/`, where `sz install` puts local packages;
/// 4. `$SEREZ_HOME`, if set;
/// 5. the directory of the `sz` executable, for a bundled stdlib;
/// 6. `$SEREZ_PACKAGES`, or `~/.serez/packages/` when it is unset.
pub fn search_dirs(current_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    if let Some(dir) = current_dir {
        dirs.push(dir.to_path_buf());
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    dirs.push(cwd.clone());
    dirs.push(cwd.join("packages"));
    if let Ok(home) = std::env::var("SEREZ_HOME") {
        dirs.push(PathBuf::from(home));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            dirs.push(exe_dir.to_path_buf());
        }
    }
    dirs.push(crate::package_manager::packages_dir());

    dirs
}

/// The four names tried inside one base directory, in order.
///
/// `.sz` beats `.szx` in the same directory, and a file beats a directory. The
/// `.sz` extension is optional in the import string and is added when absent.
pub fn candidates_in(base: &Path, spec: &str) -> Vec<PathBuf> {
    let with_ext = if spec.ends_with(".sz") {
        spec.to_string()
    } else {
        format!("{spec}.sz")
    };
    let stem = spec.trim_end_matches(".sz").trim_end_matches(".szx");

    vec![
        base.join(&with_ext),             // <base>/<spec>.sz
        base.join(format!("{stem}.szx")), // <base>/<stem>.szx
        base.join(stem).join("index.sz"), // <base>/<stem>/index.sz
        base.join(stem).join("index.szx"),
    ]
}

/// Resolves an import string to a canonical path, or `None` if nothing matches.
///
/// The result is canonicalized — symlinks and `..` resolved — so two spellings
/// of the same file are the same module to [`LoadedModules`].
pub fn resolve(spec: &str, current_dir: Option<&Path>) -> Option<PathBuf> {
    search_dirs(current_dir)
        .iter()
        .flat_map(|base| candidates_in(base, spec))
        .find_map(|candidate| {
            if candidate.exists() {
                candidate.canonicalize().ok()
            } else {
                None
            }
        })
}

/// The set of modules already executed in this process.
///
/// A module runs once. The path is recorded **before** its body runs, which is
/// what makes import cycles terminate — the second import of a file already on
/// the stack is a no-op rather than infinite recursion.
#[derive(Debug, Default)]
pub struct LoadedModules {
    loaded: HashSet<PathBuf>,
}

impl LoadedModules {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `path` as loaded. Returns `true` if this is the first time —
    /// that is, if the caller should go on to execute it.
    pub fn mark(&mut self, path: &Path) -> bool {
        self.loaded.insert(path.to_path_buf())
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.loaded.contains(path)
    }
}

/// Where the evaluator is, in module terms, right now.
///
/// Three fields of `Evaluator` until M6.2. They belong together because the
/// import path treats them as one: `eval_import` saves `current_dir` and
/// `exports`, runs the module, and restores both — a push and a pop of the same
/// context. Splitting a saved-and-restored pair across a 44-field struct is what
/// made it possible to forget one.
///
/// `loaded` is not saved and restored, and that asymmetry is deliberate: a module
/// runs once *per program*, not once per importing scope, so the set has to
/// outlive the context switch that the other two participate in. Recorded here so
/// the difference reads as intentional.
#[derive(Debug, Default)]
pub struct ModuleContext {
    /// Canonical paths already loaded. Marked *before* a body runs, which is what
    /// makes import cycles terminate.
    pub loaded: LoadedModules,
    /// Directory of the file currently executing, for relative import resolution.
    pub current_dir: Option<PathBuf>,
    /// `Some` while executing an imported module, tracking the names it exports.
    /// `None` at the top level, where `export` has nothing to report to.
    pub exports: Option<HashSet<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sz_modules_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, body).expect("fixture must be writable");
    }

    #[test]
    fn the_extension_is_optional_and_sz_beats_szx() {
        let dir = temp_dir("ext");
        write(&dir.join("pref.sz"), "// sz\n");
        write(&dir.join("pref.szx"), "// szx\n");

        for spec in ["pref", "pref.sz"] {
            let found = resolve(spec, Some(&dir)).expect("must resolve");
            assert_eq!(
                found.file_name().unwrap(),
                "pref.sz",
                "`{spec}` must pick the .sz file"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_resolves_through_its_index() {
        let dir = temp_dir("index");
        write(&dir.join("pkg").join("index.sz"), "// index\n");

        let found = resolve("pkg", Some(&dir)).expect("must resolve");
        assert_eq!(found.file_name().unwrap(), "index.sz");
        assert_eq!(found.parent().unwrap().file_name().unwrap(), "pkg");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_beats_a_directory_of_the_same_name() {
        let dir = temp_dir("filewins");
        write(&dir.join("thing.sz"), "// file\n");
        write(&dir.join("thing").join("index.sz"), "// dir\n");

        let found = resolve("thing", Some(&dir)).expect("must resolve");
        assert_eq!(found.file_name().unwrap(), "thing.sz");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_importing_files_directory_is_searched_first() {
        // A closer base beats a better-matching name in a farther one: the whole
        // list of candidates is tried against base 1 before base 2 is consulted.
        let dir = temp_dir("order");
        let near = dir.join("near");
        write(&near.join("dual.sz"), "// near\n");

        let dirs = search_dirs(Some(&near));
        assert_eq!(dirs.first(), Some(&near), "the importing dir leads");
        assert!(
            dirs.len() >= 3,
            "cwd and cwd/packages always follow: {dirs:?}"
        );
        assert!(
            dirs.iter().any(|d| d.ends_with("packages")),
            "a packages/ base must be present: {dirs:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_matching_resolves_to_none() {
        let dir = temp_dir("missing");
        assert!(resolve("definitely_not_a_module_xyz", Some(&dir)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn candidates_are_tried_in_the_documented_order() {
        let base = Path::new("/base");
        let names: Vec<String> = candidates_in(base, "thing")
            .iter()
            .map(|p| {
                p.strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            names,
            vec![
                "thing.sz".to_string(),
                "thing.szx".to_string(),
                "thing/index.sz".to_string(),
                "thing/index.szx".to_string(),
            ]
        );
    }

    #[test]
    fn a_module_is_marked_once() {
        let mut loaded = LoadedModules::new();
        let path = Path::new("/some/module.sz");

        assert!(loaded.mark(path), "the first mark says: go execute it");
        assert!(!loaded.mark(path), "the second says: already done");
        assert!(loaded.contains(path));
    }
}
