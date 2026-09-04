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

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::hash::{Sha256, to_hex};

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
/// How much of a file is read at a time while hashing it.
const DIGEST_CHUNK_BYTES: usize = 64 * 1024;

pub fn tree_digest(root: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort();

    // Streamed rather than concatenated. This built one `Vec<u8>` holding every
    // file in the package, which for a tree at the 256 MiB install ceiling meant
    // that much resident at once — and `sha256` copied its input, so twice that.
    // The digest is unchanged: the same bytes reach the hasher in the same order.
    let mut hasher = Sha256::new();
    for relative in &files {
        let path = root.join(relative);
        let length = std::fs::metadata(&path)
            .map_err(|e| format!("Cannot stat '{}' while hashing: {}", relative, e))?
            .len();

        hasher.update(relative.as_bytes());
        hasher.update(&[0]);
        hasher.update(&length.to_le_bytes());

        let mut file = std::fs::File::open(&path)
            .map_err(|e| format!("Cannot read '{}' while hashing: {}", relative, e))?;
        let mut chunk = vec![0u8; DIGEST_CHUNK_BYTES];
        let mut read_total: u64 = 0;
        loop {
            let n = file
                .read(&mut chunk)
                .map_err(|e| format!("Cannot read '{}' while hashing: {}", relative, e))?;
            if n == 0 {
                break;
            }
            hasher.update(&chunk[..n]);
            read_total += n as u64;
        }

        // The length went into the digest before the contents, so a file that
        // changed size between the two would produce a digest describing a tree
        // that never existed. Cheap to notice, and silent otherwise.
        if read_total != length {
            return Err(format!(
                "'{}' changed size while being hashed: {} bytes declared, {} read",
                relative, length, read_total
            ));
        }
    }
    Ok(format!("sha256-{}", to_hex(&hasher.finalize())))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Cannot read '{}': {}", dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("Cannot read a directory entry: {}", e))?;
        let path = entry.path();

        // `symlink_metadata` rather than `is_dir`, which follows. A staged tree
        // should contain no links at all — both install paths refuse them — but
        // a digest that silently walks through one would describe a tree the
        // package does not contain, and would differ between two machines whose
        // link targets differ.
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|e| format!("Cannot inspect '{}': {}", path.display(), e))?
            .file_type();
        if kind.is_symlink() {
            return Err(format!(
                "'{}' is a symbolic link; a package tree must contain only regular \
                 files and directories",
                path.display()
            ));
        }

        if kind.is_dir() {
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

/// The name a superseded version is parked under while the swap happens.
///
/// Kept as a function rather than inlined at both sites, because [`recover`] has
/// to recognise what [`Transaction::commit`] wrote and the two must not drift.
fn parked_prefix(name: &str) -> String {
    format!("{}.replaced-", name)
}

/// Put back what a crashed install left behind, before installing over it.
///
/// # The window this closes, and the one it does not
///
/// `commit` swaps in three renames, because `fs::rename` will not overwrite a
/// directory on Windows. Between the first and the second, **the destination does
/// not exist**: the old version has been parked as `<name>.replaced-<pid>` and
/// the new one is still in staging. A controlled error there is handled — the
/// old version is renamed back before the error returns. A *crash* there is not,
/// because nothing runs. The process dies and the project is left with no
/// package, and the only surviving copy is parked under a name that, before this
/// function existed, nothing ever looked at again.
///
/// Measured, by reconstructing that exact state and running the next install:
///
/// ```text
/// destination: ABSENT
/// siblings:    .test-pkg.staging-99999 test-pkg.replaced-99999
/// -- next run of sz install --
/// ✅ Installed test-pkg@1.0.0 → ./packages/test-pkg
/// siblings:    .test-pkg.staging-99999 test-pkg test-pkg.replaced-99999
/// ```
///
/// The re-install appears to repair it, and that is the trap: it only repairs it
/// *because the registry was still reachable and still had that version*. An
/// upgrade that crashes while the source is offline, or against a version since
/// withdrawn, loses a working install permanently — with an intact copy sitting
/// two directories away. And the parked copy is never removed, so every crashed
/// install leaves one behind for good.
///
/// So recovery happens here, before the install rather than after it: the old
/// version is restored first, and if the install that follows then fails for any
/// reason, the project still has the package it had.
///
/// # What this is not
///
/// This does not make `commit` atomic against a crash, and nothing in this module
/// should be described as crash-safe. The window is still there. What changes is
/// that landing in it is no longer permanent: the state is recognisable on disk,
/// and the next install recognises it.
///
/// # Concurrency
///
/// Two installs of the same package running at once already race destructively —
/// there is no cross-device lock anywhere in this path, and both would move the
/// same destination aside. Recovery does not introduce that race and does not fix
/// it; see §5.45. Where several parked copies exist, the most recently modified
/// one is restored, being the best evidence available of which was last good.
pub fn recover(destination: &Path) -> Result<(), String> {
    let (Some(parent), Some(name)) = (destination.parent(), destination.file_name()) else {
        return Ok(());
    };
    let prefix = parked_prefix(&name.to_string_lossy());

    let Ok(entries) = std::fs::read_dir(parent) else {
        // No parent directory yet means no previous install to recover.
        return Ok(());
    };
    let mut parked: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .map(|n| n.to_string_lossy().starts_with(&prefix))
                    .unwrap_or(false)
        })
        .collect();
    if parked.is_empty() {
        return Ok(());
    }

    if !destination.exists() {
        // Most recently modified first. `modified()` is unavailable on some
        // filesystems, and an unordered restore is still better than none.
        parked.sort_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });
        let newest = parked.pop().expect("non-empty");
        std::fs::rename(&newest, destination).map_err(|e| {
            format!(
                "Cannot restore '{}' from '{}': {}",
                destination.display(),
                newest.display(),
                e
            )
        })?;
    }

    // Whatever is still parked is a superseded copy: either this call just
    // restored a newer one, or the destination was already present, which means
    // some commit got past the swap.
    for stale in parked {
        let _ = std::fs::remove_dir_all(&stale);
    }
    Ok(())
}

