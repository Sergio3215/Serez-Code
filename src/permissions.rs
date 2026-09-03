//! The permission vocabulary.
//!
//! Which names a permission manifest may contain, and what each one actually
//! does, used to live nowhere: the enforced names existed only as string
//! literals at the nine `require_permission` call sites, and anything else a
//! program declared was inserted into a `HashSet` and never looked at again.
//!
//! That silence had a cost. A misspelled name — `Termnal` for `Terminal` — was
//! accepted, granted nothing, and the program then failed at its first
//! `Terminal` call with a message telling the author to declare a permission
//! they believed they *had* declared, one character away in the same file.
//!
//! Two other names are accepted and do nothing, which is worse than a typo
//! because they look deliberate:
//!
//!   * **`File`** is the second-most-declared capability across the official
//!     packages — 23 `use permissions` blocks and four manifests — and it gates
//!     nothing. `File.read` works with no permissions declared at all.
//!   * **A dotted name** such as `OS.exec` parses, is advertised in the
//!     parser's own comment, grants nothing, and specifically does *not* imply
//!     `OS`. Nobody in the ecosystem writes one yet.
//!
//! Naming them here does not make them enforced — that is a security decision,
//! not a diagnostics one, and `spec/security.md` records it as open. What this
//! module does is let a grant say something when a name will have no effect,
//! and give the one place to change when a capability does become enforced.

/// Namespaces `require_permission` actually checks.
///
/// Kept in step with the call sites by `enforced_permissions_match_the_evaluator`
/// in `tests/frontend_robustness.rs`.
/// What this evaluator is allowed to do.
///
/// Two fields of `Evaluator` until M6.3, and they are one thing because neither
/// answers a question on its own: every gate in the runtime asks *"is this
/// granted, and are we in lockdown?"* together.
///
/// The distinction between them is worth stating at the type, because it is
/// counter-intuitive. `granted` is a **manifest, not a sandbox** - any program
/// can hand itself the lot with `use permissions { .. }`, and File, `import` and
/// Autodiff's weight files reach the disk with no permission declared at all.
/// That is fine when you are running your own file and wrong when the source
/// arrived from somewhere else, which is what `lockdown` is for: it closes the
/// paths the manifest cannot — now including the network. See `fetch_allowlist`.
#[derive(Debug, Default, Clone)]
pub struct SecurityPolicy {
    /// Populated from `serez.json` and `use permissions { }` blocks.
    pub granted: std::collections::HashSet<String>,
    /// Untrusted-source mode.
    pub lockdown: bool,
    /// Hosts `fetch` may reach **under lockdown**. Empty means none.
    ///
    /// **DEC-M7-006.** `fetch` used to be reachable under lockdown, deliberately
    /// and with a conformance test pinning it, on the reasoning that lockdown was
    /// about the machine's own capabilities and the network was a separate
    /// question. The name did more work than the code: a mode described as
    /// untrusted-source that leaves outbound HTTP open lets untrusted source
    /// reach cloud metadata endpoints, services bound to localhost, and the host
    /// as an open relay. Decided: blocked by default, reachable only through an
    /// explicit allowlist.
    ///
    /// **The program cannot put anything in here.** It is set by the embedder —
    /// `run::RunOpts`, `Evaluator::allow_fetch_hosts`, or the CLI's
    /// `--allow-fetch` — for the same reason `use permissions { }` stops granting
    /// under lockdown: a list untrusted source can extend is not a list.
    ///
    /// Entries are hostnames, compared case-insensitively and exactly. No
    /// wildcards and no port matching: both are policy questions, and inventing
    /// an answer to them inside a security gate is how a gate acquires a hole.
    /// Outside lockdown this is not consulted at all.
    pub fetch_allowlist: std::collections::HashSet<String>,
}

impl SecurityPolicy {
    /// Is `permission` granted?
    ///
    /// Named rather than reaching into `granted`, so that every gate asks the
    /// policy instead of inspecting it. That matters if the answer ever stops
    /// being a set lookup — a wildcard, a scope, an inherited grant — because
    /// then it changes in one place rather than at every call site.
    pub fn allows(&self, permission: &str) -> bool {
        self.granted.contains(permission)
    }

    /// Record a grant. Returns whether it was new, which callers use to warn
    /// once about a redundant declaration.
    pub fn grant(&mut self, permission: impl Into<String>) -> bool {
        self.granted.insert(permission.into())
    }

    /// Add a host `fetch` may reach under lockdown. See [`Self::fetch_allowlist`].
    pub fn allow_fetch_host(&mut self, host: impl AsRef<str>) {
        self.fetch_allowlist
            .insert(host.as_ref().trim().to_ascii_lowercase());
    }

    /// May `fetch` reach `url`?
    ///
    /// Outside lockdown, always — this changes nothing for `sz file.sz`. Under
    /// lockdown, only when the URL's host is on the allowlist, and the allowlist
    /// is empty unless an embedder filled it.
    ///
    /// A URL whose host cannot be read is refused rather than guessed at: a gate
    /// that falls open on input it does not understand is not a gate.
    pub fn allows_fetch(&self, url: &str) -> bool {
        if !self.lockdown {
            return true;
        }
        match host_of(url) {
            Some(host) => self.fetch_allowlist.contains(&host),
            None => false,
        }
    }
}

