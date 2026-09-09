//! Where a program is running, as far as the person reading a diagnostic is
//! concerned.
//!
//! # The problem this exists for
//!
//! The online playground runs a snippet through `sz --eval`, which means
//! lockdown: `File`, `import`, Autodiff's weight files, `use permissions` and
//! the network are all refused, and every guarded namespace is refused too
//! because lockdown starts with an empty permission set. The refusals were
//! worded for the person who typed `sz --eval` in a terminal:
//!
//! ```text
//! ❌ ERROR [SZ6001]: fetch to 'pokeapi.co' is not available here — this code
//! runs as untrusted source, and the network is closed unless the host allows
//! it explicitly.
//! ```
//!
//! Every word of that is true and none of it helps a reader whose first contact
//! with the language is a web page. *Untrusted source*, *the host*, *allows it
//! explicitly* are terms from the security model; the reader has no security
//! model yet. What they have is a `fetch` that did not work, and the obvious
//! conclusion — the reasonable one — is that `fetch` is broken.
//!
//! # What this module changes, and what it deliberately does not
//!
//! **It changes wording, and only wording.** Nothing here is consulted by a
//! gate. [`Environment`] is metadata the *embedder* sets, it reaches the
//! evaluator alongside the security policy rather than inside it, and the one
//! question it answers is which of two texts a refusal that has *already
//! happened* should carry. A capability that is closed stays closed in every
//! environment, and a capability that is open is never closed by this.
//!
//! That separation is the reason `Environment` is not a field of
//! `permissions::SecurityPolicy`: a value that can only affect a message cannot
//! be mistaken for one that affects a decision if it does not live where the
//! decisions are made.
//!
//! # The four things a reader can be looking at
//!
//! | | Cause | What it says |
//! | --- | --- | --- |
//! | A | The online beta switches a capability off | `❌ BETA LIMITATION` — the text below |
//! | B | A local run lacks a permission | `❌ ERROR [SZ6001]: '…' requires permission '…'` — unchanged |
//! | C | The program is wrong | the ordinary diagnostic — unchanged |
//! | D | The embedder refused something | the embedder's own message — unchanged |
//!
//! Only A is new. B, C and D are byte-identical to what they were, which is
//! what makes this safe to land: `sz file.sz` and `sz --eval` from a terminal
//! are [`Environment::Local`], and [`Environment::Local`] is the default.

/// Which environment a program is running in.
///
/// Set by the **embedder** — `run::RunOpts::environment`,
/// `Evaluator::set_environment`, or the CLI's `--online-beta`. There is
/// deliberately no way for a running program to change it, for the same reason
/// it cannot extend the fetch allowlist: it would be describing itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    /// A Serez Code installation on someone's machine. The default, and what
    /// `sz file.sz` and a terminal `sz --eval` are.
    #[default]
    Local,
    /// The online beta playground at serezcode.org: a hosted `sz --eval` behind
    /// a web page, where a reader may never have seen the language before.
    OnlineBeta,
}

impl Environment {
    pub fn is_online_beta(self) -> bool {
        matches!(self, Environment::OnlineBeta)
    }
}

/// A capability the online beta does not expose.
///
/// One entry per gate that can refuse in the playground, so the text lives here
/// rather than at fourteen call sites. A builtin says *which* capability it is;
/// it does not say anything about the beta.
///
/// Every one of these **exists in Serez Code** and works in a local install.
/// That is the whole point of the module: the list is what the environment
/// withholds, not what the language lacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// `fetch`. Closed by lockdown unless the embedder opens a host.
    Network,
    /// The `File` namespace.
    Filesystem,
    /// `import`, and the packages it reaches.
    Modules,
    /// `Autodiff.saveWeights` / `Autodiff.loadWeights`.
    ModelWeights,
    /// `use permissions { … }`, which lockdown refuses instead of granting.
    PermissionGrants,
    /// The `OS` namespace: running and spawning processes.
    ProcessExecution,
    /// The `Terminal` namespace: raw mode, size, input events.
    Terminal,
    /// The `Env` namespace.
    EnvironmentVariables,
    /// The `System` namespace.
    SystemInfo,
    /// The `Socket` namespace.
    Sockets,
    /// The `Media` namespace: audio playback.
    Media,
    /// The `Gui` namespace: windows, drawing, clipboard and file dialogs.
    Gui,
    /// The `Task` namespace: worker threads.
    BackgroundTasks,
    /// `Time`, and the `DateTime` readers that need the same permission.
    Clock,
}

