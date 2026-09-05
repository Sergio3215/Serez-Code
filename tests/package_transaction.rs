//! Package, manifest and lockfile as one recoverable transaction.
//!
//! # The decision
//!
//! The logical unit of installation is the package tree **plus** its
//! `serez.json` line **plus** its `serez.lock` line. All three represent one
//! confirmed state. What that has to mean in practice:
//!
//! - a failure before the commit leaves the previous state;
//! - no new package active with old metadata;
//! - no manifest updated against an old lockfile;
//! - no lockfile line without the install it describes;
//! - an interruption between filesystem operations is repaired deterministically
//!   on the next run.
//!
//! # What was there before
//!
//! The package tree was transactional and the metadata was not: two writes after
//! a committed install, each of which printed a warning on failure and carried
//! on. A crash between them left exactly the states the decision forbids.
//!
//! # Atomic is not claimed
//!
//! Three paths change and no filesystem commits three paths at once. What is
//! implemented is a **write-ahead journal**: every byte of the target state is
//! recorded before the first mutation, and the next run finishes the job. The
//! window between the first rename and the last is real; it is *recoverable*,
//! not absent, and the tests below say which is which.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A project with a private registry, driven through the real binary.
struct Project {
    dir: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("serez-txn-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("reg/test-pkg/1.0.0")).expect("registry");
        fs::write(dir.join("reg/test-pkg/1.0.0/index.sz"), "out \"v1\";\n").expect("v1");
        fs::create_dir_all(dir.join("reg/test-pkg/2.0.0")).expect("registry");
        fs::write(dir.join("reg/test-pkg/2.0.0/index.sz"), "out \"v2\";\n").expect("v2");
        fs::create_dir_all(dir.join("reg/other/1.0.0")).expect("registry");
        fs::write(dir.join("reg/other/1.0.0/index.sz"), "out \"other\";\n").expect("other");
        fs::write(
            dir.join("serez.json"),
            r#"{"name":"txn","version":"1.0.0","dependencies":{"test-pkg":"1.0.0"}}"#,
        )
        .expect("manifest");
        Project { dir }
    }

    fn sz(&self, args: &[&str]) -> Output {
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
        fs::read_to_string(self.path(rel)).unwrap_or_default()
    }

    fn exists(&self, rel: &str) -> bool {
        Path::new(&self.path(rel)).exists()
    }

    /// The lockfile's entry for `name`, if any.
    fn locked(&self, name: &str) -> Option<(String, String)> {
        self.read("serez.lock")
            .lines()
            .find(|l| l.starts_with(&format!("{name}\t")))
            .map(|l| {
                let f: Vec<&str> = l.split('\t').collect();
                (f[1].to_string(), f[2].to_string())
            })
    }

    /// The invariant the decision is about: every lockfile line has its package,
    /// and every installed package has its line.
    fn assert_consistent(&self) {
        for line in self.read("serez.lock").lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let name = line.split('\t').next().unwrap_or("");
            assert!(
                self.exists(&format!("packages/{name}")),
                "serez.lock records '{name}' and packages/{name} does not exist"
            );
        }
        let Ok(entries) = fs::read_dir(self.path("packages")) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.contains(".replaced-") {
                continue;
            }
            assert!(
                self.locked(&name).is_some(),
                "packages/{name} is installed and serez.lock does not record it"
            );
        }
        assert!(
            !self.exists(".serez-install.journal"),
            "a completed install left its journal behind"
        );
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

// ── 1. a new install ────────────────────────────────────────────────────────

#[test]
fn a_new_install_commits_all_three() {
    let project = Project::new("new");
    let out = project.sz(&["install"]);
    assert!(out.status.success(), "install failed: {}", stderr(&out));

    assert!(project.exists("packages/test-pkg/index.sz"), "no package");
    let (version, integrity) = project.locked("test-pkg").expect("no lockfile entry");
    assert_eq!(version, "1.0.0");
    assert!(integrity.starts_with("sha256-"), "no digest: {integrity}");
    assert!(
        project.read("serez.json").contains("test-pkg"),
        "the manifest lost its dependency"
    );
    project.assert_consistent();
}

// ── 2. an update ────────────────────────────────────────────────────────────

#[test]
fn an_update_moves_all_three_together() {
    let project = Project::new("update");
    assert!(project.sz(&["install"]).status.success());
    let (_, first) = project.locked("test-pkg").expect("first install");

    let out = project.sz(&["install", "test-pkg@2.0.0"]);
    assert!(out.status.success(), "update failed: {}", stderr(&out));

    assert_eq!(
        project.read("packages/test-pkg/index.sz"),
        "out \"v2\";\n",
        "the package tree is still the old version"
    );
    let (version, second) = project.locked("test-pkg").expect("after update");
    assert_eq!(
        version, "2.0.0",
        "the lockfile still records the old version"
    );
    assert_ne!(first, second, "the digest did not change with the tree");
    assert!(
        project.read("serez.json").contains("2.0.0"),
        "the manifest still records the old version"
    );
    project.assert_consistent();
}

