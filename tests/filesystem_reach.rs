//! What `File` can actually reach, and what a relative path is relative *to*.
//! Recorded rather than assumed.
//!
//! `security.md` says `File` is accepted but inert: `File.read` succeeds with no
//! permissions declared at all, and declaring `File` changes nothing. What it
//! does not say — and what nothing tested — is how far a path may reach, or what
//! it is measured from.
//!
//! Two conformance programs implied the first. `sec_path_traversal.sz` said
//! "path traversal via relative segments must be rejected" and named escaping
//! "the sandbox"; `sec_path_traversal_abs.sz` said absolute access to a system
//! file "must be rejected". Both passed, on all three CI platforms, only because
//! the paths they named do not exist there. There is no guard: `..` walks out of
//! the directory and an absolute path goes wherever it points.
//!
//! The second is a trap of its own. `import "./lib"` resolves against the
//! **file**, so a program keeps working wherever it is run from.
//! `File.read("./data.txt")` resolves against the **process working directory**,
//! so the same script reads its own data file when run from its folder and fails
//! with `SZ4005` when run from one level up. Two features, both spelled `./`,
//! measured from different places.
//!
//! These tests exist so the state is written down and has to be changed
//! deliberately. Confining `File`, or moving its base to the script's directory,
//! are both breaking changes and both are capability decisions rather than bug
//! fixes — when either is made, these are the tests that must be updated.

use std::path::{Path, PathBuf};
use std::process::Command;

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_sz")
}

/// A scratch tree: `<root>/outside.txt` beside `<root>/project/`, so a program
/// in `project/` has something real to reach for.
fn scratch(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("sz_reach_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("project")).expect("a scratch tree");
    std::fs::write(root.join("outside.txt"), "CONTENT OUTSIDE THE PROJECT\n")
        .expect("the file to reach for");
    root
}

/// Run `program` with the process working directory set to `from`, which is the
/// variable these tests are about.
fn run(program: &Path, from: &Path) -> (String, i32) {
    let out = Command::new(binary())
        .arg(program)
        .current_dir(from)
        .output()
        .expect("the sz binary must run");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (text, out.status.code().unwrap_or(-1))
}

#[test]
fn file_read_follows_dot_dot_out_of_the_working_directory() {
    let root = scratch("rel");
    let program = root.join("project/read_up.sz");
    std::fs::write(&program, "out File.read(\"../outside.txt\");\n").expect("fixture");

    let (text, code) = run(&program, &root.join("project"));
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(code, 0, "reading through `..` succeeds today:\n{text}");
    assert!(
        text.contains("CONTENT OUTSIDE THE PROJECT"),
        "reading through `..` returns the file's contents today:\n{text}"
    );
}

#[test]
fn file_read_accepts_an_absolute_path_out_of_the_working_directory() {
    let root = scratch("abs");
    let target = root.join("outside.txt");
    let program = root.join("project/read_abs.sz");
    // Serez string escapes: a Windows path's separators have to survive.
    let literal = target.display().to_string().replace('\\', "\\\\");
    std::fs::write(&program, format!("out File.read(\"{literal}\");\n")).expect("fixture");

    let (text, code) = run(&program, &root.join("project"));
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(code, 0, "an absolute path is accepted today:\n{text}");
    assert!(
        text.contains("CONTENT OUTSIDE THE PROJECT"),
        "an absolute path returns the file's contents today:\n{text}"
    );
}

#[test]
fn a_relative_file_path_is_measured_from_the_caller_but_an_import_is_not() {
    // The same program, run from two directories. `import` follows the file;
    // `File` follows whoever invoked `sz`. Nothing documents the difference, and
    // the failure it produces — "the script cannot find its own data file" —
    // does not look like a working-directory problem when you hit it.
    let root = scratch("cwd");
    let project = root.join("project");
    std::fs::write(&project.join("data.txt"), "DATA BESIDE THE SCRIPT\n").expect("fixture");
    std::fs::write(
        project.join("lib.sz"),
        "export fn int helper() { return 7; }\n",
    )
    .expect("fixture");
    let program = project.join("app.sz");
    std::fs::write(
        &program,
        "import \"./lib\";\nout \"import: \" + helper();\nout \"file: \" + File.read(\"./data.txt\");\n",
    )
    .expect("fixture");

    let (inside, inside_code) = run(&program, &project);
    let (above, above_code) = run(&program, &root);
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(
        inside_code, 0,
        "run from its own folder it works:\n{inside}"
    );
    assert!(inside.contains("DATA BESIDE THE SCRIPT"), "{inside}");

    assert_eq!(
        above_code, 1,
        "run from one level up the same program fails:\n{above}"
    );
    assert!(
        above.contains("import: 7"),
        "the import still resolved — it follows the file:\n{above}"
    );
    assert!(
        above.contains("SZ4005"),
        "and only the File read failed — it follows the caller:\n{above}"
    );
}

#[test]
fn a_path_that_is_not_there_still_fails_with_a_structured_io_error() {
    // The half the two conformance programs really do test. Keeping it here as
    // well means the tests above cannot be read as "reads always succeed".
    let root = scratch("missing");
    let program = root.join("project/read_missing.sz");
    std::fs::write(
        &program,
        "out File.read(\"../__no_such_file_for_the_path_tests__\");\n",
    )
    .expect("fixture");

    let (text, code) = run(&program, &root.join("project"));
    let _ = std::fs::remove_dir_all(&root);

    assert_eq!(code, 1, "a missing path fails:\n{text}");
    assert!(
        text.contains("SZ4005"),
        "and fails as a structured IOError, not as an empty string:\n{text}"
    );
}
