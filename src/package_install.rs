//! Installing a package without being able to half-install one.
//!
//! # The three properties, and what was missing
//!
//! `MATURITY_AUDIT.md` carried "non-atomic package installation; no lockfile,
//! integrity or signature policy" as **high, open**. The decision asks for the
//! first three; a publisher-signature policy is deliberately a separate
//! question and nothing here anticipates it.
//!
//! **Atomic.** `install_package` used to `remove_dir_all(&dest)` *before*
//! fetching the new version, so a download that failed afterwards left the
//! project with no package at all — the old one deleted and the new one never
//! written. Installation now happens in a staging directory beside the
//! destination and becomes visible in a rename; a failure at any point before
//! that leaves the project exactly as it was.
//!
//! **Reproducible.** `serez.lock` records the resolved graph as
//! `name <TAB> version <TAB> integrity`, sorted by name, so the same manifest
//! resolves to the same bytes on another machine and a diff of the lockfile is
//! readable.
//!
//! **Verified.** The integrity of what was staged is checked against the
//! lockfile *before* the rename that commits it. A mismatch aborts with the
//! project untouched.
//!
//! # Why the digest is of the installed tree
//!
//! A remote install has an archive to hash and a local-registry install does
//! not — it copies a directory. Hashing the **tree** covers both with one notion
//! of integrity rather than two, and it verifies what actually landed rather
//! than what was supposed to have landed. For a remote install the two are the
//! same statement, because extraction is a pure function of the archive.
//!
//! The digest covers relative paths *and* contents, both in a fixed order, so it
//! is identical across platforms and is not fooled by a file being renamed.
//!
//! # No new dependency
//!
//! SHA-256 already exists in this repository, because `Crypto.sha256` needs it.
//! It is reused rather than adding a crate — `DEVELOPMENT.md`'s "minimal runtime
//! dependencies" invariant — and reusing it also means the hash a package is
//! verified with is the same one the language exposes.
//!
//! It lives in `crate::hash`, a leaf, and it had to be moved there: reaching
//! into `evaluator::namespaces_crypto` for it created
//! `evaluator -> package_manager -> package_install -> evaluator`, which
//! `tests/architecture.rs` refused. A package manager depending on the evaluator
//! is the wrong direction, and the gate said so before the commit.

use std::path::{Path, PathBuf};

use crate::hash::{sha256, to_hex};

/// The lockfile's name, beside `serez.json`.
pub const LOCKFILE: &str = "serez.lock";

const LOCK_HEADER: &str = "\
# serez-lock/1
# The resolved dependency graph, as `<name>\\t<version>\\t<integrity>`, sorted.
#
# `integrity` is `sha256-<hex>` over the installed tree: every file's relative
# path and contents, in a fixed order, so it is identical on every platform.
#
# Written by `sz install` and read by it: a package whose staged tree does not
# match the line here is refused before the install is committed, and the project
# is left as it was. Delete a line to allow that package to be re-resolved.";

/// One resolved package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockEntry {
    pub name: String,
    pub version: String,
    /// `sha256-<hex>` over the installed tree.
    pub integrity: String,
}

/// The lockfile, sorted by name.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Lockfile {
    pub entries: Vec<LockEntry>,
}

impl Lockfile {
    /// Read `dir/serez.lock`. A missing file is an empty lockfile, not an error:
    /// the first install in a project is the one that creates it.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let path = dir.join(LOCKFILE);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(Self::default());
        };
        let mut entries = Vec::new();
        for (number, raw) in text.lines().enumerate() {
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = raw.split('\t').collect();
            if parts.len() != 3 {
                return Err(format!(
                    "{}:{}: expected `name<TAB>version<TAB>integrity`, got {:?}",
                    LOCKFILE,
                    number + 1,
                    raw
                ));
            }
            entries.push(LockEntry {
                name: parts[0].to_string(),
                version: parts[1].to_string(),
                integrity: parts[2].to_string(),
            });
        }
        entries.sort();
        Ok(Self { entries })
    }

    /// Write `dir/serez.lock`, sorted, with `\n` endings on every platform.
    ///
    /// Sorted and newline-normalised so the file is a function of the resolved
    /// graph alone: two machines that resolve the same graph produce the same
    /// bytes, and a diff shows what changed rather than how it was written.
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        let mut sorted = self.entries.clone();
        sorted.sort();
        let mut out = String::from(LOCK_HEADER);
        out.push('\n');
        for entry in &sorted {
            out.push_str(&format!(
                "{}\t{}\t{}\n",
                entry.name, entry.version, entry.integrity
            ));
        }
        std::fs::write(dir.join(LOCKFILE), out)
            .map_err(|e| format!("Cannot write {}: {}", LOCKFILE, e))
    }

    pub fn get(&self, name: &str) -> Option<&LockEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Record or replace `entry`, keeping the file sorted.
    pub fn upsert(&mut self, entry: LockEntry) {
        self.entries.retain(|e| e.name != entry.name);
        self.entries.push(entry);
        self.entries.sort();
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.name != name);
        self.entries.len() != before
    }
}