// ── 3. a failure before the commit ──────────────────────────────────────────

/// A package the registry does not have: nothing changes.
#[test]
fn a_failure_before_the_commit_leaves_the_previous_state() {
    let project = Project::new("prefail");
    assert!(project.sz(&["install"]).status.success());
    let manifest = project.read("serez.json");
    let lock = project.read("serez.lock");

    let out = project.sz(&["install", "no-such-pkg@1.0.0"]);
    assert!(!out.status.success(), "a missing package installed");

    assert_eq!(project.read("serez.json"), manifest, "the manifest moved");
    assert_eq!(project.read("serez.lock"), lock, "the lockfile moved");
    assert!(
        !project.exists("packages/no-such-pkg"),
        "a failed install left a package tree"
    );
    project.assert_consistent();
}

/// An integrity mismatch: the same, with a package already installed.
#[test]
fn a_refused_integrity_check_leaves_all_three_alone() {
    let project = Project::new("integrity");
    assert!(project.sz(&["install"]).status.success());
    let manifest = project.read("serez.json");
    let lock = project.read("serez.lock");

    fs::write(
        project.path("reg/test-pkg/1.0.0/index.sz"),
        "out \"tampered\";\n",
    )
    .expect("tamper");

    let out = project.sz(&["install"]);
    assert!(!out.status.success(), "a tampered package installed");
    assert!(
        stderr(&out).contains("integrity check failed"),
        "refused for some other reason: {}",
        stderr(&out)
    );
    assert_eq!(project.read("serez.json"), manifest);
    assert_eq!(project.read("serez.lock"), lock);
    assert_eq!(
        project.read("packages/test-pkg/index.sz"),
        "out \"v1\";\n",
        "the installed copy was replaced by the tampered one"
    );
    project.assert_consistent();
}

// ── 4. a failure while writing metadata ─────────────────────────────────────

/// A manifest that does not parse fails **before** the package tree moves.
///
/// This is the ordering the decision asks for: the manifest's new text is worked
/// out while nothing has changed, so a manifest problem cannot leave a new
/// package beside old metadata. Before, `record_dependency` ran after the swap
/// and printed a warning.
#[test]
fn a_broken_manifest_fails_before_the_package_moves() {
    let project = Project::new("badmanifest");
    assert!(project.sz(&["install"]).status.success());

    fs::write(project.path("serez.json"), "{ this is not json").expect("break it");

    let out = project.sz(&["install", "other@1.0.0"]);
    assert!(
        !out.status.success(),
        "an install proceeded past an unparseable manifest"
    );
    assert!(
        !project.exists("packages/other"),
        "the package tree moved even though the manifest could not be written"
    );
    assert!(
        project.locked("other").is_none(),
        "the lockfile recorded a package that was not installed"
    );
    assert!(
        !project.exists(".serez-install.journal"),
        "a failure before the journal was written left one behind"
    );
}

// ── 5. recovery from an interrupted commit ──────────────────────────────────

/// A journal left by a crash is completed on the next run, from the journal
/// alone.
///
/// The crash is reconstructed rather than caused: the state is built exactly as
/// an interruption **between the package swap and the lockfile write** would
/// leave it — package present, lockfile missing its line, journal describing the
/// rest — and then an ordinary command is run. That is what "deterministic
/// recovery on the next run" means, and it is testable without killing a process
/// at a precise instruction.
///
/// The journalled lockfile is a **real** one, produced by a real install, rather
/// than hand-built. A fixture that invented a digest would leave the recovered
/// lockfile describing a tree that does not exist, and the next install would
/// refuse it on integrity — which is what the first version of this test did,
/// and is the right answer to the wrong question.
#[test]
fn an_interrupted_commit_is_completed_on_the_next_run() {
    let project = Project::new("recover");
    assert!(project.sz(&["install"]).status.success());

    // The state the interrupted run was heading for, learned by getting there.
    let target_lock = project.read("serez.lock");
    assert!(target_lock.contains("test-pkg"), "fixture precondition");

    // Now unwind to the moment before the lockfile write: the package is in
    // place, the lockfile is not.
    fs::remove_file(project.path("serez.lock")).expect("unwind the lockfile");
    write_journal(&project, "test-pkg", "1.0.0", &target_lock, None);

    let out = project.sz(&["install"]);
    assert!(out.status.success(), "recovery failed: {}", stderr(&out));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("Completed an interrupted install"),
        "recovery happened silently: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    assert_eq!(
        project.read("serez.lock"),
        target_lock,
        "the lockfile was not brought up to the journalled state"
    );
    assert!(
        !project.exists(".serez-install.journal"),
        "the journal survived its own recovery"
    );
    project.assert_consistent();
}

