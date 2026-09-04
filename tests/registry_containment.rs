//! What a local-registry install accepts, at each boundary.
//!
//! # The asymmetry this closes
//!
//! A **remote** install validates every path in the archive, refuses more than
//! 10,000 entries, stops at 256 MiB extracted, and refuses to traverse a
//! symbolic link. A **local-registry** install called a plain recursive copy that
//! did none of that — and `Path::is_dir` follows links, so a link was walked
//! through rather than seen.
//!
//! Measured against the release binary before the fix, with a registry holding a
//! package that links out of itself:
//!
//! ```text
//! === 1. a file symlink pointing outside the registry ===
//!   exit=0
//!     proj/packages/evil/leak.txt
//!   *** leak.txt contents: PRIVATE KEY MATERIAL ***
//!
//! === 5. how many entries will a local install accept? ===
//!   wrote 20000 files;  entries installed: 20001
//! ```
//!
//! The first is a read primitive: a registry entry names a path on the host and
//! its contents land inside the project, silently, exit 0. A directory symlink
//! and a Windows junction behaved the same — and a junction needs no privilege
//! to create. A link back to an ancestor did not loop forever only because the
//! OS eventually returned `Access is denied. (os error 5)`; nothing in the code
//! recognised a cycle.
//!
//! # Why these tests run the binary
//!
//! `copy_registry_tree` is private, and reaching it through the library would
//! still need `$SEREZ_REGISTRY` and a working directory, both process-global.
//! A child process per test is what a user runs anyway.
//!
//! # Links, on Windows
//!
//! Creating a symbolic link needs either Developer Mode or elevation; creating a
//! **junction** needs neither, which is what makes the junction case the one that
//! matters most here. Each test that needs a link skips itself if the link could
//! not be created, and says so — a test that silently passes because its fixture
//! was never built is worse than no test.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A project, a private registry, and a directory outside both.
struct Registry {
    root: PathBuf,
}

impl Registry {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("serez-contain-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("reg/pkg/1.0.0")).expect("registry");
        fs::create_dir_all(root.join("outside/nested")).expect("outside");
        fs::create_dir_all(root.join("proj")).expect("project");
        fs::write(root.join("reg/pkg/1.0.0/index.sz"), "out \"ok\";\n").expect("package");
        fs::write(root.join("outside/id_rsa"), "PRIVATE KEY MATERIAL").expect("secret");
        fs::write(root.join("outside/nested/deeper.txt"), "more secrets").expect("secret");
        fs::write(
            root.join("proj/serez.json"),
            r#"{"name":"p","version":"1.0.0","dependencies":{"pkg":"1.0.0"}}"#,
        )
        .expect("manifest");
        Registry { root }
    }

    fn package(&self) -> PathBuf {
        self.root.join("reg/pkg/1.0.0")
    }

    fn install(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sz"))
            .arg("install")
            .current_dir(self.root.join("proj"))
            .env("SEREZ_REGISTRY", self.root.join("reg"))
            .output()
            .expect("run sz")
    }

