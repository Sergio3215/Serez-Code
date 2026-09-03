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
//! The spread is the point of running it that way. The decision asks for enough
//! evidence to know the runners' real variability before promoting anything to a
//! gate, and `max/min` per phase is that evidence — collected on every run,
//! including on the three CI operating systems.
//!
//! It fails on exactly two things, and neither is a timing: a missing baseline
//! and a malformed one. A comparison against nothing is not a comparison.
//!
//! # Why the minimum
//!
//! The same reasoning `run_benchmarks.sh` documents. The fastest of N runs is the
//! least-contaminated estimate of the work; the mean folds in whatever else the
//! machine was doing. The mean and max are still recorded, because their distance
//! from the minimum is how much to trust the number.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::run::{RunOpts, run_source_detailed};
use serez_code::semantic;
use serez_code::type_checker::TypeChecker;

/// How many times each phase is measured. The minimum of these is the statistic.
const SAMPLES: usize = 7;

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
# estimate of the work itself.
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

/// One phase measurement.
struct Timing {
    phase: &'static str,
    min: Duration,
    mean: Duration,
    max: Duration,
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

/// Run `work` `SAMPLES` times and keep the minimum, mean and max.
fn measure(phase: &'static str, mut work: impl FnMut()) -> Timing {
    let mut samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        work();
        samples.push(start.elapsed());
    }
    let total: Duration = samples.iter().sum();
    Timing {
        phase,
        min: *samples.iter().min().expect("SAMPLES > 0"),
        mean: total / SAMPLES as u32,
        max: *samples.iter().max().expect("SAMPLES > 0"),
    }
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
    let mut out = String::from(HEADER);
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

    let timings = vec![
        measure("frontend.parse", || {
            for (_, source) in &inputs {
                let mut parser = Parser::new(Lexer::new(source.clone()));
                std::hint::black_box(parser.parse_program());
            }
        }),
        measure("semantic.validate", || {
            for program in &programs {
                std::hint::black_box(semantic::validate::validate(program));
            }
        }),
        measure("semantic.declarations", || {
            for program in &programs {
                std::hint::black_box(semantic::declarations(program));
            }
        }),
        measure("types.check", || {
            for program in &programs {
                let mut checker = TypeChecker::new(program);
                checker.check();
                std::hint::black_box(checker.take_errors());
            }
        }),
        // The whole pipeline through the runtime. Not the sum of the phases
        // above — it includes evaluation, which is where most of the time is —
        // and it is the number a user actually experiences.
        measure("runtime.execute", || {
            for (name, source) in &inputs {
                std::hint::black_box(run_source_detailed(
                    source.clone(),
                    name,
                    RunOpts::default(),
                ));
            }
        }),
    ];

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

    eprintln!("\n── phase timings (minimum of {SAMPLES} runs, µs) ──");
    eprintln!(
        "{:<24} {:>10} {:>10} {:>8} {:>8}",
        "phase", "baseline", "now", "ratio", "spread"
    );

    let mut over_budget = Vec::new();
    let mut missing = Vec::new();
    for timing in &timings {
        let now = timing.min.as_micros();
        // The runner's own variability, measured on this run rather than assumed.
        let spread = if timing.min.as_micros() > 0 {
            timing.max.as_micros() as f64 / timing.min.as_micros() as f64
        } else {
            1.0
        };
        match baseline.iter().find(|(p, _)| p == timing.phase) {
            Some((_, was)) => {
                let ratio = if *was > 0 {
                    now as f64 / *was as f64
                } else {
                    1.0
                };
                eprintln!(
                    "{:<24} {was:>10} {now:>10} {ratio:>7.2}x {spread:>7.2}x",
                    timing.phase
                );
                if ratio > BUDGET {
                    over_budget.push(format!(
                        "  {} is {ratio:.2}x its baseline ({was} -> {now} µs); the budget is \
                         {BUDGET:.1}x and this run's own spread was {spread:.2}x",
                        timing.phase
                    ));
                }
            }
            None => {
                eprintln!(
                    "{:<24} {:>10} {now:>10} {:>8} {spread:>7.2}x",
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
