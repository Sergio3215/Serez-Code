//! Where the time goes, per phase, against a recorded baseline.
//!
//! # What was missing
//!
//! `MATURITY_AUDIT.md` carries "no benchmark regression budget in CI, no stored
//! baseline" as medium, open. `run_benchmarks.{sh,ps1}` already measure 17
//! whole-program benchmarks, report the *minimum* of N runs — a process is only
//! ever slowed by its neighbours, never sped up — and can compare against a
//! recorded run. What they cannot do is say **which phase** got slower, and
//! nothing was committed for them to compare against.
//!
//! This measures the pipeline's phases directly: parse, semantic, type-check and
//! evaluate. A whole-program benchmark that moves 15% tells you something
//! changed; a phase measurement tells you where.
//!
//! # Warning, not a gate — deliberately, and for now
//!
//! Timing on a shared CI runner is noisy, and a flaky gate is worse than no gate:
//! it teaches people to re-run until green, which is how a real regression gets
//! merged. So this **never fails on a slow measurement**. It prints the table,
//! flags anything past its budget, and records the spread.
//!
//! The spread is the point of running it that way. **DEC-M10-003** asks for
//! enough evidence to know the runners' real variability before promoting
//! anything to a gate, and the two spread columns are that evidence — collected
//! on every run, including on the three CI operating systems.
//!
//! It fails on exactly two things, and neither is a timing: a missing baseline
//! and a malformed one. A comparison against nothing is not a comparison.
//!
//! # How it measures, and why that changed
//!
//! Each phase used to run `SAMPLES` times in a row before the next one started,
//! with no warmup. Two problems, both visible in the numbers:
//!
//!   * whatever else the machine did during a phase's block of runs landed on
//!     **that phase**, entirely — which is how one phase acquires a 3× spread
//!     while its neighbours look clean;
//!   * the first run of a phase pays for cold caches and a heap that has not
//!     reached a steady size, and it went straight into `max`, which is the
//!     number the spread column reports.
//!
//! Now: three warmup rounds are discarded, then fifteen rounds run **every**
//! phase once each. A slow stretch of wall-clock touches every phase's sample
//! for that round rather than one phase's whole distribution. Measured on
//! `windows/x86_64`, the reported spread fell from 1.8–3.6× (`max/min`,
//! consecutive) to 1.1–1.6× (`max/median`, interleaved).
//!
//! **The change invalidated the old baseline**, and that is worth saying plainly
//! rather than presenting the new numbers as an improvement in the code. A phase
//! measured after a *different* phase sees a colder cache than one measured
//! after itself, so the minimum rises — most for the sub-100 µs phases. The
//! baseline was re-recorded under the new regime and the two sets of numbers are
//! not comparable. §5.54.
//!
//! # Why the minimum, and why the median beside it
//!
//! The same reasoning `run_benchmarks.sh` documents. The fastest of N runs is the
//! least-contaminated estimate of the work; the mean folds in whatever else the
//! machine was doing. The **median** is reported next to it because the distance
//! between them is how much to trust the minimum, and `max/median` is a better
//! stability signal than `max/min`: one scheduling hiccup moves `max` and leaves
//! the median where it is, while `max/min` is moved by an unusually fast run too,
//! which is evidence of nothing.
//!
//! # Which machine
//!
//! The baseline records the OS and architecture it was made on, and a comparison
//! against a different one says so in the output. A baseline recorded on one
//! machine and compared on another says as much about the two machines as about
//! the code.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::run::{RunOpts, run_source_detailed};
use serez_code::semantic;
use serez_code::type_checker::TypeChecker;

/// How many times each phase is measured. The minimum of these is the statistic.
const SAMPLES: usize = 15;

/// Runs discarded before any sample is kept.
///
/// The first run of a phase pays for cold instruction and data caches, lazily
/// initialised statics and a heap that has not yet reached a steady size, and
/// none of that is the work being measured. It went into the minimum before,
/// which is the one statistic a slow first run cannot corrupt — but it went into
/// `max` every time, and `max` is what the spread column reports.
const WARMUP: usize = 3;

/// How much slower than the baseline a phase may be before it is flagged.
///
/// 1.5× is loose on purpose. A tighter budget on an unmeasured runner produces
/// noise, and this run's job is to *establish* what the runners do before anyone
/// argues about a number. Tightening it is a decision to take against the spread
/// this collects, not in advance of it.
const BUDGET: f64 = 1.5;

const BASELINE: &str = "perf-baseline.txt";

