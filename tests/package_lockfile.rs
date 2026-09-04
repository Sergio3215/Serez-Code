//! `serez.lock` is written by the install that ordinary projects actually run.
//!
//! # The defect this pins
//!
//! `install_package` took a `record: bool` that gated two unrelated records: the
//! dependency line in `serez.json`, and the integrity line in `serez.lock`.
//!
//! `install_all` — what a bare `sz install` runs, and therefore what a fresh
//! clone and CI run — reads its dependencies *from* the manifest, so it rightly
//! did not want to write them back. It passed `record: false`, and that took the
//! lockfile with it. The result was that the most common install path produced no
//! lockfile at all, so the next install had nothing to verify against and the
//! integrity check never fired. The guarantee existed and was unreachable.
//!
//! Measured before the fix, in a project whose only dependency came from
//! `serez.json`:
//!
//! ```text
//! ✅ Installed test-pkg@1.0.0 → ./packages/test-pkg
//! ls: cannot access 'serez.lock': No such file or directory
//! ```
//!
//! # Why these tests run the binary
//!
//! `install_all` reads `std::env::current_dir()` and `$SEREZ_REGISTRY`, both
//! process-global. Calling it in-process would make these tests order-dependent
//! on each other and on every other test in the binary. Each test here gets its
//! own child process, which is also the thing a user runs.
//!
//! # What is asserted, positive and negative
//!
//! Writing the file is the cheap half. A lockfile that is written but never
//! consulted would pass a test that only stats the path, so the negative control
//! tampers with the registry *after* a successful install and requires the second
//! install to refuse — and requires the already-installed copy to survive that
//! refusal. That is the property the lockfile exists for; the file's presence is
//! only the mechanism.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A project directory with a manifest, a private registry, and no lockfile.
///
/// Each test builds its own registry rather than sharing `tests/registry`,
/// because the negative control has to modify one, and a shared fixture cannot be
/// modified without breaking every other test in the suite.
struct Project {
    dir: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("serez-lockfile-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("reg/test-pkg/1.0.0")).expect("create registry");
        fs::write(dir.join("reg/test-pkg/1.0.0/index.sz"), "out \"v1\";\n").expect("write package");
        fs::write(
            dir.join("serez.json"),
            r#"{"name":"lockfile-fixture","version":"1.0.0","dependencies":{"test-pkg":"1.0.0"}}"#,
        )
        .expect("write manifest");
        Project { dir }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sz"))
            .args(args)
            .current_dir(&self.dir)
            .env("SEREZ_REGISTRY", self.dir.join("reg"))
            .output()
            .expect("run sz")
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.path(rel)).unwrap_or_else(|e| panic!("read {}: {}", rel, e))
    }

    fn exists(&self, rel: &str) -> bool {
        Path::new(&self.path(rel)).exists()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The regression itself: a bare `sz install` must leave a lockfile behind.
///
/// This is the test that failed before the fix. It passed the install and failed
/// the lockfile, which is exactly the shape of the defect — the package arrives,
/// and the record of what arrived does not.
#[test]
fn install_all_writes_the_lockfile() {
    let project = Project::new("writes");
    let out = project.run(&["install"]);

    assert!(out.status.success(), "install failed: {}", stderr(&out));
    assert!(
        project.exists("packages/test-pkg/index.sz"),
        "the package itself was not installed"
    );
    assert!(
        project.exists("serez.lock"),
        "no serez.lock after `sz install` — the integrity record the next install \
         verifies against was never written"
    );

    let lock = project.read("serez.lock");
    let entry = lock
        .lines()
        .find(|l| l.starts_with("test-pkg\t"))
        .unwrap_or_else(|| panic!("no test-pkg entry in serez.lock:\n{}", lock));
    let fields: Vec<&str> = entry.split('\t').collect();
    assert_eq!(fields.len(), 3, "malformed lock entry: {:?}", entry);
    assert_eq!(fields[1], "1.0.0", "wrong version recorded");
    assert!(
        fields[2].starts_with("sha256-") && fields[2].len() > "sha256-".len(),
        "the integrity field is not a digest: {:?}",
        fields[2]
    );
}

/// The other half of the split: `install_all` still must not rewrite the manifest.
///
/// Without this, "always write the lockfile" could be implemented by making
/// `install_all` record everything, which would reformat a hand-written
/// `serez.json` on every install of an unchanged project.
#[test]
fn install_all_leaves_the_manifest_alone() {
    let project = Project::new("manifest");
    let before = project.read("serez.json");

    let out = project.run(&["install"]);
    assert!(out.status.success(), "install failed: {}", stderr(&out));

    assert_eq!(
        before,
        project.read("serez.json"),
        "`sz install` rewrote serez.json; its dependencies came from there"
    );
}

/// Negative control: the lockfile is consulted, not merely produced.
///
/// The registry is changed underneath an installed package. The recorded digest
/// no longer matches what the registry now offers, so the second install must
/// refuse — and must leave the good copy in place rather than half-replacing it.
#[test]
fn a_recorded_digest_refuses_a_changed_package() {
    let project = Project::new("tamper");

    let first = project.run(&["install"]);
    assert!(
        first.status.success(),
        "first install failed: {}",
        stderr(&first)
    );
    assert!(
        project.exists("serez.lock"),
        "no lockfile to verify against"
    );

    fs::write(
        project.path("reg/test-pkg/1.0.0/index.sz"),
        "out \"tampered\";\n",
    )
    .expect("tamper with the registry");

    let second = project.run(&["install"]);
    assert!(
        !second.status.success(),
        "a changed package was installed over a recorded digest"
    );
    assert!(
        stderr(&second).contains("integrity check failed"),
        "refused for some other reason: {}",
        stderr(&second)
    );
    assert_eq!(
        project.read("packages/test-pkg/index.sz"),
        "out \"v1\";\n",
        "the refused install damaged the copy that was already correct"
    );
}

/// Positive control for the negative one: an *unchanged* package reinstalls.
///
/// Without this, `a_recorded_digest_refuses_a_changed_package` would also pass if
/// the second install always failed, for any reason at all.
#[test]
fn an_unchanged_package_reinstalls_cleanly() {
    let project = Project::new("stable");

    let first = project.run(&["install"]);
    assert!(
        first.status.success(),
        "first install failed: {}",
        stderr(&first)
    );
    let recorded = project.read("serez.lock");

    let second = project.run(&["install"]);
    assert!(
        second.status.success(),
        "reinstalling an unchanged package failed: {}",
        stderr(&second)
    );
    assert_eq!(
        recorded,
        project.read("serez.lock"),
        "the lockfile changed without the package changing"
    );
}

/// The explicit form still records both, which is what `ManifestPolicy::Record` is.
#[test]
fn installing_a_named_package_records_it_in_both() {
    let project = Project::new("named");
    fs::write(
        project.path("serez.json"),
        r#"{"name":"lockfile-fixture","version":"1.0.0","dependencies":{}}"#,
    )
    .expect("empty the dependencies");

    let out = project.run(&["install", "test-pkg@1.0.0"]);
    assert!(out.status.success(), "install failed: {}", stderr(&out));

    assert!(
        project.read("serez.json").contains("test-pkg"),
        "`sz install <pkg>` did not add the dependency to serez.json"
    );
    assert!(
        project.read("serez.lock").contains("test-pkg\t1.0.0\t"),
        "`sz install <pkg>` did not record the dependency in serez.lock"
    );
}