/// The manifest half of the same window.
///
/// A crash after the lockfile write and before the manifest write leaves the
/// state the decision names explicitly — "no debe quedar lockfile actualizado
/// sin la instalación correspondiente" and its mirror. Recovery writes the
/// recorded manifest, so the three end up describing one state.
#[test]
fn a_crash_before_the_manifest_write_is_finished_too() {
    let project = Project::new("manifesthalf");
    assert!(project.sz(&["install", "other@1.0.0"]).status.success());

    let target_lock = project.read("serez.lock");
    let target_manifest = project.read("serez.json");
    assert!(target_manifest.contains("other"), "fixture precondition");

    // Unwind only the manifest: package and lockfile committed, manifest not.
    fs::write(
        project.path("serez.json"),
        r#"{"name":"txn","version":"1.0.0","dependencies":{"test-pkg":"1.0.0"}}"#,
    )
    .expect("unwind the manifest");
    write_journal(
        &project,
        "other",
        "1.0.0",
        &target_lock,
        Some(&target_manifest),
    );

    let out = project.sz(&["install"]);
    assert!(out.status.success(), "recovery failed: {}", stderr(&out));
    assert_eq!(
        project.read("serez.json"),
        target_manifest,
        "the manifest was not brought up to the journalled state"
    );
    project.assert_consistent();
}

/// Recovery is idempotent: running it twice is running it once.
#[test]
fn recovery_can_be_repeated() {
    let project = Project::new("idempotent");
    assert!(project.sz(&["install"]).status.success());
    let target_lock = project.read("serez.lock");

    fs::remove_file(project.path("serez.lock")).expect("unwind");
    write_journal(&project, "test-pkg", "1.0.0", &target_lock, None);
    assert!(project.sz(&["install"]).status.success());
    let after_first = project.read("serez.lock");

    // The same journal, applied to the state its first application produced.
    write_journal(&project, "test-pkg", "1.0.0", &target_lock, None);
    assert!(project.sz(&["install"]).status.success());
    assert_eq!(
        project.read("serez.lock"),
        after_first,
        "applying the same journal twice produced a different state"
    );
    project.assert_consistent();
}

/// A journal cut short by a crash *while it was being written* is discarded.
///
/// Safe precisely because it is written before the first mutation: a torn record
/// means nothing happened, so there is nothing to finish. Rolling it forward on
/// a guess is what would be dangerous.
#[test]
fn a_torn_journal_is_discarded_rather_than_guessed_at() {
    let project = Project::new("torn");
    assert!(project.sz(&["install"]).status.success());
    let lock = project.read("serez.lock");
    let manifest = project.read("serez.json");

    // A complete journal, truncated before its terminator.
    let target = lock.replace("1.0.0", "2.0.0");
    write_journal(&project, "test-pkg", "2.0.0", &target, None);
    let full = fs::read(project.path(".serez-install.journal")).expect("journal");
    fs::write(
        project.path(".serez-install.journal"),
        &full[..full.len() - 12],
    )
    .expect("truncate");

    let out = project.sz(&["install"]);
    assert!(
        out.status.success(),
        "a torn journal broke the run: {}",
        stderr(&out)
    );
    assert_eq!(
        project.read("serez.lock"),
        lock,
        "a torn journal was applied anyway"
    );
    assert_eq!(project.read("serez.json"), manifest);
    assert!(
        !project.exists(".serez-install.journal"),
        "the torn journal was left in place"
    );
}

/// A journal whose digest does not match the body is discarded too.
#[test]
fn a_corrupted_journal_is_discarded() {
    let project = Project::new("corrupt");
    assert!(project.sz(&["install"]).status.success());
    let lock = project.read("serez.lock");

    write_journal(
        &project,
        "test-pkg",
        "2.0.0",
        &lock.replace("1.0.0", "2.0.0"),
        None,
    );
    let mut bytes = fs::read(project.path(".serez-install.journal")).expect("journal");
    // Flip a byte in the recorded version, leaving the terminator intact.
    let at = bytes
        .windows(7)
        .position(|w| w == b"version")
        .expect("version line")
        + 8;
    bytes[at] = b'9';
    fs::write(project.path(".serez-install.journal"), &bytes).expect("corrupt");

    assert!(project.sz(&["install"]).status.success());
    assert_eq!(
        project.read("serez.lock"),
        lock,
        "a journal whose digest did not match was applied"
    );
}

// ── 6. install_all with no lockfile ─────────────────────────────────────────