impl Capability {
    /// Every capability the online beta withholds.
    ///
    /// Exists so a test can assert the shape of all of them at once rather than
    /// of whichever one someone remembered to cover.
    pub const ALL: &'static [Capability] = &[
        Capability::Network,
        Capability::Filesystem,
        Capability::Modules,
        Capability::ModelWeights,
        Capability::PermissionGrants,
        Capability::ProcessExecution,
        Capability::Terminal,
        Capability::EnvironmentVariables,
        Capability::SystemInfo,
        Capability::Sockets,
        Capability::Media,
        Capability::Gui,
        Capability::BackgroundTasks,
        Capability::Clock,
    ];

    /// The subject of the first sentence: *"&lt;subject&gt; is disabled in this
    /// online beta environment."*
    ///
    /// Singular by construction, so the template needs no verb agreement and a
    /// new entry cannot quietly produce *"Modules is disabled"*. Pinned by
    /// `every_subject_reads_as_a_singular_english_sentence`.
    pub fn subject(self) -> &'static str {
        match self {
            Capability::Network => "Network access",
            Capability::Filesystem => "Filesystem access",
            Capability::Modules => "Module and package loading",
            Capability::ModelWeights => "Saving and loading model weights",
            Capability::PermissionGrants => "Granting permissions",
            Capability::ProcessExecution => "Process execution",
            Capability::Terminal => "Terminal control",
            Capability::EnvironmentVariables => "Environment variable access",
            Capability::SystemInfo => "System information access",
            Capability::Sockets => "Socket access",
            Capability::Media => "Audio playback",
            Capability::Gui => "Window, clipboard and file-dialog access",
            Capability::BackgroundTasks => "Background task execution",
            Capability::Clock => "Clock access",
        }
    }

    /// What the reader was reaching for, for the closing line: *"To use
    /// &lt;example&gt; and the full Serez Code runtime, …"*.
    ///
    /// Names the thing they typed rather than the category, because the sentence
    /// has to connect to the line they are looking at.
    ///
    /// Carries no `and` of its own: the template supplies one, and a second
    /// turns the closing line into *"to use X and Y and the full Serez Code
    /// runtime"*. Pinned by `no_example_collides_with_the_and_in_the_template`.
    pub fn example(self) -> &'static str {
        match self {
            Capability::Network => "`fetch`",
            Capability::Filesystem => "the `File` namespace",
            Capability::Modules => "`import`",
            Capability::ModelWeights => "weight files like `Autodiff.saveWeights`",
            Capability::PermissionGrants => "`use permissions`",
            Capability::ProcessExecution => "the `OS` namespace",
            Capability::Terminal => "the `Terminal` namespace",
            Capability::EnvironmentVariables => "the `Env` namespace",
            Capability::SystemInfo => "the `System` namespace",
            Capability::Sockets => "the `Socket` namespace",
            Capability::Media => "the `Media` namespace",
            Capability::Gui => "the `Gui` namespace",
            Capability::BackgroundTasks => "the `Task` namespace",
            Capability::Clock => "clock readers like `DateTime.now`",
        }
    }

    /// The capability a permission name gates.
    ///
    /// `require_permission` knows the permission it asked for and nothing else,
    /// so the mapping lives here rather than being repeated at eleven guards.
    /// `None` for a name this table does not cover, and a `None` falls back to
    /// the ordinary permission diagnostic — an unrecognised name is exactly the
    /// case where inventing a friendly sentence would be inventing a fact.
    ///
    /// Held to `permissions::ENFORCED` by
    /// `every_enforced_permission_names_a_capability`.
    pub fn for_permission(name: &str) -> Option<Capability> {
        Some(match name {
            "Env" => Capability::EnvironmentVariables,
            "Gui" => Capability::Gui,
            "Media" => Capability::Media,
            "OS" => Capability::ProcessExecution,
            "Socket" => Capability::Sockets,
            "System" => Capability::SystemInfo,
            "Task" => Capability::BackgroundTasks,
            "Terminal" => Capability::Terminal,
            "Time" => Capability::Clock,
            _ => return None,
        })
    }

    /// The message this capability reports when the online beta refuses it.
    pub fn limitation(self) -> String {
        limitation(self.subject(), self.example())
    }
}

/// Where a reader is told to go for everything the playground does not have.
///
/// One constant rather than fourteen string literals: a domain that appears in
/// every refusal is a thing that must never be wrong in one of them.
pub const INSTALL_SOURCE: &str = "serezcode.org";