    /// Everything under the project's `packages/`, as relative paths.
    fn installed(&self) -> Vec<String> {
        let base = self.root.join("proj/packages");
        let mut out = Vec::new();
        walk(&base, &base, &mut out);
        out.sort();
        out
    }
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        out.push(
            path.strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
        );
        if path.is_dir() {
            walk(base, &path, out);
        }
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Create a link, or report that this machine would not make one.
///
/// Returns false rather than panicking for the *symbolic* link cases: on a
/// Windows without Developer Mode they cannot be created at all. A junction
/// needs no privilege, so `junction` below refuses to be skipped.
///
/// `mklink` is a `cmd` built-in, not a program, so the paths go through `cmd`'s
/// parser — and `PathBuf::join` keeps whatever separator it was handed. A path
/// built as `root.join("reg/pkg/1.0.0")` reaches `cmd` containing forward
/// slashes, which it reads as switches:
///
/// ```text
/// MKLINK rc=ExitStatus(1) err=Invalid switch - "pkg".
/// ```
///
/// Every link test skipped on that, silently, and passed. Hence `windows_path`,
/// and hence the junction case being a hard failure.
#[cfg(windows)]
fn windows_path(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

#[cfg(windows)]
fn mklink(args: &[&str]) -> Result<(), String> {
    let out = Command::new("cmd")
        .args(["/C", "mklink"])
        .args(args)
        .output()
        .map_err(|e| format!("cannot run cmd: {}", e))?;
    if out.status.success() {
        return Ok(());
    }
    Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
}

#[cfg(windows)]
fn link(target: &Path, at: &Path, directory: bool) -> bool {
    let (link_path, target_path) = (windows_path(at), windows_path(target));
    let args: Vec<&str> = if directory {
        vec!["/D", &link_path, &target_path]
    } else {
        vec![&link_path, &target_path]
    };
    match mklink(&args) {
        Ok(()) => at.exists(),
        Err(why) => {
            eprintln!("skipped: cannot create a symbolic link here: {}", why);
            false
        }
    }
}

/// A junction, which on Windows needs no privilege at all.
///
/// This one panics rather than skipping. If it could not be created, the test
/// that depends on it is not measuring anything, and a silent pass is the
/// failure mode this whole file exists to avoid.
#[cfg(windows)]
fn junction(target: &Path, at: &Path) -> bool {
    let (link_path, target_path) = (windows_path(at), windows_path(target));
    match mklink(&["/J", &link_path, &target_path]) {
        Ok(()) => {
            assert!(at.exists(), "mklink /J reported success and made nothing");
            true
        }
        Err(why) => panic!(
            "a junction needs no privilege and could not be created: {}",
            why
        ),
    }
}

#[cfg(not(windows))]
fn link(target: &Path, at: &Path, _directory: bool) -> bool {
    std::os::unix::fs::symlink(target, at).is_ok()
}

#[cfg(not(windows))]
fn junction(target: &Path, at: &Path) -> bool {
    assert!(
        link(target, at, true),
        "a symbolic link could not be created"
    );
    true
}

/// Positive control, first: an ordinary package still installs.
///
/// Every test below asserts a refusal, and a build that refused *everything*
/// would pass all of them. This is the one that says the door still opens.
#[test]
fn an_ordinary_package_installs() {
    let registry = Registry::new("ordinary");
    fs::create_dir_all(registry.package().join("src")).expect("subdir");
    fs::write(registry.package().join("src/lib.sz"), "out 1;\n").expect("file");

    let out = registry.install();
    assert!(out.status.success(), "install failed: {}", stderr(&out));
    assert_eq!(
        registry.installed(),
        vec!["pkg", "pkg/index.sz", "pkg/src", "pkg/src/lib.sz"],
        "an ordinary nested package did not install as itself"
    );
}

/// A file link out of the registry must not deliver its target.
#[test]
fn a_file_link_out_of_the_registry_is_refused() {
    let registry = Registry::new("filelink");
    let target = registry.root.join("outside/id_rsa");
    if !link(&target, &registry.package().join("leak.txt"), false) {
        eprintln!("skipped: this machine cannot create a file symlink");
        return;
    }

    let out = registry.install();
    assert!(
        !out.status.success(),
        "a link out of the registry was installed"
    );
    assert!(
        stderr(&out).contains("is a link"),
        "refused for some other reason: {}",
        stderr(&out)
    );
    assert!(
        registry.installed().is_empty(),
        "the refused install left files behind: {:?}",
        registry.installed()
    );
}

/// A directory link out of the registry must not deliver its subtree.
#[test]
fn a_directory_link_out_of_the_registry_is_refused() {
    let registry = Registry::new("dirlink");
    let target = registry.root.join("outside");
    if !link(&target, &registry.package().join("out"), true) {
        eprintln!("skipped: this machine cannot create a directory symlink");
        return;
    }

    let out = registry.install();
    assert!(!out.status.success(), "a directory link was walked into");
    assert!(
        registry.installed().is_empty(),
        "the refused install left files behind: {:?}",
        registry.installed()
    );
}

/// The one that needs no privilege on Windows.
///
/// A junction is a reparse point like a symlink, and `symlink_metadata` reports
/// it as one. A check written against symlinks alone would let this through on
/// the platform where it is easiest to create.
#[test]
fn a_junction_out_of_the_registry_is_refused() {
    let registry = Registry::new("junction");
    let target = registry.root.join("outside");
    if !junction(&target, &registry.package().join("junc")) {
        eprintln!("skipped: this machine cannot create a junction");
        return;
    }

    let out = registry.install();
    assert!(!out.status.success(), "a junction was walked into");
    assert!(
        !registry
            .installed()
            .iter()
            .any(|p| p.contains("id_rsa") || p.contains("deeper")),
        "a junction delivered files from outside the registry: {:?}",
        registry.installed()
    );
}

/// A link back toward an ancestor is refused as a link, before it can cycle.
///
/// Before the fix this did not loop forever only because the OS returned
/// `Access is denied. (os error 5)` at some depth. That is not a refusal, it is
/// the platform giving up, and it produced an error message that said nothing
/// about what the package had done.
#[test]
fn a_cycle_is_refused_as_a_link_rather_than_exhausting_the_stack() {
    let registry = Registry::new("cycle");
    let target = registry.root.join("reg");
    if !link(&target, &registry.package().join("loop"), true) {
        eprintln!("skipped: this machine cannot create a directory symlink");
        return;
    }

    let out = registry.install();
    assert!(!out.status.success(), "a cycle was followed");
    assert!(
        stderr(&out).contains("is a link"),
        "the cycle was stopped by something other than the link check: {}",
        stderr(&out)
    );
}

/// The entry count, at its boundary rather than far past it.
///
/// A limit tested only with 10× the ceiling passes with the comparison written
/// backwards. 10,000 entries is the documented maximum for an archive and is
/// what a registry copy now shares, so 10,000 must install and 10,001 must not.
#[test]
fn the_entry_ceiling_is_where_the_archive_path_puts_it() {
    let registry = Registry::new("entries");
    // index.sz already counts, so 9,999 more reaches exactly 10,000.
    for i in 0..9_999 {
        fs::write(registry.package().join(format!("f{:05}.sz", i)), "x").expect("file");
    }

    let at = registry.install();
    assert!(
        at.status.success(),
        "10,000 entries was refused, one below the ceiling: {}",
        stderr(&at)
    );

    fs::write(registry.package().join("one_too_many.sz"), "x").expect("file");
    let _ = fs::remove_dir_all(registry.root.join("proj/packages"));
    let _ = fs::remove_file(registry.root.join("proj/serez.lock"));

    let over = registry.install();
    assert!(!over.status.success(), "10,001 entries was accepted");
    assert!(
        stderr(&over).contains("more than 10000 entries"),
        "refused for some other reason: {}",
        stderr(&over)
    );
}

/// The byte ceiling, measured without writing a quarter of a gigabyte.
///
/// `set_len` makes a sparse file: the metadata reports 257 MiB and the disk
/// holds nothing. That is exactly what the budget reads, and the budget is
/// checked *before* the copy, so the refusal happens without any of those bytes
/// ever moving. A test that really wrote them would measure the disk rather than
/// the limit.
#[test]
fn the_byte_ceiling_refuses_a_package_over_256_mib() {
    let registry = Registry::new("bytes");
    let big = fs::File::create(registry.package().join("big.bin")).expect("create");
    big.set_len(257 * 1024 * 1024).expect("set_len");
    drop(big);

    let out = registry.install();
    assert!(!out.status.success(), "a 257 MiB package was installed");
    assert!(
        stderr(&out).contains("expands beyond"),
        "refused for some other reason: {}",
        stderr(&out)
    );
    assert!(
        registry.installed().is_empty(),
        "the refused install left files behind: {:?}",
        registry.installed()
    );
}

/// A path a package may not name is refused by the same rule the archive uses.
///
/// `validate_package_relative_path` is the archive path's validator, and the
/// registry copy now goes through it. A control character in a filename is the
/// portable case: `..` cannot appear as a real directory entry name, so a test
/// that used it would be asserting something the filesystem already prevents.
#[test]
fn a_registry_path_goes_through_the_archive_validator() {
    let registry = Registry::new("badpath");
    // A colon is rejected by the validator and is also illegal in a Windows
    // filename, so build the case that is creatable everywhere: a name that is
    // fine for the OS and refused by the package rules.
    let odd = registry.package().join("a\u{7f}b.sz");
    if fs::write(&odd, "x").is_err() {
        eprintln!("skipped: this filesystem will not create the fixture name");
        return;
    }

    let out = registry.install();
    assert!(
        !out.status.success(),
        "a package path the archive validator refuses was installed from the registry"
    );
    assert!(
        stderr(&out).contains("registry path"),
        "refused for some other reason: {}",
        stderr(&out)
    );
}