const HEADER: &str = "\
# serez-perf-baseline/1
# Phase timings in microseconds, as `<phase>\\t<micros>`, sorted.
#
# The statistic is the MINIMUM of several runs: a process is only ever slowed by
# its neighbours, never sped up, so the fastest run is the least-contaminated
# estimate of the work itself. The runs are interleaved — every phase once per
# round — after discarding warmup rounds, so a slow stretch of wall-clock lands
# on every phase rather than on whichever one happened to be running.
#
# These numbers are NOT comparable with a baseline recorded before that change:
# a phase measured after a different phase sees a colder cache than one measured
# after itself. See tests/perf_budget.rs and ROADMAP_STATE.md §5.54.
#
# These are advisory. `tests/perf_budget.rs` flags a phase more than 1.5x its
# baseline and does **not** fail: timing on a shared runner is noisy, and a flaky
# gate teaches people to re-run until green, which is how a real regression gets
# merged. It records the observed spread on every run so that promoting any of
# these to a gate is a decision taken against evidence.
#
# Numbers are machine-specific. A baseline recorded on one machine and compared
# on another says more about the two machines than about the code — which is why
# a difference here is a prompt to look, not a verdict.
#
# Refresh with: SEREZ_PERF_UPDATE=1 cargo test --release --test perf_budget";

/// The machine line a baseline carries, so a comparison can name both sides.
const MACHINE_PREFIX: &str = "# recorded on: ";

/// The machine a committed baseline was recorded on, if it says.
///
/// `None` for a baseline written before this line existed, which compares
/// exactly as it did before rather than refusing.
fn baseline_machine(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix(MACHINE_PREFIX))
        .map(|rest| rest.trim().to_string())
}

/// One phase measurement, as a distribution rather than a number.
struct Timing {
    phase: &'static str,
    min: Duration,
    median: Duration,
    mean: Duration,
    max: Duration,
}

impl Timing {
    /// From a round's worth of samples, warmup already discarded.
    fn from_samples(phase: &'static str, mut samples: Vec<Duration>) -> Timing {
        assert!(!samples.is_empty(), "a phase must have at least one sample");
        samples.sort_unstable();
        let total: Duration = samples.iter().sum();
        Timing {
            phase,
            min: samples[0],
            median: samples[samples.len() / 2],
            mean: total / samples.len() as u32,
            max: samples[samples.len() - 1],
        }
    }

    /// How far the slowest run was from the typical one.
    ///
    /// `max/median`, not `max/min`. A single scheduling hiccup moves `max` and
    /// leaves `median` where it is, so this says "one run was slow"; `max/min`
    /// says the same thing but is also moved by an unusually *fast* run, which
    /// is not evidence of anything. Both are printed — the two together are what
    /// distinguishes a noisy runner from a shifted distribution.
    fn spread(&self) -> f64 {
        ratio(self.max, self.median)
    }

    fn range(&self) -> f64 {
        ratio(self.max, self.min)
    }
}

fn ratio(a: Duration, b: Duration) -> f64 {
    if b.as_nanos() == 0 {
        return 1.0;
    }
    a.as_nanos() as f64 / b.as_nanos() as f64
}

/// The machine a baseline was recorded on, so a comparison can say when it is
/// comparing two machines rather than two revisions.
fn machine() -> String {
    format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The programs every phase is measured over.
///
/// Fixed and committed rather than "the whole corpus": a measurement whose input
/// changes whenever a fixture is added is not comparable with the run before it.
/// Chosen to be representative rather than extreme — the decision asks to avoid
/// microbenchmarks, and a 5,000-line pathological file would measure one thing
/// nobody writes.
const INPUTS: &[&str] = &[
    "tests/08_classes.sz",
    "tests/10_lambdas.sz",
    "tests/32_e2e_full.sz",
    "tests/40_algorithms_e2e.sz",
    "std/collections.sz",
];

fn load_inputs() -> Vec<(String, String)> {
    let root = crate_root();
    let mut out = Vec::new();
    for name in INPUTS {
        let path = root.join(name);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("the perf corpus must exist: {} ({e})", path.display()));
        out.push(((*name).to_string(), source));
    }
    out
}

/// One phase: its name, and the work one sample runs.
///
/// Named rather than written inline at both sites, which clippy asked for and
/// which is right anyway — the tuple appears in a signature and in a local, and
/// they have to agree.
type Phase<'a> = (&'static str, Box<dyn FnMut() + 'a>);