/// The path the decision names explicitly: manifest present, lockfile absent.
#[test]
fn install_all_without_a_lockfile_produces_a_reproducible_one() {
    let project = Project::new("nolock");
    assert!(!project.exists("serez.lock"), "fixture precondition");

    let out = project.sz(&["install"]);
    assert!(out.status.success(), "install failed: {}", stderr(&out));

    let (version, integrity) = project.locked("test-pkg").expect("no lockfile written");
    assert_eq!(version, "1.0.0");
    assert!(integrity.starts_with("sha256-"));
    project.assert_consistent();

    // Reproducible: the same manifest and registry produce the same bytes.
    let again = Project::new("nolock2");
    assert!(again.sz(&["install"]).status.success());
    assert_eq!(
        again.read("serez.lock"),
        project.read("serez.lock"),
        "two projects with the same inputs produced different lockfiles"
    );
}

/// A failure part-way through `install_all` leaves the packages it did install
/// fully consistent, and the rest untouched.
#[test]
fn a_partial_install_all_is_consistent_as_far_as_it_got() {
    let project = Project::new("partial");
    fs::write(
        project.path("serez.json"),
        r#"{"name":"txn","version":"1.0.0","dependencies":{"other":"1.0.0","missing":"9.9.9"}}"#,
    )
    .expect("manifest");

    let out = project.sz(&["install"]);
    assert!(!out.status.success(), "a missing dependency installed");

    // Whatever it managed is complete; nothing is half-recorded.
    project.assert_consistent();
    assert!(
        project.locked("missing").is_none(),
        "the lockfile recorded a package that was never installed"
    );
}

// ── 7. integrity ────────────────────────────────────────────────────────────

#[test]
fn the_recorded_digest_is_the_installed_trees_digest() {
    let project = Project::new("digest");
    assert!(project.sz(&["install"]).status.success());
    let (_, first) = project.locked("test-pkg").expect("entry");

    // A different tree must produce a different digest, or the field is
    // decoration.
    let other = Project::new("digest2");
    fs::write(
        other.path("reg/test-pkg/1.0.0/index.sz"),
        "out \"different\";\n",
    )
    .expect("vary");
    assert!(other.sz(&["install"]).status.success());
    let (_, second) = other.locked("test-pkg").expect("entry");

    assert_ne!(first, second, "two different trees hashed the same");
}

// ── 8. idempotence ──────────────────────────────────────────────────────────

#[test]
fn installing_twice_changes_nothing_the_second_time() {
    let project = Project::new("twice");
    assert!(project.sz(&["install"]).status.success());
    let manifest = project.read("serez.json");
    let lock = project.read("serez.lock");
    let tree = project.read("packages/test-pkg/index.sz");

    let out = project.sz(&["install"]);
    assert!(
        out.status.success(),
        "the second install failed: {}",
        stderr(&out)
    );

    assert_eq!(project.read("serez.json"), manifest, "the manifest churned");
    assert_eq!(project.read("serez.lock"), lock, "the lockfile churned");
    assert_eq!(project.read("packages/test-pkg/index.sz"), tree);
    project.assert_consistent();
}

// ── the fixture that reconstructs a crash ───────────────────────────────────

/// Write a journal in the on-disk format, as an interrupted install would leave.
///
/// Built here rather than exported from the crate on purpose: a test that used
/// the encoder to check the decoder would pass against a format that agrees with
/// itself and nothing else. This spells the bytes out, so the two have to agree
/// with the *format* rather than with each other.
fn write_journal(
    project: &Project,
    package: &str,
    version: &str,
    lockfile: &str,
    manifest: Option<&str>,
) {
    let dest = project.path(&format!("packages/{package}"));
    let staging = project.path(&format!("packages/.{package}.staging-0"));
    let lock_path = project.path("serez.lock");
    let manifest_path = project.path("serez.json");

    let mut body = String::new();
    body.push_str("serez-install-journal/1\n");
    body.push_str(&format!("package\t{package}\n"));
    body.push_str(&format!("version\t{version}\n"));
    body.push_str(
        "integrity\tsha256-0000000000000000000000000000000000000000000000000000000000000000\n",
    );

    body.push_str(&format!("destination\t{}\n", dest.display()));
    body.push_str(&format!("staging\t{}\n", staging.display()));
    body.push_str(&format!(
        "file\t{}\t{}\n{lockfile}\n",
        lockfile.len(),
        lock_path.display()
    ));
    if let Some(manifest) = manifest {
        body.push_str(&format!(
            "file\t{}\t{}\n{manifest}\n",
            manifest.len(),
            manifest_path.display()
        ));
    }

    let digest = sha256_hex(body.as_bytes());
    body.push_str(&format!("end\t{digest}\n"));
    fs::write(project.path(".serez-install.journal"), body).expect("journal");
}

/// SHA-256, spelled out here for the same reason the journal format is.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(value);
        }
    }
    h.iter().map(|w| format!("{:08x}", w)).collect()
}