/// `sha256-<hex>` over every file in `root`, by relative path and contents.
///
/// Deterministic across platforms: paths are separated with `/`, the file list
/// is sorted, and each file contributes its path, its length and its bytes — the
/// length so that concatenation cannot be ambiguous between two different trees.
/// Directories contribute nothing of their own; an empty one is invisible, which
/// matches what a package is.
pub fn tree_digest(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort();

    let mut buffer: Vec<u8> = Vec::new();
    for relative in &files {
        let bytes = std::fs::read(root.join(relative))
            .map_err(|e| format!("Cannot read '{}' while hashing: {}", relative, e))?;
        buffer.extend_from_slice(relative.as_bytes());
        buffer.push(0);
        buffer.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        buffer.extend_from_slice(&bytes);
    }
    Ok(format!("sha256-{}", to_hex(&sha256(&buffer))))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Cannot read '{}': {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Cannot read a directory entry: {}", e))?;
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| "a walked path escaped the root".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            out.push(relative);
        }
    }
    Ok(())
}

/// An install in progress: a staging directory that is either committed whole or
/// leaves nothing behind.
///
/// The staging directory sits **beside** the destination rather than in the
/// system temp directory, so the commit is a rename within one filesystem. A
/// cross-device rename fails, and a package manager that works on one machine and
/// not another because of where `TMPDIR` points is worse than one that does not
/// work at all.
pub struct Transaction {
    destination: PathBuf,
    staging: PathBuf,
    committed: bool,
}

impl Transaction {
    /// Begin an install of `destination`. The staging directory is created empty.
    pub fn begin(destination: &Path) -> Result<Self, String> {
        let parent = destination
            .parent()
            .ok_or_else(|| format!("'{}' has no parent directory", destination.display()))?;
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create '{}': {}", parent.display(), e))?;

        let name = destination
            .file_name()
            .ok_or_else(|| format!("'{}' has no name", destination.display()))?
            .to_string_lossy()
            .into_owned();
        // The pid keeps two concurrent installs of the same package from staging
        // into one directory, the same reason `szx::translated_path` carries it.
        let staging = parent.join(format!(".{}.staging-{}", name, std::process::id()));
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        std::fs::create_dir_all(&staging)
            .map_err(|e| format!("Cannot create staging directory: {}", e))?;

        Ok(Self {
            destination: destination.to_path_buf(),
            staging,
            committed: false,
        })
    }

    /// Where to put the files. Nothing here is visible to the project yet.
    pub fn staging_dir(&self) -> &Path {
        &self.staging
    }

    /// The digest of what has been staged.
    pub fn digest(&self) -> Result<String, String> {
        tree_digest(&self.staging)
    }

    /// Refuse unless the staged tree matches `expected`.
    ///
    /// Called **before** [`Self::commit`], which is what makes the guarantee
    /// "verified before the install is committed" rather than "verified, and
    /// then rolled back".
    pub fn verify(&self, expected: &str) -> Result<(), String> {
        let actual = self.digest()?;
        if actual == expected {
            return Ok(());
        }
        Err(format!(
            "integrity check failed for '{}':\n  expected {expected}\n  got      {actual}\n\
             The lockfile records a different tree than the one just fetched. Nothing was \
             installed. Either the source changed or the lockfile is stale; delete the line \
             to re-resolve it deliberately.",
            self.destination
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        ))
    }