/// Measure every phase, interleaved, with the warmup rounds discarded.
///
/// # Why interleaved
///
/// Each phase used to be run `SAMPLES` times in a row before the next one
/// started. Whatever else the machine did during those runs landed on **one**
/// phase, entirely — which is how a phase acquires a 3× spread while its
/// neighbours look clean, and why the numbers were not comparable to each other.
///
/// One round runs every phase once. A slow stretch of wall-clock now touches
/// every phase's sample for that round instead of one phase's whole
/// distribution, so the contamination is visible as a shift in all of them
/// rather than a regression in one.
fn measure_all(phases: &mut [Phase<'_>]) -> Vec<Timing> {
    let mut samples: Vec<Vec<Duration>> = vec![Vec::with_capacity(SAMPLES); phases.len()];

    for round in 0..(WARMUP + SAMPLES) {
        for (index, (_, work)) in phases.iter_mut().enumerate() {
            let start = Instant::now();
            work();
            let elapsed = start.elapsed();
            if round >= WARMUP {
                samples[index].push(elapsed);
            }
        }
    }

    phases
        .iter()
        .zip(samples)
        .map(|((phase, _), taken)| Timing::from_samples(phase, taken))
        .collect()
}

fn read_baseline(path: &Path) -> Result<Vec<(String, u128)>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        if raw.is_empty() || raw.starts_with('#') {
            continue;
        }
        let (phase, micros) = raw
            .split_once('\t')
            .ok_or_else(|| format!("{BASELINE}:{}: expected `phase<TAB>micros`", number + 1))?;
        let micros: u128 = micros
            .trim()
            .parse()
            .map_err(|_| format!("{BASELINE}:{}: {micros:?} is not a number", number + 1))?;
        out.push((phase.to_string(), micros));
    }
    Ok(out)
}

fn write_baseline(path: &Path, timings: &[Timing]) {
    // Recorded, not asserted on: see `baseline_machine`. A committed baseline
    // that does not say which machine produced it makes every cross-platform
    // comparison silently ambiguous, which is most of what made the numbers
    // hard to argue about.
    let mut out = String::from(HEADER);
    out.push('\n');
    out.push_str(MACHINE_PREFIX);
    out.push_str(&machine());
    out.push('\n');
    let mut rows: Vec<_> = timings
        .iter()
        .map(|t| (t.phase, t.min.as_micros()))
        .collect();
    rows.sort();
    for (phase, micros) in rows {
        out.push_str(&format!("{phase}\t{micros}\n"));
    }
    std::fs::write(path, out).expect("cannot write the baseline");
}