/// The three paragraphs every online-beta refusal is made of.
///
/// The order is the order the questions arrive in:
///
/// 1. **what is off, and where** — so the reader stops looking for a bug in
///    their code;
/// 2. **why** — the playground is a demo, not a reduced version of the
///    language;
/// 3. **what to do** — one place to go, naming the thing they actually typed.
///
/// Taken as a whole it never says the capability does not exist, and never says
/// anything about untrusted source, hosts, sandboxes or policy. Those are true
/// and they are the *runtime's* concerns; a first-time reader is not debugging
/// the runtime.
///
/// `subject` is normally [`Capability::subject`]. It is a parameter because one
/// caller — a beta that opened a short list of hosts and was asked for one that
/// is not on it — has a more specific first line to say, and it must say it
/// through this function rather than beside it.
pub fn limitation(subject: &str, example: &str) -> String {
    format!(
        "{subject} is disabled in this online beta environment.\n\
         \n\
         This playground is intended for quick testing and does not expose all \
         Serez Code capabilities.\n\
         \n\
         To use {example} and the full Serez Code runtime, install Serez Code \
         locally from {INSTALL_SOURCE}."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_is_the_default_so_nothing_becomes_a_beta_message_by_accident() {
        assert_eq!(Environment::default(), Environment::Local);
        assert!(!Environment::default().is_online_beta());
        assert!(Environment::OnlineBeta.is_online_beta());
    }

    #[test]
    fn every_message_answers_the_three_questions_a_reader_has() {
        for capability in Capability::ALL {
            let message = capability.limitation();
            assert!(
                message.contains("is disabled in this online beta environment."),
                "{capability:?}: {message}"
            );
            assert!(
                message.contains("intended for quick testing"),
                "{capability:?}: {message}"
            );
            assert!(
                message.contains(INSTALL_SOURCE),
                "{capability:?} must say where to get the full runtime: {message}"
            );
            assert!(
                message.contains(capability.example()),
                "{capability:?} must name what the reader typed: {message}"
            );
        }
    }

    #[test]
    fn no_message_leaks_the_vocabulary_of_the_security_model() {
        // The defect this module exists for. These words are accurate and they
        // are addressed to someone who is debugging the runtime, not to someone
        // meeting the language on a web page.
        const JARGON: &[&str] = &[
            "untrusted",
            "lockdown",
            "sandbox",
            "permission policy",
            "the host",
            "embedder",
            "ssrf",
            "allowlist",
        ];
        for capability in Capability::ALL {
            let message = capability.limitation().to_lowercase();
            for word in JARGON {
                assert!(
                    !message.contains(word),
                    "{capability:?} leaks internal vocabulary ({word}): {}",
                    capability.limitation()
                );
            }
        }
    }

    #[test]
    fn no_message_suggests_the_capability_does_not_exist() {
        // "Serez Code is broken" is the reading this module exists to prevent,
        // and the phrasings that produce it are the ones that talk about the
        // *language* rather than about the environment.
        for capability in Capability::ALL {
            let message = capability.limitation().to_lowercase();
            for wrong in [
                "not supported",
                "unsupported",
                "does not exist",
                "unknown function",
                "not implemented",
            ] {
                assert!(
                    !message.contains(wrong),
                    "{capability:?} reads as a missing feature: {}",
                    capability.limitation()
                );
            }
        }
    }

    #[test]
    fn every_subject_reads_as_a_singular_english_sentence() {
        // The template supplies "is", so a plural subject would produce
        // "Modules is disabled". Checked structurally rather than by eye,
        // because the failure is silent: the sentence still renders.
        //
        // Two shapes are singular, and which one a subject is decides where its
        // head noun is. A gerund phrase — "Granting permissions", "Saving and
        // loading model weights" — is headed by the gerund at the *front* and
        // takes "is" whatever follows it. Everything else is headed by the noun
        // at the end.
        for capability in Capability::ALL {
            let subject = capability.subject();
            let mut words = subject.split_whitespace();
            let first = words.next().unwrap_or_default();
            let head = if first.to_lowercase().ends_with("ing") {
                first
            } else {
                subject.split_whitespace().last().unwrap_or_default()
            };
            // "access" and "process" end in `s` and are singular; a bare plural
            // does not double it.
            let looks_plural = head.ends_with('s') && !head.ends_with("ss");
            assert!(
                !looks_plural,
                "{capability:?}'s subject is headed by a plural noun ('{head}'), \
                 which the template's 'is' will not agree with: {subject}"
            );
            assert!(
                subject.starts_with(|c: char| c.is_uppercase()),
                "{capability:?}'s subject opens a sentence: {subject}"
            );
        }
    }

    #[test]
    fn no_example_collides_with_the_and_in_the_template() {
        // "To use `import` and packages and the full Serez Code runtime" — the
        // closing line already spends its one `and` joining the capability to
        // the runtime, so an example that brings its own reads as a list that
        // lost its commas.
        for capability in Capability::ALL {
            let example = capability.example();
            assert!(
                !example.contains(" and "),
                "{capability:?}'s example adds a second 'and' to the closing \
                 line: To use {example} and the full Serez Code runtime, …"
            );
        }
    }

    #[test]
    fn every_enforced_permission_names_a_capability() {
        // If a permission is added to the runtime and not here, its refusal in
        // the playground silently falls back to "declare it in serez.json",
        // which is advice nobody can follow there.
        for name in crate::permissions::ENFORCED {
            assert!(
                Capability::for_permission(name).is_some(),
                "permission '{name}' is enforced but has no capability entry; the \
                 playground would tell a reader to declare it in a serez.json \
                 that does not exist"
            );
        }
    }

    #[test]
    fn an_unrecognised_permission_maps_to_nothing_rather_than_to_a_guess() {
        assert_eq!(Capability::for_permission("File"), None);
        assert_eq!(Capability::for_permission("Frobnicate"), None);
        assert_eq!(Capability::for_permission("os"), None);
    }

    #[test]
    fn a_caller_with_a_more_specific_subject_uses_the_same_three_paragraphs() {
        let message = limitation(
            "Network access to 'evil.test'",
            Capability::Network.example(),
        );
        assert!(message.starts_with("Network access to 'evil.test' is disabled"));
        assert!(message.contains("intended for quick testing"));
        assert!(message.ends_with("install Serez Code locally from serezcode.org."));
    }
}