    /// Make the staged tree the installed one, replacing any previous version.
    ///
    /// The previous version is moved aside first rather than deleted, so a
    /// failure part-way through can put it back. `fs::rename` refuses to
    /// overwrite a directory on Windows, which is why the swap is three steps
    /// rather than one.
    pub fn commit(mut self) -> Result<(), String> {
        let previous = self
            .destination
            .with_extension(format!("replaced-{}", std::process::id()));
        let had_previous = self.destination.exists();

        if had_previous {
            std::fs::rename(&self.destination, &previous).map_err(|e| {
                format!(
                    "Cannot move the previous '{}' aside: {}",
                    self.destination.display(),
                    e
                )
            })?;
        }

        if let Err(e) = std::fs::rename(&self.staging, &self.destination) {
            // Put the old version back before reporting. Losing a working
            // package to a failed upgrade is the exact failure this type exists
            // to prevent.
            if had_previous {
                let _ = std::fs::rename(&previous, &self.destination);
            }
            return Err(format!(
                "Cannot install into '{}': {}",
                self.destination.display(),
                e
            ));
        }

        if had_previous {
            let _ = std::fs::remove_dir_all(&previous);
        }
        self.committed = true;
        Ok(())
    }
}

impl Drop for Transaction {
    /// A transaction that was not committed leaves nothing behind.
    ///
    /// In `Drop` rather than at each early return: the install path has a dozen
    /// ways to fail and one of them will eventually forget to clean up.
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_dir_all(&self.staging);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "sz_pkg_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(path, body).expect("write");
    }

    // ── the digest ────────────────────────────────────────────────────────

    #[test]
    fn the_same_tree_hashes_the_same_and_a_changed_one_does_not() {
        let root = temp("digest");
        write(&root.join("a/one.sz"), "out 1;");
        write(&root.join("b.sz"), "out 2;");

        let first = tree_digest(&root).expect("digest");
        assert_eq!(first, tree_digest(&root).expect("digest"), "not stable");

        write(&root.join("b.sz"), "out 3;");
        assert_ne!(
            first,
            tree_digest(&root).expect("digest"),
            "content ignored"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_renamed_file_changes_the_digest() {
        // Contents alone would not: the two trees hold the same bytes. Paths are
        // part of the hash for exactly this case.
        let one = temp("digest_a");
        let two = temp("digest_b");
        write(&one.join("x.sz"), "out 1;");
        write(&two.join("y.sz"), "out 1;");
        assert_ne!(
            tree_digest(&one).expect("digest"),
            tree_digest(&two).expect("digest")
        );
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn splitting_a_file_differently_changes_the_digest() {
        // The length prefix. Without it, `("ab", "")` and `("a", "b")` would
        // concatenate to the same bytes and hash identically.
        let one = temp("digest_split_a");
        let two = temp("digest_split_b");
        write(&one.join("f"), "ab");
        write(&one.join("g"), "");
        write(&two.join("f"), "a");
        write(&two.join("g"), "b");
        assert_ne!(
            tree_digest(&one).expect("digest"),
            tree_digest(&two).expect("digest")
        );
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    // ── the transaction ───────────────────────────────────────────────────

    #[test]
    fn a_committed_install_replaces_the_previous_version() {
        let root = temp("commit");
        let dest = root.join("pkg");
        write(&dest.join("old.sz"), "out 1;");

        let tx = Transaction::begin(&dest).expect("begin");
        write(&tx.staging_dir().join("new.sz"), "out 2;");
        tx.commit().expect("commit");

        assert!(dest.join("new.sz").exists(), "the new version is not there");
        assert!(!dest.join("old.sz").exists(), "the old version survived");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_abandoned_install_leaves_the_previous_version_untouched() {
        // The whole point. `install_package` used to delete the destination
        // *before* fetching, so a failed download left the project with nothing.
        let root = temp("rollback");
        let dest = root.join("pkg");
        write(&dest.join("old.sz"), "out 1;");

        {
            let tx = Transaction::begin(&dest).expect("begin");
            write(&tx.staging_dir().join("half.sz"), "out 2;");
            // Dropped without committing — a download that failed half way.
        }

        assert!(dest.join("old.sz").exists(), "the old version was lost");
        assert!(
            !dest.join("half.sz").exists(),
            "a partial install is visible"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_abandoned_install_leaves_no_staging_directory_behind() {
        let root = temp("staging");
        let dest = root.join("pkg");
        {
            let tx = Transaction::begin(&dest).expect("begin");
            write(&tx.staging_dir().join("x.sz"), "out 1;");
        }
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("read")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_first_install_needs_no_previous_version() {
        let root = temp("fresh");
        let dest = root.join("pkg");
        let tx = Transaction::begin(&dest).expect("begin");
        write(&tx.staging_dir().join("x.sz"), "out 1;");
        tx.commit().expect("commit");
        assert!(dest.join("x.sz").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn verify_accepts_the_tree_it_describes_and_refuses_any_other() {
        let root = temp("verify");
        let dest = root.join("pkg");
        let tx = Transaction::begin(&dest).expect("begin");
        write(&tx.staging_dir().join("x.sz"), "out 1;");

        let digest = tx.digest().expect("digest");
        tx.verify(&digest)
            .expect("the tree must match its own digest");

        let wrong = tx
            .verify("sha256-0000")
            .expect_err("a wrong digest must be refused");
        assert!(wrong.contains("integrity check failed"), "{wrong}");
        assert!(wrong.contains("Nothing was installed"), "{wrong}");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_failed_verification_installs_nothing() {
        let root = temp("verify_rollback");
        let dest = root.join("pkg");
        write(&dest.join("old.sz"), "out 1;");
        {
            let tx = Transaction::begin(&dest).expect("begin");
            write(&tx.staging_dir().join("new.sz"), "out 2;");
            assert!(tx.verify("sha256-0000").is_err());
            // Not committed, because verification failed.
        }
        assert!(dest.join("old.sz").exists(), "the old version was lost");
        assert!(
            !dest.join("new.sz").exists(),
            "the bad version was installed"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── the lockfile ──────────────────────────────────────────────────────

    #[test]
    fn a_lockfile_round_trips_and_is_sorted() {
        let root = temp("lock");
        let mut lock = Lockfile::default();
        lock.upsert(LockEntry {
            name: "serez-ui".into(),
            version: "1.0.0".into(),
            integrity: "sha256-bbb".into(),
        });
        lock.upsert(LockEntry {
            name: "serez-http".into(),
            version: "2.0.0".into(),
            integrity: "sha256-aaa".into(),
        });
        lock.write(&root).expect("write");

        let read = Lockfile::read(&root).expect("read");
        assert_eq!(read, lock);
        assert_eq!(
            read.entries
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>(),
            vec!["serez-http", "serez-ui"],
            "entries must be sorted by name"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn writing_the_same_graph_twice_produces_the_same_bytes() {
        // Reproducibility, asserted rather than assumed: insertion order must not
        // reach the file, or two machines resolving the same graph produce
        // different lockfiles and every diff is noise.
        let one = temp("lock_a");
        let two = temp("lock_b");
        let entries = [
            ("serez-ui", "1.0.0", "sha256-b"),
            ("serez-http", "2.0.0", "sha256-a"),
            ("serez-graph", "3.0.0", "sha256-c"),
        ];

        let mut first = Lockfile::default();
        for (n, v, i) in entries {
            first.upsert(LockEntry {
                name: n.into(),
                version: v.into(),
                integrity: i.into(),
            });
        }
        let mut second = Lockfile::default();
        for (n, v, i) in entries.iter().rev() {
            second.upsert(LockEntry {
                name: (*n).into(),
                version: (*v).into(),
                integrity: (*i).into(),
            });
        }
        first.write(&one).expect("write");
        second.write(&two).expect("write");

        assert_eq!(
            std::fs::read_to_string(one.join(LOCKFILE)).expect("read"),
            std::fs::read_to_string(two.join(LOCKFILE)).expect("read"),
            "the lockfile depends on insertion order"
        );
        let _ = std::fs::remove_dir_all(&one);
        let _ = std::fs::remove_dir_all(&two);
    }

    #[test]
    fn a_missing_lockfile_is_empty_rather_than_an_error() {
        // The first install in a project is the one that creates it.
        let root = temp("lock_missing");
        assert_eq!(Lockfile::read(&root).expect("read"), Lockfile::default());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_malformed_lockfile_line_is_reported_with_its_number() {
        let root = temp("lock_bad");
        std::fs::write(root.join(LOCKFILE), "# ok\nserez-ui\t1.0.0\n").expect("write");
        let error = Lockfile::read(&root).expect_err("a two-field line is not an entry");
        assert!(error.contains(":2:"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn upsert_replaces_rather_than_duplicates() {
        let mut lock = Lockfile::default();
        for version in ["1.0.0", "2.0.0"] {
            lock.upsert(LockEntry {
                name: "serez-ui".into(),
                version: version.into(),
                integrity: "sha256-x".into(),
            });
        }
        assert_eq!(lock.entries.len(), 1);
        assert_eq!(lock.get("serez-ui").expect("present").version, "2.0.0");
        assert!(lock.remove("serez-ui"));
        assert!(!lock.remove("serez-ui"));
    }
}