impl Transaction {
    /// Begin an install of `destination`, recovering a crashed one first.
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
        // Before anything else: a previous install of this destination may have
        // died mid-swap, and its only surviving copy is parked beside us.
        recover(destination)?;

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
    ///
    /// # What survives what
    ///
    /// A **controlled error** — either rename failing — is handled here: the
    /// previous version goes back before the error returns, and the caller sees
    /// a failure over an unchanged project. A **panic** unwinds through [`Drop`],
    /// which removes the staging directory, so the same holds.
    ///
    /// A **crash** does not run either of those. Between the two renames the
    /// destination does not exist, and a process killed there leaves the project
    /// without the package. That window is real and is not closed by this
    /// function; it is closed by [`recover`], which the next [`Transaction::begin`]
    /// calls. Nothing here is crash-safe on its own, and the first install of a
    /// package — where there is no previous version to move aside — is the only
    /// case that is a single rename.
    pub fn commit(mut self) -> Result<(), String> {
        let previous = self.destination.with_file_name(format!(
            "{}{}",
            parked_prefix(
                &self
                    .destination
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ),
            std::process::id()
        ));
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

    /// Build the on-disk state that a crash inside `commit` leaves behind.
    ///
    /// Reconstructing the state is not the same as surviving a kill, and this
    /// module does not pretend otherwise — `dropping_a_transaction_is_not_a_crash_guarantee`
    /// below measures the difference. What these fixtures do give is the one
    /// thing a real kill cannot: the ability to land in each window *exactly*,
    /// rather than whenever the scheduler happens to allow.
    ///
    /// The pid is a stranger's on purpose. A crashed install's leftovers carry
    /// the pid of a process that is gone, so recovery must not depend on
    /// recognising its own.
    struct CrashSite {
        root: PathBuf,
        destination: PathBuf,
    }

    impl CrashSite {
        fn new(tag: &str) -> Self {
            let root = temp(tag).join("packages");
            std::fs::create_dir_all(&root).expect("packages dir");
            CrashSite {
                destination: root.join("pkg"),
                root,
            }
        }

        fn dir(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.root.join(name);
            std::fs::create_dir_all(&path).expect("dir");
            std::fs::write(path.join("index.sz"), contents).expect("file");
            path
        }

        fn installed(&self) -> Option<String> {
            std::fs::read_to_string(self.destination.join("index.sz")).ok()
        }

        fn siblings(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&self.root)
                .expect("read packages")
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    /// The window: killed between the two renames, the package is simply gone.
    ///
    /// This is the state the probe measured before recovery existed. The old
    /// version is intact but parked under a name nothing consulted, so the
    /// project had no package and the only copy of it was invisible.
    #[test]
    fn a_crash_between_the_renames_leaves_no_package_at_all() {
        let site = CrashSite::new("crash_window");
        site.dir("pkg.replaced-99999", "old");
        site.dir(".pkg.staging-99999", "new");

        assert_eq!(
            site.installed(),
            None,
            "the fixture does not reproduce the window it claims to"
        );
    }

    /// And recovery puts it back, from the parked copy alone.
    ///
    /// No registry, no network, no manifest — the point is that the old version
    /// returns from what is on disk. An install that repairs the state by
    /// re-fetching only works while the source is still reachable.
    #[test]
    fn recovery_restores_the_package_a_crash_removed() {
        let site = CrashSite::new("crash_restore");
        site.dir("pkg.replaced-99999", "old");

        recover(&site.destination).expect("recover");

        assert_eq!(
            site.installed().as_deref(),
            Some("old"),
            "the parked version was not restored"
        );
        assert_eq!(
            site.siblings(),
            vec!["pkg"],
            "recovery restored the package but left the parked copy behind"
        );
    }

    /// A crash *after* the swap parks a superseded copy that is pure litter.
    ///
    /// Recovery must remove it and must not touch the installed version, which
    /// is the newer one. Restoring here would be a downgrade.
    #[test]
    fn recovery_discards_a_superseded_copy_without_touching_the_install() {
        let site = CrashSite::new("crash_litter");
        site.dir("pkg", "new");
        site.dir("pkg.replaced-99999", "old");

        recover(&site.destination).expect("recover");

        assert_eq!(
            site.installed().as_deref(),
            Some("new"),
            "recovery overwrote a good install with a superseded copy"
        );
        assert_eq!(site.siblings(), vec!["pkg"], "the parked copy survived");
    }

    /// Several crashes, and the most recently parked copy is the one restored.
    #[test]
    fn recovery_prefers_the_most_recently_parked_copy() {
        let site = CrashSite::new("crash_many");
        site.dir("pkg.replaced-1", "older");
        std::thread::sleep(std::time::Duration::from_millis(20));
        site.dir("pkg.replaced-2", "newer");

        recover(&site.destination).expect("recover");

        assert_eq!(site.installed().as_deref(), Some("newer"));
        assert_eq!(
            site.siblings(),
            vec!["pkg"],
            "an older copy was left behind"
        );
    }

    /// Negative control: with nothing parked, recovery does nothing at all.
    ///
    /// Without this, every test above would still pass if `recover` unpacked
    /// whatever it found, or if it created a destination out of nothing.
    #[test]
    fn recovery_on_a_clean_tree_changes_nothing() {
        let site = CrashSite::new("crash_clean");
        site.dir("pkg", "installed");

        recover(&site.destination).expect("recover");

        assert_eq!(site.installed().as_deref(), Some("installed"));
        assert_eq!(site.siblings(), vec!["pkg"]);

        // And on a destination that has never existed.
        let never = site.root.join("absent");
        recover(&never).expect("recover");
        assert!(!never.exists(), "recovery invented a package");
    }

    /// Recovery must not mistake a *different* package's parked copy for its own.
    ///
    /// `pkg` and `pkg-extra` share a prefix, and a `starts_with` on the wrong
    /// string would move one package's history into the other's place.
    #[test]
    fn recovery_does_not_confuse_a_package_with_a_longer_named_neighbour() {
        let site = CrashSite::new("crash_prefix");
        site.dir("pkg-extra.replaced-99999", "someone else's");

        recover(&site.destination).expect("recover");

        assert_eq!(
            site.installed(),
            None,
            "a neighbour's parked copy was installed as this package"
        );
        assert_eq!(
            site.siblings(),
            vec!["pkg-extra.replaced-99999"],
            "a neighbour's parked copy was deleted"
        );
    }

    /// A package name with a dot in it parks under a name recovery can find.
    ///
    /// `Path::with_extension` replaces an extension rather than appending one, so
    /// `my.pkg` parked as `my.replaced-<pid>` — a name whose prefix no longer
    /// matched the package, leaving it unrecoverable and undeletable. The commit
    /// path builds the name with `with_file_name` for that reason, and this
    /// asserts the two halves still agree.
    #[test]
    fn a_dotted_package_name_parks_where_recovery_looks() {
        let site = CrashSite::new("crash_dotted");
        let dotted = site.root.join("my.pkg");
        std::fs::create_dir_all(site.root.join("my.pkg.replaced-99999")).expect("parked");
        std::fs::write(
            site.root.join("my.pkg.replaced-99999").join("index.sz"),
            "old",
        )
        .expect("file");

        recover(&dotted).expect("recover");

        assert_eq!(
            std::fs::read_to_string(dotted.join("index.sz"))
                .ok()
                .as_deref(),
            Some("old"),
            "a dotted package name was parked where recovery cannot see it"
        );
    }

    /// The end-to-end shape: crash, then a *failing* install, and the old version
    /// is still there.
    ///
    /// This is what recovery is for. Repairing the state by re-fetching works
    /// only while the source is reachable; running recovery first means the
    /// project keeps its package even when the install that follows fails.
    #[test]
    fn an_install_that_fails_after_a_crash_still_leaves_the_old_version() {
        let site = CrashSite::new("crash_then_fail");
        site.dir("pkg.replaced-99999", "old");

        // `begin` recovers, then stages. The transaction is dropped without a
        // commit, which is what any failure between the two amounts to.
        let transaction = Transaction::begin(&site.destination).expect("begin");
        std::fs::write(transaction.staging_dir().join("index.sz"), "new").expect("stage");
        drop(transaction);

        assert_eq!(
            site.installed().as_deref(),
            Some("old"),
            "a failed install after a crash left the project with nothing"
        );
        assert_eq!(
            site.siblings(),
            vec!["pkg"],
            "the abandoned staging directory or a parked copy survived"
        );
    }

    /// The honest limit: `Drop` is cleanup, not a crash guarantee.
    ///
    /// A controlled failure and a panic both unwind through `Drop`, so the
    /// staging directory goes away. A killed process runs no destructor at all,
    /// and `mem::forget` is the closest thing to that a test can do in-process.
    /// The staging directory survives — which is exactly why `begin` removes a
    /// stale one of its own rather than trusting that one cannot exist.
    #[test]
    fn dropping_a_transaction_is_not_a_crash_guarantee() {
        let site = CrashSite::new("crash_forget");
        let transaction = Transaction::begin(&site.destination).expect("begin");
        let staging = transaction.staging_dir().to_path_buf();
        std::fs::write(staging.join("index.sz"), "half-written").expect("stage");

        std::mem::forget(transaction);

        assert!(
            staging.exists(),
            "this test no longer measures what it claims: Drop ran"
        );

        // And the next install of the same destination clears it, because the
        // staging name is keyed to the pid and this process is reusing its own.
        let next = Transaction::begin(&site.destination).expect("begin again");
        assert_eq!(
            std::fs::read_dir(next.staging_dir()).unwrap().count(),
            0,
            "a stale staging directory was reused with its contents"
        );
    }

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

    /// The digest of a fixed tree, pinned to the value it had before it streamed.
    ///
    /// `tree_digest` used to concatenate every file into one `Vec<u8>` and hash
    /// that; it now feeds the hasher in 64 KiB chunks so a package at the
    /// 256 MiB ceiling is not held in memory twice. The bytes reaching the
    /// hasher are the same, and this is what says so — every `serez.lock` in
    /// existence records digests produced by the old path, and a change here
    /// would make all of them fail verification against an unmodified package.
    ///
    /// Captured from the release binary before the change, installing a
    /// two-file registry package:
    ///
    /// ```text
    /// dig <TAB> 1.0.0 <TAB> sha256-f5f721c49174e9b09eb43d6947ea24e5775afb34ed1b502904c0ef35f3d0a625
    /// ```
    #[test]
    fn the_digest_is_the_one_existing_lockfiles_recorded() {
        let root = temp("digest_pinned");
        write(
            &root.join("index.sz"),
            "alpha
",
        );
        write(&root.join("sub/b.sz"), "beta");

        assert_eq!(
            tree_digest(&root).expect("digest"),
            "sha256-f5f721c49174e9b09eb43d6947ea24e5775afb34ed1b502904c0ef35f3d0a625",
            "the tree digest changed; every existing serez.lock now fails on an              unmodified package"
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