#[test]
fn phase_timings_are_measured_and_compared_against_the_baseline() {
    let inputs = load_inputs();

    // Parsed once up front: the later phases take a tree, and re-parsing inside
    // each of them would fold the parser's time into all four measurements.
    let programs: Vec<_> = inputs
        .iter()
        .map(|(name, source)| {
            let mut parser = Parser::new(Lexer::new(source.clone()));
            let program = parser.parse_program();
            assert!(
                !parser.has_errors(),
                "the perf corpus must parse: {name} -> {:?}",
                parser.take_errors()
            );
            program
        })
        .collect();

    let mut phases: Vec<Phase> = vec![
        (
            "frontend.parse",
            Box::new(|| {
                for (_, source) in &inputs {
                    let mut parser = Parser::new(Lexer::new(source.clone()));
                    std::hint::black_box(parser.parse_program());
                }
            }),
        ),
        (
            "semantic.validate",
            Box::new(|| {
                for program in &programs {
                    std::hint::black_box(semantic::validate::validate(program));
                }
            }),
        ),
        (
            "semantic.declarations",
            Box::new(|| {
                for program in &programs {
                    std::hint::black_box(semantic::declarations(program));
                }
            }),
        ),
        (
            "types.check",
            Box::new(|| {
                for program in &programs {
                    let mut checker = TypeChecker::new(program);
                    checker.check();
                    std::hint::black_box(checker.take_errors());
                }
            }),
        ),
        // The whole pipeline through the runtime. Not the sum of the phases
        // above — it includes evaluation, which is where most of the time is —
        // and it is the number a user actually experiences.
        (
            "runtime.execute",
            Box::new(|| {
                for (name, source) in &inputs {
                    std::hint::black_box(run_source_detailed(
                        source.clone(),
                        name,
                        RunOpts::default(),
                    ));
                }
            }),
        ),
    ];
    let timings = measure_all(&mut phases);

    let path = crate_root().join(BASELINE);

    // A debug build is several times slower than a release one, so comparing a
    // debug run against a release baseline would warn on every phase, every
    // time — and a warning that always fires is read as noise within a week.
    // The numbers are still printed, because they are useful while working on a
    // phase; only the comparison is skipped.
    if cfg!(debug_assertions) && std::env::var("SEREZ_PERF_UPDATE").is_err() {
        eprintln!(
            "
── phase timings, debug build (minimum of {SAMPLES} runs, µs) ──"
        );
        for timing in &timings {
            eprintln!("{:<24} {:>10}", timing.phase, timing.min.as_micros());
        }
        eprintln!(
            "Not compared: the baseline is recorded in release. Run
               cargo test --release --test perf_budget -- --nocapture"
        );
        return;
    }

    if std::env::var("SEREZ_PERF_UPDATE").is_ok() {
        assert!(
            !cfg!(debug_assertions),
            "refusing to record a baseline from a debug build — it would be several              times slower than every release run compared against it, and the file is              committed. Use `cargo test --release --test perf_budget`."
        );
        write_baseline(&path, &timings);
        eprintln!("wrote {}", path.display());
        return;
    }

    // The two things that *are* failures. A comparison against nothing is not a
    // comparison, and silently treating a missing baseline as "fine" is how this
    // file would become decoration.
    let baseline = read_baseline(&path).expect("the perf baseline must be readable");
    assert!(
        !baseline.is_empty(),
        "{BASELINE} has no entries; regenerate it with SEREZ_PERF_UPDATE=1"
    );

    eprintln!(
        "\n── phase timings on {} ({SAMPLES} interleaved rounds after {WARMUP} \
         warmup, µs) ──",
        machine()
    );
    if let Some(recorded) = baseline_machine(&path) {
        if recorded != machine() {
            eprintln!(
                "  NOTE: the baseline was recorded on {recorded} and this is {}. \
                 A difference below is between two machines as much as between \
                 two revisions.",
                machine()
            );
        }
    }
    eprintln!(
        "{:<24} {:>9} {:>9} {:>9} {:>7} {:>8} {:>8}",
        "phase", "baseline", "min", "median", "ratio", "max/med", "max/min"
    );

    let mut over_budget = Vec::new();
    let mut missing = Vec::new();
    for timing in &timings {
        let now = timing.min.as_micros();
        let median = timing.median.as_micros();
        // The runner's own variability, measured on this run rather than assumed.
        let spread = timing.spread();
        let range = timing.range();
        match baseline.iter().find(|(p, _)| p == timing.phase) {
            Some((_, was)) => {
                let ratio = if *was > 0 {
                    now as f64 / *was as f64
                } else {
                    1.0
                };
                eprintln!(
                    "{:<24} {was:>9} {now:>9} {median:>9} {ratio:>6.2}x {spread:>7.2}x \
                     {range:>7.2}x",
                    timing.phase
                );
                if ratio > BUDGET {
                    over_budget.push(format!(
                        "  {} is {ratio:.2}x its baseline ({was} -> {now} µs); the budget is \
                         {BUDGET:.1}x, and this run's own max/median was {spread:.2}x \
                         (max/min {range:.2}x)",
                        timing.phase
                    ));
                }
            }
            None => {
                eprintln!(
                    "{:<24} {:>9} {now:>9} {median:>9} {:>7} {spread:>7.2}x {range:>7.2}x",
                    timing.phase, "new", "-"
                );
                missing.push(timing.phase);
            }
        }
        let _ = timing.mean;
    }

    if !missing.is_empty() {
        eprintln!(
            "\n{} phase(s) are not in the baseline yet: {missing:?}. \
             Refresh with SEREZ_PERF_UPDATE=1.",
            missing.len()
        );
    }

    if over_budget.is_empty() {
        eprintln!("\nEvery phase is within {BUDGET:.1}x of its baseline.");
        return;
    }

    // Loud, and not a failure. See the module docs: a flaky timing gate teaches
    // people to re-run until green, and this run's `spread` column is the
    // evidence for whether any of these is stable enough to become one.
    eprintln!(
        "\n⚠ {} phase(s) past the budget:\n{}\n\
         This is a WARNING and not a failure. Compare against the spread column \
         before treating it as a regression, and remember the baseline is \
         machine-specific.",
        over_budget.len(),
        over_budget.join("\n")
    );
}

#[test]
fn the_perf_corpus_is_present_and_is_not_trivial() {
    // The control. Every timing above is measured over `INPUTS`, so a run where
    // those files are missing or empty would report suspiciously good numbers and
    // no failure at all.
    let inputs = load_inputs();
    assert_eq!(inputs.len(), INPUTS.len());
    let total: usize = inputs.iter().map(|(_, s)| s.len()).sum();
    assert!(
        total > 20_000,
        "the perf corpus is only {total} bytes; a shrunken corpus would make every \
         measurement look faster without anything having improved"
    );
}
