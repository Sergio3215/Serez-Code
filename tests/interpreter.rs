/// Rust integration tests — verify the *interpreter implementation* is correct.
///
/// These are distinct from the `.sz` test files in this directory, which test
/// *language features* from the user's perspective.  Here we drive the `sz`
/// binary as a black-box and assert on stdout / stderr.
use std::fs;
use std::process::Command;

const SZ: &str = env!("CARGO_BIN_EXE_sz");

/// Each test runs in its own thread; use the thread-ID as the temp-file suffix
/// so parallel tests never collide on the same path.
fn run(code: &str) -> (String, String) {
    let tid = format!("{:?}", std::thread::current().id());
    let tid: String = tid.chars().filter(|c| c.is_alphanumeric()).collect();
    let tmp = std::env::temp_dir().join(format!("serez_itest_{tid}.sz"));
    fs::write(&tmp, code).unwrap();
    let out = Command::new(SZ)
        .arg(tmp.to_str().unwrap())
        .output()
        .expect("failed to run sz");
    fs::remove_file(&tmp).ok();
    (
        String::from_utf8_lossy(&out.stdout).trim_end().to_string(),
        String::from_utf8_lossy(&out.stderr).trim_end().to_string(),
    )
}

fn stdout(code: &str) -> String {
    let (out, err) = run(code);
    assert!(err.is_empty(), "unexpected stderr:\n{err}\nfor code:\n{code}");
    out
}

// ── Arithmetic ─────────────────────────────────────────────────────────────────

#[test]
fn integer_arithmetic() {
    assert_eq!(stdout("out 2 + 3;"), "5");
    assert_eq!(stdout("out 10 - 4;"), "6");
    assert_eq!(stdout("out 3 * 7;"), "21");
    assert_eq!(stdout("out 8 / 2;"), "4");
    assert_eq!(stdout("out 10 % 3;"), "1");
}

#[test]
fn decimal_arithmetic() {
    assert_eq!(stdout("out 1.5 + 2.5;"), "4.0");
    assert_eq!(stdout("out 3.14 * 2.0;"), "6.28");
}

#[test]
fn power_and_bitwise() {
    assert_eq!(stdout("out 2 ** 10;"), "1024");
    assert_eq!(stdout("out 0xFF & 0x0F;"), "15");
    assert_eq!(stdout("out 1 << 4;"), "16");
}

// ── Variables ──────────────────────────────────────────────────────────────────

#[test]
fn let_and_assign() {
    assert_eq!(stdout("let x = 5; x = 10; out x;"), "10");
}

#[test]
fn const_immutable() {
    let (_, err) = run("const PI = 3.14; PI = 1.0;");
    assert!(!err.is_empty(), "expected a runtime error for const reassignment");
    assert!(err.contains("PI") || err.contains("const") || err.contains("immutable"),
        "error should mention PI or const, got: {err}");
}

#[test]
fn string_interpolation() {
    assert_eq!(stdout(r#"let name = "World"; out "Hello, {name}!";"#), "Hello, World!");
}

// ── Control flow ───────────────────────────────────────────────────────────────

#[test]
fn if_else() {
    assert_eq!(stdout(r#"if (3 > 2) { out "yes"; } else { out "no"; }"#), "yes");
}

#[test]
fn while_loop() {
    let code = "let i = 0; while (i < 3) { out i; i = i + 1; }";
    assert_eq!(stdout(code), "0\n1\n2");
}

#[test]
fn for_loop() {
    assert_eq!(stdout("for (let i = 0; i < 3; i = i + 1) { out i; }"), "0\n1\n2");
}

#[test]
fn foreach_array() {
    assert_eq!(stdout("let a = [10, 20, 30]; for (let x in a) { out x; }"), "10\n20\n30");
}

// ── Functions ──────────────────────────────────────────────────────────────────

#[test]
fn function_return() {
    let code = "fn int add(int a, int b) { return a + b; } out add(3, 4);";
    assert_eq!(stdout(code), "7");
}

#[test]
fn closures_capture_by_reference() {
    // Named fn declarations use reference semantics: rebinding outer `x`
    // must be visible inside `getX()`.
    let code = "let x = 10;\nfn getX() { return x; }\nout getX();\nx = 20;\nout getX();";
    assert_eq!(stdout(code), "10\n20");
}

#[test]
fn default_params() {
    let code = "fn string greet(string name = \"World\") { return \"Hello, {name}!\"; }\nout greet();\nout greet(\"Sergio\");";
    assert_eq!(stdout(code), "Hello, World!\nHello, Sergio!");
}

// ── Arrays ─────────────────────────────────────────────────────────────────────

#[test]
fn array_push_pop() {
    let code = "let a = [1, 2, 3]; a.push(4); out a.length; out a.pop();";
    assert_eq!(stdout(code), "4\n4");
}

#[test]
fn array_map_filter() {
    let code = "let a = [1,2,3,4,5];\nlet d = a.map(x => x * 2);\nlet e = a.filter(x => x % 2 == 0);\nout d.join(\", \");\nout e.join(\", \");";
    assert_eq!(stdout(code), "2, 4, 6, 8, 10\n2, 4");
}

// ── Classes ────────────────────────────────────────────────────────────────────

#[test]
fn class_constructor_and_method() {
    let code = "class Point {\n  public Point(int x, int y) { this.x = x; this.y = y; }\n  public decimal dist() { return Math.sqrt(this.x * this.x + this.y * this.y); }\n}\nlet p = new Point(3, 4);\nout p.dist();";
    assert_eq!(stdout(code), "5.0");
}

#[test]
fn inheritance_override() {
    let code = "class A {\n  public string greet() { return \"A\"; }\n}\nclass B : A {\n  public string greet() { return \"B\"; }\n}\nlet b = new B();\nout b.greet();";
    assert_eq!(stdout(code), "B");
}

// ── Error handling ─────────────────────────────────────────────────────────────

#[test]
fn try_catch_user_exception() {
    let code = "try {\n  throw \"oops\";\n} catch (e) {\n  out \"caught: {e}\";\n}";
    assert_eq!(stdout(code), "caught: oops");
}

#[test]
fn runtime_error_not_catchable() {
    let (_, err) = run("try { let a = [1]; out a[99]; } catch (e) { out \"caught\"; }");
    assert!(err.contains("❌"), "expected runtime error, got: {err}");
}

// ── Type system ────────────────────────────────────────────────────────────────

#[test]
fn is_operator() {
    assert_eq!(stdout("out (42 is int);"), "true");
    assert_eq!(stdout("out (\"hi\" is string);"), "true");
    assert_eq!(stdout("out (3.14 is decimal);"), "true");
    assert_eq!(stdout("out (null is null);"), "true");
    assert_eq!(stdout("out (42 is string);"), "false");
}

// ── Namespaces ─────────────────────────────────────────────────────────────────

#[test]
fn math_namespace() {
    assert_eq!(stdout("out Math.abs(-5);"), "5");
    assert_eq!(stdout("out Math.sqrt(9.0);"), "3.0");
    assert_eq!(stdout("out Math.pow(2.0, 8.0);"), "256.0");
}

#[test]
fn json_roundtrip() {
    // Dict literals use ({"key", value}) syntax; use JSON.parse to create from JSON string
    let code = "let s = \"[10, 20, 30]\";\nlet back = JSON.parse(s);\nout back[1];\nlet arr = [1, 2, 3];\nlet rt = JSON.parse(JSON.stringify(arr));\nout rt[2];";
    assert_eq!(stdout(code), "20\n3");
}