/// The host of an `http`/`https` URL, lowercased, without userinfo or port.
///
/// Written here rather than pulled from a URL crate because it is a security
/// decision in eight lines and it should be readable as one. It is deliberately
/// strict: anything it cannot parse confidently returns `None`, and `None` means
/// refused.
///
///   * userinfo is stripped at the **last** `@` before the path, so
///     `https://evil.test@allowed.test/` reads `allowed.test` — and, more to the
///     point, `https://allowed.test@evil.test/` reads `evil.test`, which is the
///     direction that matters;
///   * a bracketed IPv6 literal keeps its brackets and drops the port after them;
///   * an empty host is `None`.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("HTTP://"))
        .or_else(|| url.strip_prefix("HTTPS://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = match authority.rfind('@') {
        Some(at) => &authority[at + 1..],
        None => authority,
    };
    let host = if let Some(close) = authority.find(']') {
        // `[::1]:8080` -> `[::1]`
        &authority[..=close]
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    if host.is_empty() {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

/// Resolve a `Location` header against the URL it came from.
///
/// Only two shapes are resolved, and everything else returns `None` — which the
/// caller turns into a refusal, not a silent continue. A redirect target is
/// about to become a network request under a security policy, so the parser's
/// job is to be obviously right rather than complete:
///
///   * an absolute `http`/`https` URL is taken as it stands;
///   * a root-relative path (`/next`) is joined to the current scheme and
///     authority, so it stays on the same host and the allowlist check on the
///     next hop is trivially satisfied — that is the point, not a shortcut.
///
/// A protocol-relative `//host/path`, a relative `../next` and anything with a
/// different scheme are all `None`.
pub fn resolve_location(current: &str, location: &str) -> Option<String> {
    let target = location.trim();
    if target.is_empty() {
        return None;
    }
    let lower = target.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(target.to_string());
    }
    if !target.starts_with('/') || target.starts_with("//") {
        return None;
    }
    // Everything up to the end of the authority: scheme, "://", host[:port].
    let scheme_end = current.find("://")? + 3;
    let authority_len = current[scheme_end..]
        .find(['/', '?', '#'])
        .unwrap_or(current.len() - scheme_end);
    let base = &current[..scheme_end + authority_len];
    Some(format!("{base}{target}"))
}

#[cfg(test)]
mod policy_tests {
    use super::SecurityPolicy;

    #[test]
    fn a_fresh_policy_grants_nothing_and_is_not_locked_down() {
        let policy = SecurityPolicy::default();
        assert!(!policy.allows("Terminal"));
        assert!(!policy.lockdown, "lockdown is opt-in; see run::RunOpts");
    }

    #[test]
    fn a_grant_is_exact_and_reports_whether_it_was_new() {
        let mut policy = SecurityPolicy::default();
        assert!(policy.grant("OS"));
        assert!(!policy.grant("OS"), "a repeated grant is not new");
        assert!(policy.allows("OS"));
        assert!(
            !policy.allows("OS.exec"),
            "a grant does not imply a narrower one"
        );
        assert!(!policy.allows("os"), "and it is case-sensitive");
    }

    #[test]
    fn lockdown_is_independent_of_what_is_granted() {
        // The pair is counter-intuitive and worth pinning: granted is a manifest
        // a program can write itself, and lockdown closes paths the manifest
        // does not cover. Neither implies anything about the other.
        let mut policy = SecurityPolicy::default();
        policy.grant("File");
        policy.lockdown = true;
        assert!(policy.allows("File"));
        assert!(policy.lockdown);
    }
}

pub const ENFORCED: &[&str] = &[
    "Env", "Gui", "Media", "OS", "Socket", "System", "Task", "Terminal", "Time",
];

/// Names accepted for compatibility that gate nothing today.
///
/// `File` is here because the ecosystem declares it widely and rejecting it
/// would break working programs for no security gain — it is inert either way.
/// See `spec/security.md`, which states the same thing normatively.
pub const ACCEPTED_BUT_INERT: &[&str] = &["File"];

/// What declaring `name` will do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grant {
    /// The name gates a capability; declaring it has an effect.
    Enforced,
    /// The name is recognised and deliberately gates nothing.
    Inert,
    /// A dotted form like `OS.exec`. It grants nothing and does not imply its
    /// prefix, so `use permissions { OS.exec }` leaves `OS` denied.
    Dotted,
    /// Not a name this runtime knows. Usually a typo.
    Unknown,
}

pub fn classify(name: &str) -> Grant {
    if ENFORCED.contains(&name) {
        Grant::Enforced
    } else if ACCEPTED_BUT_INERT.contains(&name) {
        Grant::Inert
    } else if name.contains('.') {
        Grant::Dotted
    } else {
        Grant::Unknown
    }
}

/// The warning to print for a grant that will not do what its author expects,
/// or `None` when there is nothing useful to say.
///
/// A warning, not a refusal: rejecting an unrecognised name would break any
/// program that declares one today. The point is that the author finds out at
/// the grant rather than at the first denied call.
///
/// **`File` deliberately does not warn.** It is inert, but it is inert because
/// the *runtime* does not gate file access, not because the author wrote
/// anything wrong: they followed the documented convention, and the ecosystem
/// does so twenty-three times. A warning on every run of correct-by-convention
/// code is noise, and noise is what teaches people to ignore the warning that
/// matters — the one-character typo two lines below. `File`'s inertness belongs
/// in `spec/security.md`, where it now is, and in `classify` so it stays
/// testable; not on stderr.
pub fn grant_warning(name: &str) -> Option<String> {
    match classify(name) {
        Grant::Enforced | Grant::Inert => None,
        Grant::Dotted => {
            let base = name.split('.').next().unwrap_or(name);
            let hint = if ENFORCED.contains(&base) {
                format!(" Declare '{base}' instead; there is no per-operation permission.")
            } else {
                String::new()
            };
            Some(format!(
                "permission '{name}' grants nothing: dotted names are parsed but \
                 never checked, and this one does not imply '{base}'.{hint}"
            ))
        }
        Grant::Unknown => {
            let suggestion = closest(name);
            let hint = match suggestion {
                Some(near) => format!(" Did you mean '{near}'?"),
                None => format!(" Known permissions: {}.", ENFORCED.join(", ")),
            };
            Some(format!(
                "permission '{name}' is not a permission this runtime checks, so \
                 declaring it has no effect.{hint}"
            ))
        }
    }
}

/// The enforced name within edit distance 2, if exactly one is that close.
///
/// Deliberately conservative: a wrong suggestion in a security-adjacent message
/// is worse than none, so an ambiguous match suggests nothing.
fn closest(name: &str) -> Option<&'static str> {
    let mut best: Option<(&'static str, usize)> = None;
    let mut tie = false;
    for candidate in ENFORCED {
        let distance = edit_distance(&name.to_lowercase(), &candidate.to_lowercase());
        if distance > 2 {
            continue;
        }
        match best {
            Some((_, d)) if distance > d => {}
            Some((_, d)) if distance == d => tie = true,
            _ => {
                best = Some((candidate, distance));
                tie = false;
            }
        }
    }
    if tie { None } else { best.map(|(c, _)| c) }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            row[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(row[j] + 1);
        }
        std::mem::swap(&mut prev, &mut row);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_enforced_name_warns_about_nothing() {
        for name in ENFORCED {
            assert_eq!(classify(name), Grant::Enforced, "{name}");
            assert_eq!(grant_warning(name), None, "{name}");
        }
    }

    #[test]
    fn file_is_recognised_as_inert_but_stays_silent() {
        // The fact is represented — and testable — without being shouted on
        // every run of code that did nothing wrong. See `grant_warning`.
        assert_eq!(classify("File"), Grant::Inert);
        assert_eq!(grant_warning("File"), None);
    }

    #[test]
    fn a_dotted_name_says_it_does_not_imply_its_prefix() {
        assert_eq!(classify("OS.exec"), Grant::Dotted);
        let warning = grant_warning("OS.exec").expect("a dotted name must warn");
        assert!(warning.contains("does not imply 'OS'"), "{warning}");
        assert!(warning.contains("Declare 'OS' instead"), "{warning}");
    }

    #[test]
    fn a_typo_suggests_the_name_that_was_meant() {
        // The case that motivated this module: one character away from a real
        // permission, silently granted nothing.
        let warning = grant_warning("Termnal").expect("a typo must warn");
        assert!(warning.contains("Did you mean 'Terminal'?"), "{warning}");

        for (typo, expected) in [("Sockett", "Socket"), ("env", "Env"), ("Tim", "Time")] {
            let warning = grant_warning(typo).unwrap_or_default();
            assert!(
                warning.contains(&format!("Did you mean '{expected}'?")),
                "{typo}: {warning}"
            );
        }
    }

    #[test]
    fn a_name_resembling_nothing_lists_the_real_ones() {
        let warning = grant_warning("Frobnicate").expect("an unknown name must warn");
        assert!(warning.contains("Known permissions:"), "{warning}");
        assert!(warning.contains("Terminal"), "{warning}");
        assert!(!warning.contains("Did you mean"), "{warning}");
    }

    #[test]
    fn an_ambiguous_typo_suggests_nothing_rather_than_guessing() {
        // A wrong suggestion in a security-adjacent message is worse than none,
        // so a tie names neither. "Ani" is distance 2 from both Env and Gui.
        assert_eq!(edit_distance("ani", "env"), 2);
        assert_eq!(edit_distance("ani", "gui"), 2);
        assert_eq!(closest("Ani"), None);

        let warning = grant_warning("Ani").expect("an unknown name still warns");
        assert!(!warning.contains("Did you mean"), "{warning}");
        assert!(warning.contains("Known permissions:"), "{warning}");
    }
}
