//! Regression tests for the structured program-evaluation boundary.
//!
//! The evaluator still uses `EvalResult` internally, but a complete program
//! must expose why it stopped without forcing embedders to parse stderr.

use serez_code::ast::Program;
use serez_code::evaluator::{Evaluator, InvalidControlFlow, ProgramOutcome};
use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::run::{RunFailure, RunOpts, run_source, run_source_detailed};

fn parse(src: &str) -> Program {
    let lexer = Lexer::new(src.to_string());
    let mut parser = Parser::new(lexer);
    parser.set_source(src.lines().map(str::to_string).collect());
    parser.set_source_name("<runtime-outcome>");
    let program = parser.parse_program();
    assert!(
        !parser.has_errors(),
        "fixture must parse cleanly: {:?}",
        parser.take_errors()
    );
    program
}

fn evaluate(src: &str) -> ProgramOutcome {
    evaluate_with_permissions(src, &[])
}

fn evaluate_with_permissions(src: &str, permissions: &[&str]) -> ProgramOutcome {
    let program = parse(src);
    let mut evaluator = Evaluator::new();
    evaluator.set_source(src.lines().map(str::to_string).collect());
    evaluator.set_permissions(
        permissions
            .iter()
            .map(|permission| (*permission).to_string())
            .collect(),
    );
    evaluator.eval_program_outcome(&program)
}

fn evaluate_with_permissions_and_lockdown(src: &str, permissions: &[&str]) -> ProgramOutcome {
    let program = parse(src);
    let mut evaluator = Evaluator::new();
    evaluator.set_source(src.lines().map(str::to_string).collect());
    evaluator.set_permissions(
        permissions
            .iter()
            .map(|permission| (*permission).to_string())
            .collect(),
    );
    evaluator.set_lockdown(true);
    evaluator.eval_program_outcome(&program)
}

#[test]
fn runtime_error_keeps_its_stable_payload() {
    match evaluate("out(1 / 0);") {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4004");
            assert_eq!(error.kind, "DivisionByZero");
            assert_eq!(error.message, "Division by zero");
        }
        other => panic!("expected structured runtime error, got {other:?}"),
    }
}

#[test]
fn user_throw_is_not_a_runtime_error() {
    match evaluate("throw \"boom\";") {
        ProgramOutcome::UncaughtException { message } => assert_eq!(message, "boom"),
        other => panic!("expected uncaught user exception, got {other:?}"),
    }
}

#[test]
fn top_level_control_flow_has_its_own_outcome() {
    let cases = [
        ("return 1;", InvalidControlFlow::Return),
        ("break;", InvalidControlFlow::Break),
        ("continue;", InvalidControlFlow::Continue),
    ];

    for (src, expected) in cases {
        match evaluate(src) {
            ProgramOutcome::InvalidControlFlow(actual) => assert_eq!(actual, expected),
            other => panic!("{src}: expected invalid control flow, got {other:?}"),
        }
    }
}

#[test]
fn caught_runtime_error_does_not_replace_later_control_flow() {
    let src = r#"
        try {
            out(1 / 0);
        } catch (e) {}
        return 1;
    "#;

    assert!(matches!(
        evaluate(src),
        ProgramOutcome::InvalidControlFlow(InvalidControlFlow::Return)
    ));
}

#[test]
fn evaluator_reuse_never_reuses_a_stale_runtime_error() {
    let first_src = "out(1 / 0);";
    let legacy_src = "let value = new MissingClass();";
    let first = parse(first_src);
    let legacy = parse(legacy_src);
    let mut evaluator = Evaluator::new();

    evaluator.set_source(first_src.lines().map(str::to_string).collect());
    assert!(matches!(
        evaluator.eval_program_outcome(&first),
        ProgramOutcome::RuntimeError(_)
    ));

    // A later ReferenceError must replace, rather than inherit, the earlier
    // division payload when the evaluator instance is reused.
    evaluator.set_source(legacy_src.lines().map(str::to_string).collect());
    match evaluator.eval_program_outcome(&legacy) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4001");
            assert_eq!(error.kind, "ReferenceError");
            assert!(error.message.contains("MissingClass"));
        }
        other => panic!("expected a fresh class ReferenceError, got {other:?}"),
    }
}

#[test]
fn construction_validation_errors_are_structured_and_catchable() {
    let cases = [
        (
            "new MissingConstructionTarget();",
            "SZ4001",
            "ReferenceError",
            "Unknown class or interface",
        ),
        (
            "interface PositionalIface { value: int; } new PositionalIface(1);",
            "SZ4002",
            "TypeError",
            "must be instantiated",
        ),
        (
            "interface ExtraIface { value: int; } new ExtraIface({ value: 1, extra: 2 });",
            "SZ4002",
            "TypeError",
            "not declared",
        ),
        (
            "interface MissingIface { value: int; other: int; } new MissingIface({ value: 1 });",
            "SZ4002",
            "TypeError",
            "Missing field",
        ),
        (
            "interface TypedIface { value: int; } new TypedIface({ value: \"wrong\" });",
            "SZ4002",
            "TypeError",
            "expects 'int'",
        ),
        (
            "abstract class AbstractTarget {} new AbstractTarget();",
            "SZ4002",
            "TypeError",
            "Cannot instantiate abstract",
        ),
        (
            "class PositionalClass {} new PositionalClass({ value: 1 });",
            "SZ4002",
            "TypeError",
            "uses positional arguments",
        ),
        (
            "class NeedsArg { public NeedsArg(int value) {} } new NeedsArg();",
            "SZ4002",
            "TypeError",
            "Constructor 'NeedsArg' expects",
        ),
        (
            "class NoConstructor {} new NoConstructor(1);",
            "SZ4002",
            "TypeError",
            "has no constructor",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured construction error, got {other:?}"),
        }
    }

    let caught = r#"
        interface CatchIface { value: int; other: int; }
        interface CatchTypedIface { value: int; }
        abstract class CatchAbstract {}
        class CatchPositional {}
        class CatchNeedsArg { public CatchNeedsArg(int value) {} }
        class CatchNoConstructor {}

        let caughtCount = 0;
        try { new CatchMissing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        try { new CatchIface(1); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchIface({ value: 1, other: 2, extra: 3 }); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchIface({ value: 1 }); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchTypedIface({ value: "wrong" }); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchAbstract(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchPositional({ value: 1 }); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchNeedsArg(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchNoConstructor(1); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }

        if (caughtCount != 9) { throw "construction validation errors were not catchable"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn super_validation_errors_are_structured_and_catchable() {
    let cases = [
        (
            "super();",
            "SZ4002",
            "TypeError",
            "outside of a constructor",
        ),
        (
            "super.missing();",
            "SZ4002",
            "TypeError",
            "outside of a class method",
        ),
        (
            "class RootCtor { public RootCtor() { super(); } } new RootCtor();",
            "SZ4002",
            "TypeError",
            "has no parent",
        ),
        (
            "class RootMethod { public any fail() { return super.missing(); } } new RootMethod().fail();",
            "SZ4002",
            "TypeError",
            "has no parent",
        ),
        (
            "class MethodParent {} class MethodChild : MethodParent { public any fail() { return super.missing(); } } new MethodChild().fail();",
            "SZ4001",
            "ReferenceError",
            "has no method",
        ),
        (
            "class CtorParent { public CtorParent(int value) {} } class CtorChild : CtorParent { public CtorChild() { super(); } } new CtorChild();",
            "SZ4002",
            "TypeError",
            "super() for 'CtorParent' expects",
        ),
        (
            "class ArityParent { public int read(int value) { return value; } } class ArityChild : ArityParent { public int fail() { return super.read(); } } new ArityChild().fail();",
            "SZ4002",
            "TypeError",
            "Method 'ArityParent::read' expects",
        ),
        (
            "class RequiredParent { public RequiredParent(int value) {} } class ImplicitChild : RequiredParent {} new ImplicitChild();",
            "SZ4002",
            "TypeError",
            "cannot be chained automatically",
        ),
        (
            "class EmptyParent {} class EmptyChild : EmptyParent { public EmptyChild() { super(1); } } new EmptyChild();",
            "SZ4002",
            "TypeError",
            "has no constructor",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured super error, got {other:?}"),
        }
    }

    let caught = r#"
        class CatchRootCtor { public CatchRootCtor() { super(); } }
        class CatchRootMethod { public any fail() { return super.missing(); } }
        class CatchMethodParent {}
        class CatchMethodChild : CatchMethodParent {
            public any fail() { return super.missing(); }
        }
        class CatchCtorParent { public CatchCtorParent(int value) {} }
        class CatchCtorChild : CatchCtorParent { public CatchCtorChild() { super(); } }
        class CatchArityParent { public int read(int value) { return value; } }
        class CatchArityChild : CatchArityParent {
            public int fail() { return super.read(); }
        }
        class CatchRequiredParent { public CatchRequiredParent(int value) {} }
        class CatchImplicitChild : CatchRequiredParent {}
        class CatchEmptyParent {}
        class CatchEmptyChild : CatchEmptyParent { public CatchEmptyChild() { super(1); } }

        let caughtCount = 0;
        try { super(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { super.missing(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchRootCtor(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchRootMethod().fail(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchMethodChild().fail(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        try { new CatchCtorChild(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchArityChild().fail(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchImplicitChild(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { new CatchEmptyChild(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }

        if (caughtCount != 9) { throw "super validation errors were not catchable"; }

        class HealthyParent { public int read() { return 7; } }
        class HealthyChild : HealthyParent { public int readParent() { return super.read(); } }
        if (new HealthyChild().readParent() != 7) { throw "super cleanup corrupted later dispatch"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn member_dispatch_errors_are_structured_and_catchable() {
    let cases = [
        (
            "class MissingMember {} new MissingMember().missing();",
            "SZ4001",
            "ReferenceError",
            "has no field or method",
        ),
        (
            "class InstanceArity { public int read(int value) { return value; } } new InstanceArity().read();",
            "SZ4002",
            "TypeError",
            "expects 1 argument",
        ),
        (
            "class StaticArity { public static int read(int value) { return value; } } StaticArity.read();",
            "SZ4002",
            "TypeError",
            "expects 1 argument",
        ),
        (
            "class PrivateCall { private int secret() { return 1; } } new PrivateCall().secret();",
            "SZ4002",
            "TypeError",
            "private and cannot be called externally",
        ),
        (
            "class PrivateReference { private int secret() { return 1; } } let callback = new PrivateReference().secret;",
            "SZ4002",
            "TypeError",
            "private and cannot be referenced externally",
        ),
        (
            "class BadReturn { public int read() { return \"wrong\"; } } new BadReturn().read();",
            "SZ4002",
            "TypeError",
            "declared return 'int'",
        ),
        (
            "class MissingStatic {} MissingStatic.missing();",
            "SZ4001",
            "ReferenceError",
            "has no static method",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured dispatch error, got {other:?}"),
        }
    }

    let caught = r#"
        class DispatchTarget {
            private int secret() { return 1; }
            public int read(int value) { return value; }
            public int bad() { return "wrong"; }
            public static int staticRead(int value) { return value; }
        }

        let target = new DispatchTarget();
        let caughtCount = 0;
        try { target.missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        try { target.read(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DispatchTarget.staticRead(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { target.secret(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { let callback = target.secret; }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { target.bad(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DispatchTarget.missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }

        if (caughtCount != 7) { throw "member dispatch errors were not catchable"; }
        if (target.read(7) != 7 || DispatchTarget.staticRead(8) != 8) {
            throw "member dispatch cleanup corrupted a later valid call";
        }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn property_dispatch_errors_are_structured_and_catchable() {
    let cases = [
        (
            "class ReadOnly { public ReadOnly() {} public get int value() { return 1; } } let item = new ReadOnly(); item.value = 2;",
            "getter-only property",
        ),
        (
            "let number = 1; number.value = 2;",
            "not a class or interface instance",
        ),
        (
            "class PrivateGetter { public PrivateGetter() {} private get int value() { return 1; } } let item = new PrivateGetter(); out item.value;",
            "private and cannot be called externally",
        ),
        (
            "class PrivateSetter { public PrivateSetter() {} private set value(int next) {} } let item = new PrivateSetter(); item.value = 1;",
            "private and cannot be called externally",
        ),
        (
            "class GetterArity { public GetterArity() {} public get int value(int extra) { return extra; } } let item = new GetterArity(); out item.value;",
            "expects 1 argument",
        ),
        (
            "class SetterArity { public SetterArity() {} public set value() {} } let item = new SetterArity(); item.value = 1;",
            "expects 0 argument",
        ),
        (
            "class GetterReturn { public GetterReturn() {} public get int value() { return \"wrong\"; } } let item = new GetterReturn(); out item.value;",
            "declared return 'int'",
        ),
    ];

    for (src, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4002", "{src}");
                assert_eq!(error.kind, "TypeError", "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured property error, got {other:?}"),
        }
    }

    let caught = r#"
        class PropertyTarget {
            public PropertyTarget() { this._value = 1; }
            public get int value() { return this._value; }
            private get int hidden() { return this._value; }
            private set hidden(int next) { this._value = next; }
            public set writable(int next) {
                if (next < 0) { throw "negative value"; }
                this._value = next;
            }
        }

        let item = new PropertyTarget();
        let caughtCount = 0;
        try { item.value = 2; }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { let number = 1; number.value = 2; }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { out item.hidden; }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { item.hidden = 3; }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { item.writable = -1; }
        catch (e) { if (e == "negative value") { caughtCount++; } }

        if (caughtCount != 5) { throw "property dispatch errors were not catchable"; }
        if (item.value != 1) { throw "failed property writes changed receiver state"; }
        item.writable = 7;
        if (item.value != 7) { throw "property cleanup corrupted a later valid setter"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn invalid_inheritance_graphs_are_rejected_without_poisoning_the_registry() {
    let cases = [
        (
            "class SelfCycle : SelfCycle {}",
            "SZ4002",
            "TypeError",
            "inheritance cycle",
        ),
        (
            "class CycleA : CycleB {} class CycleB : CycleA {}",
            "SZ4002",
            "TypeError",
            "inheritance cycle",
        ),
        (
            "class MissingParentChild : MissingParent {} new MissingParentChild();",
            "SZ4001",
            "ReferenceError",
            "Parent class 'MissingParent'",
        ),
        (
            "class MissingSuperChild : MissingSuperParent { public MissingSuperChild() { super(); } } new MissingSuperChild();",
            "SZ4001",
            "ReferenceError",
            "Parent class 'MissingSuperParent'",
        ),
        (
            "sealed class SealedParent {} class SealedChild : SealedParent {}",
            "SZ4002",
            "TypeError",
            "Cannot inherit from sealed class",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected inheritance error, got {other:?}"),
        }
    }

    let recovery = r#"
        let caughtCount = 0;
        try { class RejectedSelf : RejectedSelf {} }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }

        class ForwardChild : ForwardParent {}
        try { new ForwardChild(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }

        try { class ForwardParent : ForwardChild {} }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }

        class ForwardParent { public int read() { return 7; } }
        if (new ForwardChild().read() != 7) {
            throw "forward hierarchy did not recover after defining its parent";
        }

        sealed class RecoverySealed {}
        try { class RejectedSealedChild : RecoverySealed {} }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }

        class HealthyAfterInheritanceErrors { public int read() { return 8; } }
        if (new HealthyAfterInheritanceErrors().read() != 8) {
            throw "rejected hierarchy poisoned later class dispatch";
        }
        if (caughtCount != 4) { throw "inheritance errors were not catchable"; }
    "#;
    assert!(matches!(evaluate(recovery), ProgramOutcome::Value(_)));
}

#[test]
fn array_and_call_spread_type_errors_are_structured_and_catchable() {
    let uncaught_cases = [
        "let values = [...1];",
        "fn any collect(...items) { return items; } collect(...1);",
    ];

    for src in uncaught_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4002", "{src}");
                assert_eq!(error.kind, "TypeError", "{src}");
                assert!(error.message.contains("requires an array"), "{src}");
            }
            other => panic!("{src}: expected structured spread error, got {other:?}"),
        }
    }

    let caught = r#"
        fn any collect(...items) { return items; }
        let caughtArray = false;
        let caughtCall = false;
        try { let values = [...1]; }
        catch (e) { caughtArray = e.code == "SZ4002" && e.kind == "TypeError"; }
        try { collect(...1); }
        catch (e) { caughtCall = e.code == "SZ4002" && e.kind == "TypeError"; }
        if (!caughtArray || !caughtCall) { throw "spread TypeError was not catchable"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));

    let user_throw = r#"
        fn any explode() { throw "spread boom"; return []; }
        let values = [...explode()];
    "#;
    assert!(matches!(
        evaluate(user_throw),
        ProgramOutcome::UncaughtException { message } if message == "spread boom"
    ));
}

#[test]
fn iteration_and_destructuring_type_errors_are_structured_and_catchable() {
    let uncaught_cases = [
        ("for (let item in 1) {}", "for-in requires"),
        (
            "for (let [first] in [1]) {}",
            "for-in array destructuring requires",
        ),
        ("let [first] = 1;", "Array destructuring requires"),
        ("let {field} = 1;", "Object destructuring requires"),
    ];

    for (src, expected_message) in uncaught_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4002", "{src}");
                assert_eq!(error.kind, "TypeError", "{src}");
                assert!(error.message.contains(expected_message), "{src}");
            }
            other => panic!("{src}: expected structured type error, got {other:?}"),
        }
    }

    let caught = r#"
        let caughtCount = 0;
        try { for (let item in 1) {} }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { for (let [first] in [1]) {} }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { let [arrayItem] = 1; }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { let {objectField} = 1; }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        if (caughtCount != 4) { throw "iteration/destructuring TypeError was not catchable"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));

    let throw_cases = [
        r#"fn any explode() { throw "iterable boom"; return []; } for (let item in explode()) {}"#,
        r#"fn any explode() { throw "iterable boom"; return []; } let [item] = explode();"#,
        r#"fn any explode() { throw "iterable boom"; return null; } let {item} = explode();"#,
    ];
    for src in throw_cases {
        assert!(matches!(
            evaluate(src),
            ProgramOutcome::UncaughtException { message } if message == "iterable boom"
        ));
    }
}

#[test]
fn default_argument_user_throw_survives_all_call_paths() {
    let cases = [
        (
            "function-default",
            r#"
                fn int fail() { throw "function-default"; return 0; }
                fn int target(int value = fail()) { return value; }
                target();
            "#,
        ),
        (
            "callback-default",
            r#"
                class CallbackDefault {
                    public decimal fail() { throw "callback-default"; return 0.0; }
                    public decimal apply(decimal value, decimal ignored = this.fail()) {
                        return value;
                    }
                }
                let handler = new CallbackDefault();
                let source = GPU.createBufferFromArray([1.0]);
                GPU.map(source, handler.apply);
            "#,
        ),
        (
            "constructor-default",
            r#"
                fn int fail() { throw "constructor-default"; return 0; }
                class Box {
                    public Box(int value = fail()) { this.value = value; }
                }
                new Box();
            "#,
        ),
        (
            "super-constructor-default",
            r#"
                fn int fail() { throw "super-constructor-default"; return 0; }
                class Base {
                    public Base(int value = fail()) { this.value = value; }
                }
                class Child : Base {
                    public Child() { super(); }
                }
                new Child();
            "#,
        ),
        (
            "super-method-default",
            r#"
                fn int fail() { throw "super-method-default"; return 0; }
                class Base {
                    public int value(int fallback = fail()) { return fallback; }
                }
                class Child : Base {
                    public int read() { return super.value(); }
                }
                new Child().read();
            "#,
        ),
        (
            "method-default",
            r#"
                fn int fail() { throw "method-default"; return 0; }
                class Box {
                    public int read(int value = fail()) { return value; }
                }
                new Box().read();
            "#,
        ),
    ];

    for (expected, src) in cases {
        assert!(matches!(
            evaluate(src),
            ProgramOutcome::UncaughtException { message } if message == expected
        ));
    }
}

#[test]
fn default_argument_runtime_error_remains_structured_and_catchable() {
    let src = r#"
        fn int target(int value = 1 / 0) { return value; }
        let caught = false;
        try { target(); }
        catch (e) {
            caught = e.code == "SZ4004" && e.kind == "DivisionByZero";
        }
        if (!caught) { throw "default runtime error was not catchable"; }
    "#;
    assert!(matches!(evaluate(src), ProgramOutcome::Value(_)));
}

#[test]
fn math_type_errors_are_structured() {
    let cases = [
        "out(Math.sin(\"not-a-number\"));",
        "out(Math.min(1, \"not-a-number\"));",
        "out(Math.max(1, \"not-a-number\"));",
        "out(Math.pow(2, \"not-a-number\"));",
    ];

    for src in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4002", "{src}");
                assert_eq!(error.kind, "TypeError", "{src}");
                assert!(error.message.contains("expects numeric"), "{src}");
            }
            other => panic!("{src}: expected structured Math TypeError, got {other:?}"),
        }
    }
}

#[test]
fn math_arguments_preserve_user_throw() {
    let calls = [
        "Math.sin(mathBoom())",
        "Math.min(1, mathBoom())",
        "Math.max(1, mathBoom())",
        "Math.pow(2, mathBoom())",
    ];

    for call in calls {
        let src = format!(
            r#"
                fn decimal mathBoom() {{
                    throw "math-boom";
                    return 0.0;
                }}
                out({call});
            "#
        );
        match evaluate(&src) {
            ProgramOutcome::UncaughtException { message } => assert_eq!(message, "math-boom"),
            other => panic!("{call}: expected original user throw, got {other:?}"),
        }
    }
}

#[test]
fn ordinary_operator_errors_are_structured() {
    let cases = [
        ("out(-true);", "SZ4002", "TypeError"),
        ("out(2 ** 63);", "SZ4000", "Overflow"),
        ("out(8 >> -1);", "SZ4002", "TypeError"),
        ("out(1m + 0.5);", "SZ4002", "TypeError"),
        ("out(true + false);", "SZ4002", "TypeError"),
    ];

    for (src, expected_code, expected_kind) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
            }
            other => panic!("{src}: expected structured operator error, got {other:?}"),
        }
    }
}

#[test]
fn fatal_resource_error_is_structured_but_not_catchable() {
    let src = r#"
        try {
            out("x" * 20000000);
        } catch (e) {
            out("unreachable: resource limit was caught");
        }
        out("unreachable: execution continued");
    "#;

    match evaluate(src) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ6002");
            assert_eq!(error.kind, "ResourceError");
            assert!(error.message.contains("exceeds maximum"));
        }
        other => panic!("expected fatal structured resource error, got {other:?}"),
    }
}

#[test]
fn missing_namespace_permissions_are_structured_but_not_catchable() {
    let cases = vec![
        ("Terminal.getSize()", "Terminal", "Terminal"),
        ("OS.platform()", "OS", "OS"),
        ("Env.get(\"PATH\")", "Env", "Env"),
        ("Time.now()", "Time", "Time"),
        ("DateTime.now()", "DateTime.now", "Time"),
        ("DateTime.utcNow()", "DateTime.utcNow", "Time"),
        ("System.cpuCount()", "System", "System"),
        ("Socket.close(0)", "Socket", "Socket"),
        ("Task.message()", "Task", "Task"),
        ("Gui.measureText(\"x\", 1)", "Gui", "Gui"),
    ];
    #[cfg(feature = "audio")]
    let cases = {
        let mut cases = cases;
        cases.push(("Media.isPlaying(0)", "Media", "Media"));
        cases
    };

    for (call, operation, permission) in cases {
        let src = format!(
            r#"
                try {{
                    {call};
                }} catch (e) {{
                    out("unreachable: permission denial was caught");
                }}
                out("unreachable: execution continued");
            "#
        );

        match evaluate(&src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ6001", "{call}");
                assert_eq!(error.kind, "PermissionError", "{call}");
                assert!(
                    error.message.contains(operation),
                    "{call}: {}",
                    error.message
                );
                assert!(
                    error.message.contains(permission),
                    "{call}: {}",
                    error.message
                );
            }
            other => panic!("{call}: expected fatal permission error, got {other:?}"),
        }
    }
}

#[test]
fn unsafe_gates_are_structured_but_not_catchable() {
    let cases = [
        ("File.delete()", "File.delete"),
        ("File.rename()", "File.rename"),
        ("Memory.alloc()", "Memory.alloc"),
        ("Memory.free()", "Memory.free"),
        ("Memory.read()", "Memory.read"),
        ("Memory.write()", "Memory.write"),
        ("Memory.copy()", "Memory.copy"),
        ("Memory.fill()", "Memory.fill"),
        ("Terminal.setRawMode()", "Terminal.setRawMode"),
        ("Terminal.readByte(0)", "Terminal.readByte"),
        ("Terminal.enableMouse()", "Terminal.enableMouse"),
        ("Terminal.readEvent(0)", "Terminal.readEvent"),
        ("OS.exec()", "OS.exec"),
        ("OS.spawn()", "OS.spawn"),
        ("OS.kill()", "OS.kill"),
        ("Env.set()", "Env.set"),
        ("let x = 0; let p = &x; *p = 1", "Pointer write"),
    ];

    for (body, operation) in cases {
        let src = format!(
            r#"
                try {{
                    {body};
                }} catch (e) {{
                    out("unreachable: unsafe gate was caught");
                }}
                out("unreachable: execution continued");
            "#
        );

        match evaluate_with_permissions(&src, &["Terminal", "OS", "Env"]) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ6003", "{body}");
                assert_eq!(error.kind, "UnsafeError", "{body}");
                assert!(
                    error.message.contains(operation),
                    "{body}: {}",
                    error.message
                );
                assert!(error.message.contains("unsafe"), "{body}");
            }
            other => panic!("{body}: expected fatal unsafe error, got {other:?}"),
        }
    }
}

#[test]
fn call_depth_limit_is_structured_across_all_call_paths() {
    let cases = [
        r#"
            fn int recurse(int n) { return recurse(n + 1); }
            try { recurse(0); } catch (e) { out("unreachable: depth caught"); }
        "#,
        r#"
            class Loop {
                public int recurse(int n) { return this.recurse(n + 1); }
            }
            let loop = new Loop();
            try { loop.recurse(0); } catch (e) { out("unreachable: depth caught"); }
        "#,
        r#"
            class BaseLoop {
                public int recurse(int n) { return this.recurse(n + 1); }
            }
            class ChildLoop : BaseLoop {
                public int recurse(int n) { return super.recurse(n + 1); }
            }
            let loop = new ChildLoop();
            try { loop.recurse(0); } catch (e) { out("unreachable: depth caught"); }
        "#,
        r#"
            fn int callbackLoop(int n) { return [n].map(callbackLoop)[0]; }
            try { [0].map(callbackLoop); } catch (e) { out("unreachable: depth caught"); }
        "#,
        r#"
            class OperatorLoop {
                public OperatorLoop op_add(OperatorLoop other) { return this + other; }
            }
            let left = new OperatorLoop();
            let right = new OperatorLoop();
            try { left + right; } catch (e) { out("unreachable: depth caught"); }
        "#,
    ];

    std::thread::Builder::new()
        .name("runtime-call-depth-regression".to_string())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            for src in cases {
                match evaluate(src) {
                    ProgramOutcome::RuntimeError(error) => {
                        assert_eq!(error.code, "SZ6002");
                        assert_eq!(error.kind, "ResourceError");
                        assert!(error.message.contains("maximum call depth"));
                    }
                    other => panic!("expected fatal call-depth error, got {other:?}"),
                }
            }
        })
        .expect("call-depth regression thread must start")
        .join()
        .expect("call-depth regression must not panic");
}

#[test]
fn allocation_limits_and_dimension_overflow_are_structured_and_fatal() {
    let cases = [
        (
            r#"try { unsafe { Memory.alloc(268435457); } } catch (e) { out("unreachable: Memory limit caught"); }"#,
            "Memory.alloc",
        ),
        (
            r#"try { GPU.createBuffer(33554433); } catch (e) { out("unreachable: GPU limit caught"); }"#,
            "GPU.createBuffer",
        ),
        (
            r#"try { new Tensor([4000, 4000], 0.0); } catch (e) { out("unreachable: Tensor limit caught"); }"#,
            "Tensor size",
        ),
        (
            r#"try { new Tensor([9223372036854775807, 3], 0.0); } catch (e) { out("unreachable: Tensor overflow caught"); }"#,
            "Tensor shape",
        ),
        (
            r#"
                let a = GPU.createBuffer(1);
                let b = GPU.createBuffer(1);
                try {
                    GPU.matmul(a, 9223372036854775807, 3, b, 3, 1);
                } catch (e) { out("unreachable: GPU overflow caught"); }
            "#,
            "GPU.matmul",
        ),
    ];

    for (src, operation) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ6002", "{operation}");
                assert_eq!(error.kind, "ResourceError", "{operation}");
                assert!(
                    error.message.contains(operation),
                    "{operation}: {}",
                    error.message
                );
            }
            other => panic!("{operation}: expected fatal resource error, got {other:?}"),
        }
    }

    // Zero is invalid input, not resource exhaustion, and stays recoverable.
    let zero_size = r#"
        let caught = false;
        try {
            unsafe { Memory.alloc(0); }
        } catch (e) {
            if (e.code == "SZ4002" && e.kind == "TypeError") { caught = true; }
        }
        if (!caught) { throw "Memory.alloc(0) did not stay catchable"; }
    "#;
    assert!(matches!(evaluate(zero_size), ProgramOutcome::Value(_)));
}

#[test]
fn oversized_file_reads_are_structured_and_fatal_without_reading_contents() {
    let path =
        std::env::temp_dir().join(format!("serez-oversized-read-{}.bin", std::process::id()));
    let file = std::fs::File::create(&path).expect("create sparse resource-limit fixture");
    file.set_len(256 * 1024 * 1024 + 1)
        .expect("set sparse fixture length");
    drop(file);

    let escaped_path = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let outcomes: Vec<_> = ["File.read", "File.read_asBinary"]
        .into_iter()
        .map(|operation| {
            let src = format!(
                r#"
                    try {{ {operation}("{escaped_path}"); }}
                    catch (e) {{ out("unreachable: file limit caught"); }}
                "#
            );
            (operation, evaluate(&src))
        })
        .collect();
    let _ = std::fs::remove_file(&path);

    for (operation, outcome) in outcomes {
        match outcome {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ6002", "{operation}");
                assert_eq!(error.kind, "ResourceError", "{operation}");
                assert!(error.message.contains("maximum read size"));
            }
            other => panic!("{operation}: expected fatal file resource error, got {other:?}"),
        }
    }
}

#[test]
fn protected_process_targets_are_structured_but_not_catchable() {
    let calls = [
        r#"OS.exec("C:\\Windows\\System32\\cmd.exe", [])"#,
        r#"OS.spawn("C:\\Windows\\System32\\cmd.exe", [])"#,
    ];

    for call in calls {
        let src = format!(
            r#"
                unsafe {{
                    try {{ {call}; }}
                    catch (e) {{ out("unreachable: protected path caught"); }}
                }}
                out("unreachable: execution continued");
            "#
        );
        match evaluate_with_permissions(&src, &["OS"]) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ6004", "{call}");
                assert_eq!(error.kind, "SecurityError", "{call}");
                assert!(error.message.contains("protected system path"));
            }
            other => panic!("{call}: expected fatal security error, got {other:?}"),
        }
    }
}

#[test]
fn exact_decimal_arithmetic_errors_are_structured() {
    let cases = [
        ("out(1m / 0m);", "SZ4004", "DivisionByZero"),
        ("out(5m % 0m);", "SZ4004", "DivisionByZero"),
        ("out(2m ** 1.5m);", "SZ4002", "TypeError"),
        ("out(Dec.MAX + 1m);", "SZ4000", "Overflow"),
    ];

    for (src, expected_code, expected_kind) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
            }
            other => panic!("{src}: expected structured decimal error, got {other:?}"),
        }
    }
}

#[test]
fn datetime_failures_are_structured_and_classified() {
    let cases = [
        (
            "DateTime.from(2026, 1);",
            "SZ4002",
            "TypeError",
            "takes 3 to 7 integers",
        ),
        (
            "DateTime.from(\"2026\", 1, 1);",
            "SZ4002",
            "TypeError",
            "expects integer arguments",
        ),
        (
            "DateTime.from(2026, 13, 1);",
            "SZ4000",
            "RangeError",
            "out-of-range field",
        ),
        (
            "DateTime.from(2026, 2, 30);",
            "SZ4000",
            "RangeError",
            "invalid calendar date",
        ),
        (
            "DateTime.fromEpoch();",
            "SZ4002",
            "TypeError",
            "requires 1 integer",
        ),
        (
            "DateTime.fromEpoch(9223372036854775807);",
            "SZ4000",
            "RangeError",
            "out-of-range timestamp",
        ),
        (
            "DateTime.missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown DateTime method",
        ),
        (
            "DateTime.from(2026, 1, 1).format();",
            "SZ4002",
            "TypeError",
            "requires 1 string argument",
        ),
        (
            "DateTime.from(2026, 1, 1).format(1);",
            "SZ4002",
            "TypeError",
            "requires a string pattern",
        ),
        (
            "DateTime.from(2026, 1, 1).missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown DateTime field/method",
        ),
        (
            "DateTime.from(2026, 1, 1).day.add();",
            "SZ4002",
            "TypeError",
            "requires 1 integer",
        ),
        (
            "DateTime.from(2026, 1, 1).day.add(\"x\");",
            "SZ4002",
            "TypeError",
            "expects integer arguments",
        ),
        (
            "DateTime.from(2026, 1, 1).day.add(9223372036854775807);",
            "SZ4000",
            "Overflow",
            "overflowed the representable date range",
        ),
        (
            "DateTime.from(2026, 1, 1).day.missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown DateField method",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured DateTime error, got {other:?}"),
        }
    }
}

#[test]
fn datetime_errors_are_catchable_and_arity_is_enforced_before_arguments() {
    let src = r#"
        use permissions { Time }

        let caughtCount = 0;
        try { DateTime.from(2026, 2, 30); }
        catch (e) { if (e.code == "SZ4000" && e.kind == "RangeError") { caughtCount++; } }
        try { DateTime.from("bad", 1, 1); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DateTime.missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }

        let touched = 0;
        fn int touch() { touched++; return 1; }
        try { DateTime.from(touch()); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DateTime.fromEpoch(touch(), touch()); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DateTime.now(touch()); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DateTime.from(2026, 1, 1).year(touch()); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DateTime.from(2026, 1, 1).timestamp(touch()); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DateTime.from(2026, 1, 1).day.toInt(touch()); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { DateTime.from(2026, 1, 1).day.add(touch(), touch()); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }

        if (caughtCount != 10) { throw "DateTime errors were not catchable"; }
        if (touched != 0) { throw "invalid-arity calls evaluated arguments"; }
        if (DateTime.from(2026, 1, 1).day.add(1).day != 2) {
            throw "DateTime did not recover after caught errors";
        }
    "#;

    assert!(matches!(evaluate(src), ProgramOutcome::Value(_)));
}

#[test]
fn datetime_arguments_preserve_nested_runtime_errors_and_user_throws() {
    let runtime_cases = [
        "DateTime.from(1 / 0, 1, 1);",
        "DateTime.fromEpoch(1 / 0);",
        "DateTime.from(2026, 1, 1).format(1 / 0);",
        "DateTime.from(2026, 1, 1).day.add(1 / 0);",
    ];
    for src in runtime_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4004", "{src}");
                assert_eq!(error.kind, "DivisionByZero", "{src}");
            }
            other => panic!("{src}: expected original runtime error, got {other:?}"),
        }
    }

    let calls = [
        "DateTime.from(dateBoom(), 1, 1)",
        "DateTime.fromEpoch(dateBoom())",
        "DateTime.from(2026, 1, 1).format(dateBoom())",
        "DateTime.from(2026, 1, 1).day.add(dateBoom())",
    ];
    for call in calls {
        let src = format!(
            r#"
                fn int dateBoom() {{ throw "date-boom"; return 0; }}
                {call};
            "#
        );
        match evaluate(&src) {
            ProgramOutcome::UncaughtException { message } => assert_eq!(message, "date-boom"),
            other => panic!("{call}: expected original user throw, got {other:?}"),
        }
    }
}

#[test]
fn random_failures_are_structured_and_catchable() {
    let cases = [
        ("Random.seed();", "SZ4002", "TypeError", "requires 1"),
        (
            "Random.seed(\"bad\");",
            "SZ4002",
            "TypeError",
            "requires an integer",
        ),
        (
            "Random.int(2, 1);",
            "SZ4000",
            "RangeError",
            "min (2) must be <= max (1)",
        ),
        (
            "Random.uniform(1.0, 1.0);",
            "SZ4000",
            "RangeError",
            "lo must be < hi",
        ),
        (
            "Random.normal(0.0, 0.0 - 1.0);",
            "SZ4000",
            "RangeError",
            "std must be non-negative",
        ),
        (
            "Random.choice([]);",
            "SZ4000",
            "RangeError",
            "array is empty",
        ),
        (
            "Random.bernoulli(2.0);",
            "SZ4000",
            "RangeError",
            "p must be in [0, 1]",
        ),
        (
            "Random.normalTensor([0], 0.0, 1.0);",
            "SZ4000",
            "RangeError",
            "dimensions must be positive",
        ),
        (
            "Random.missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown Random method",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured Random error, got {other:?}"),
        }
    }

    let caught = r#"
        let caughtCount = 0;
        try { Random.int(1); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { Random.choice([]); }
        catch (e) { if (e.code == "SZ4000" && e.kind == "RangeError") { caughtCount++; } }
        try { Random.missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        if (caughtCount != 3) { throw "Random errors were not catchable"; }
        Random.seed(1);
        Random.int(0, 1);
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn random_arguments_preserve_nested_outcomes_and_arity_short_circuits() {
    let runtime_cases = [
        "Random.seed(1 / 0);",
        "Random.int(0, 1 / 0);",
        "Random.uniform(0.0, 1 / 0);",
        "Random.normal(0.0, 1 / 0);",
        "Random.shuffle(1 / 0);",
        "Random.choice(1 / 0);",
        "Random.bernoulli(1 / 0);",
    ];
    for src in runtime_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4004", "{src}");
                assert_eq!(error.kind, "DivisionByZero", "{src}");
            }
            other => panic!("{src}: expected original Random argument error, got {other:?}"),
        }
    }

    let user_throw = r#"
        fn int randomBoom() { throw "random-boom"; return 0; }
        Random.int(0, randomBoom());
    "#;
    assert!(matches!(
        evaluate(user_throw),
        ProgramOutcome::UncaughtException { message } if message == "random-boom"
    ));

    let arity = r#"
        let touched = 0;
        fn int touch() { touched++; return 1; }
        try { Random.decimal(touch()); }
        catch (e) {
            if (e.code != "SZ4002" || e.kind != "TypeError") { throw "wrong arity error"; }
        }
        if (touched != 0) { throw "Random evaluated arguments after arity failure"; }
    "#;
    assert!(matches!(evaluate(arity), ProgramOutcome::Value(_)));
}

#[test]
fn random_int_supports_the_complete_i64_domain_without_changing_small_sequences() {
    let src = r#"
        Random.seed(12345);
        if (Random.int(0, 1000) != 181) { throw "small draw 1 changed"; }
        if (Random.int(0, 1000) != 242) { throw "small draw 2 changed"; }
        if (Random.int(0, 1000) != 79) { throw "small draw 3 changed"; }

        let min = (0 - 9223372036854775807) - 1;
        let max = 9223372036854775807;
        let sawWide = false;
        for (let i = 0; i < 32; i++) {
            let value = Random.int(min, max);
            if (value < min || value > max) { throw "draw escaped i64 bounds"; }
            if (value < (0 - 2147483648) || value > 2147483647) { sawWide = true; }
        }
        if (!sawWide) { throw "full-domain draws remained truncated to 31 bits"; }
    "#;

    assert!(matches!(evaluate(src), ProgramOutcome::Value(_)));
}

#[test]
fn string_method_failures_are_structured_and_catchable() {
    let cases = [
        (
            "\"abc\".startsWith();",
            "SZ4002",
            "TypeError",
            "expects 1 argument",
        ),
        (
            "\"abc\".startsWith(1);",
            "SZ4002",
            "TypeError",
            "prefix must be a string",
        ),
        (
            "\"abc\".charAt(1.0);",
            "SZ4002",
            "TypeError",
            "index must be an int",
        ),
        (
            "\"abc\".padStart(3, 7);",
            "SZ4002",
            "TypeError",
            "padString must be a string",
        ),
        (
            "\"abc\".padEnd(0 - 1, \"0\");",
            "SZ4000",
            "RangeError",
            "must be non-negative",
        ),
        (
            "\"abc\".slice(0, 1, 2);",
            "SZ4002",
            "TypeError",
            "expects 0, 1 or 2 arguments",
        ),
        (
            "\"abc\".missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown string method",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured String error, got {other:?}"),
        }
    }

    let caught = r#"
        let caughtCount = 0;
        try { "x".startsWith(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { "x".padStart(0 - 1); }
        catch (e) { if (e.code == "SZ4000" && e.kind == "RangeError") { caughtCount++; } }
        try { "x".missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        if (caughtCount != 3) { throw "String errors were not catchable"; }
        if ("abc".substring(1) != "bc") { throw "String did not recover"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn string_arguments_preserve_nested_outcomes_and_arity_short_circuits() {
    let runtime_cases = [
        "\"abc\".startsWith(1 / 0);",
        "\"abc\".replace(\"a\", 1 / 0);",
        "\"abc\".substring(1 / 0);",
        "\"abc\".padEnd(3, 1 / 0);",
        "\"abc\".slice(1 / 0);",
    ];
    for src in runtime_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4004", "{src}");
                assert_eq!(error.kind, "DivisionByZero", "{src}");
            }
            other => panic!("{src}: expected original String argument error, got {other:?}"),
        }
    }

    let throw_cases = [
        r#"fn string boom() { throw "string-boom"; return ""; } "x".startsWith(boom());"#,
        r#"fn int boom() { throw "string-boom"; return 0; } "x".slice(boom());"#,
    ];
    for src in throw_cases {
        assert!(matches!(
            evaluate(src),
            ProgramOutcome::UncaughtException { message } if message == "string-boom"
        ));
    }

    let arity = r#"
        let touched = 0;
        fn int touch() { touched++; return 1; }
        try { "abc".length(touch()); }
        catch (e) {
            if (e.code != "SZ4002" || e.kind != "TypeError") { throw "wrong arity error"; }
        }
        try { "abc".padStart(3, "0", touch()); } catch (e) {}
        if (touched != 0) { throw "String evaluated arguments after arity failure"; }
    "#;
    assert!(matches!(evaluate(arity), ProgramOutcome::Value(_)));
}

#[test]
fn string_padding_is_bounded_linear_and_preserves_valid_results() {
    let src = r#"
        if ("x".padStart(4, "ab") != "babx") { throw "padStart compatibility changed"; }
        if ("x".padEnd(4, "ab") != "xaba") { throw "padEnd compatibility changed"; }
        if ("é".padStart(3, "🙂") != "🙂🙂é") { throw "Unicode padding is byte-based"; }
        if ("abc".slice((0 - 9223372036854775807) - 1) != "abc") {
            throw "minimum slice index did not clamp";
        }
        if ("abc".slice(0, (0 - 9223372036854775807) - 1) != "") {
            throw "minimum slice end did not clamp";
        }
    "#;
    assert!(matches!(evaluate(src), ProgramOutcome::Value(_)));

    let limit = r#"
        try { "x".padEnd(10000001, "x"); }
        catch (e) { throw "padding resource ceiling was catchable"; }
        throw "padding resource ceiling did not stop execution";
    "#;
    match evaluate(limit) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ6002");
            assert_eq!(error.kind, "ResourceError");
            assert!(error.message.contains("padding target length"));
        }
        other => panic!("expected fatal String padding limit, got {other:?}"),
    }
}

#[test]
fn array_method_failures_are_structured_and_catchable() {
    let cases = [
        (
            "let values = [1]; values.push();",
            "SZ4002",
            "TypeError",
            "push expects 1 argument",
        ),
        (
            "let values [int] = [1]; values.push(\"bad\");",
            "SZ4002",
            "TypeError",
            "Cannot push 'string' into [int] array",
        ),
        (
            "let values = []; values.pop();",
            "SZ4003",
            "IndexOutOfBounds",
            "empty array",
        ),
        (
            "let values = [1]; values.remove(4);",
            "SZ4003",
            "IndexOutOfBounds",
            "out of bounds",
        ),
        (
            "let values = [1, \"x\"]; values.sort();",
            "SZ4002",
            "TypeError",
            "homogeneous array",
        ),
        (
            "let values = [1]; values.sort(\"sideways\");",
            "SZ4000",
            "RangeError",
            "asc",
        ),
        (
            "[1].slice(\"bad\");",
            "SZ4002",
            "TypeError",
            "start must be an int",
        ),
        (
            "[1].flat(\"bad\");",
            "SZ4002",
            "TypeError",
            "depth must be an int",
        ),
        (
            "[1].missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown array method",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured Array error, got {other:?}"),
        }
    }

    let caught = r#"
        let caughtCount = 0;
        let values = [1];
        try { values.push(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { values.remove(9); }
        catch (e) { if (e.code == "SZ4003" && e.kind == "IndexOutOfBounds") { caughtCount++; } }
        try { values.missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        if (caughtCount != 3) { throw "Array errors were not catchable"; }
        values.push(2);
        if (values.length() != 2) { throw "Array did not recover"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn array_arguments_and_callbacks_preserve_nested_outcomes() {
    let runtime_cases = [
        "let values = [1]; values.remove(1 / 0);",
        "[1].join(1 / 0);",
        "[1].slice(1 / 0);",
        "[1].flat(1 / 0);",
        "[1].map(x => 1 / 0);",
        "[1].filter(x => 1 / 0);",
        "[1].reduce(0, (a, x) => 1 / 0);",
        "[1].find(x => 1 / 0);",
        "[1].every(x => 1 / 0);",
    ];
    for src in runtime_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4004", "{src}");
                assert_eq!(error.kind, "DivisionByZero", "{src}");
            }
            other => panic!("{src}: expected original Array runtime error, got {other:?}"),
        }
    }

    let throw_cases = [
        r#"fn int boom() { throw "array-boom"; return 0; } let values = [1]; values.remove(boom());"#,
        r#"fn string boom() { throw "array-boom"; return ""; } [1].join(boom());"#,
        r#"fn int boom() { throw "array-boom"; return 0; } [1].slice(boom());"#,
        r#"fn int boom() { throw "array-boom"; return 0; } [1].flat(boom());"#,
        r#"fn bool boom(any value) { throw "array-boom"; return false; } [1].some(boom);"#,
    ];
    for src in throw_cases {
        assert!(matches!(
            evaluate(src),
            ProgramOutcome::UncaughtException { message } if message == "array-boom"
        ));
    }
}

#[test]
fn array_validation_precedes_arguments_and_failed_sort_is_atomic() {
    let src = r#"
        let touched = 0;
        fn int touch() { touched++; return 1; }
        let values = [3, 1, 2];
        try { values.pop(touch()); } catch (e) {}
        try { values.reverse(touch()); } catch (e) {}
        try { values.sort((a, b) => a - b, touch()); } catch (e) {}
        if (touched != 0) { throw "Array evaluated arguments after arity failure"; }

        fn string invalidComparator(int a, int b) { return "bad"; }
        try { values.sort(invalidComparator); } catch (e) {
            if (e.code != "SZ4002" || e.kind != "TypeError") { throw "wrong comparator error"; }
        }
        if (values[0] != 3 || values[1] != 1 || values[2] != 2) {
            throw "failed comparator partially mutated the receiver";
        }

        let empty = [];
        let callbackValidated = false;
        try { empty.find(1); }
        catch (e) { callbackValidated = e.code == "SZ4002" && e.kind == "TypeError"; }
        if (!callbackValidated) { throw "empty Array skipped callback validation"; }
    "#;
    assert!(matches!(evaluate(src), ProgramOutcome::Value(_)));
}

#[test]
fn free_variables_in_a_function_resolve_dynamically() {
    // Serez resolves a free variable by walking the whole scope stack, and a
    // call pushes its frame onto that same stack rather than starting a fresh
    // one. A callee therefore sees the *caller's* locals: this is dynamic
    // scoping, not lexical, and it is not documented anywhere.
    //
    // This test pins the behavior rather than endorsing it. Changing it is a
    // language-level decision recorded in MATURITY_AUDIT.md and spec/scopes.md;
    // the point of pinning it is that the change cannot then happen by accident.
    let src = r#"
        fn string callee() { return secret; }
        fn string first()  { let secret = "from-first";  return callee(); }
        fn string second() { let secret = "from-second"; return callee(); }
        if (first() != "from-first")   { throw "callee did not see first's local"; }
        if (second() != "from-second") { throw "callee did not see second's local"; }
    "#;
    assert!(matches!(evaluate(src), ProgramOutcome::Value(_)));

    // With the name bound nowhere on the stack it is still an error, so this is
    // dynamic resolution and not an implicit global.
    let orphan = r#"
        fn string callee() { return secret; }
        fn string orphan() { return callee(); }
        orphan();
    "#;
    match evaluate(orphan) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4001", "{error:?}");
            assert!(error.message.contains("secret"), "{error:?}");
        }
        other => panic!("an unbound free variable must still fail, got {other:?}"),
    }

    // Closures are separate machinery and are genuinely lexical: they capture a
    // cell at creation and keep seeing it.
    let closure = r#"
        fn any counter() { let n = 0; return () => { n = n + 1; return n; }; }
        let c = counter();
        c(); c();
        if (c() != 3) { throw "closure cell did not persist"; }

        let captured = 10;
        let read = () => { return captured; };
        captured = 20;
        if (read() != 20) { throw "closure must see the later write"; }
    "#;
    assert!(matches!(evaluate(closure), ProgramOutcome::Value(_)));
}

#[test]
fn a_mutator_on_a_nested_receiver_writes_back() {
    // Receiver writeback covered `obj.field.push(x)` and `dict["k"].push(x)`,
    // but not an array index and not a chain. `a[0].push(x)` and
    // `this.c[l][h]["k"].push(x)` mutated a copy and dropped it — no error, no
    // effect. serez-agentai's KVCache.store() is exactly the second shape, so
    // its cache never accumulated and seqLen() always answered 0.
    let cases = [
        // (source, expression that must be true afterwards)
        ("let a = [[1]]; a[0].push(2);", "a[0].length() == 2"),
        ("let a = [[1]]; a[0].unshift(0);", "a[0].length() == 2"),
        ("let a = [[2, 1]]; a[0].sort();", "a[0][0] == 1"),
        ("let a = [[1, 2]]; a[0].reverse();", "a[0][0] == 2"),
        ("let a = [[1, 2]]; a[0].pop();", "a[0].length() == 1"),
        (
            r#"let d <string, any> = ({"k", [[1]]}); d["k"][0].push(2);"#,
            r#"d["k"][0].length() == 2"#,
        ),
        (
            r#"class H { public H() { this.f = [[1]]; } } let h = new H(); h.f[0].push(2);"#,
            "h.f[0].length() == 2",
        ),
    ];

    for (setup, check) in cases {
        let src = format!("{setup} if (!({check})) {{ throw \"no writeback\"; }}");
        match evaluate(&src) {
            ProgramOutcome::Value(_) => {}
            other => panic!("{setup}: mutation was dropped ({check}): {other:?}"),
        }
    }

    // The shape serez-agentai/src/kvcache.sz uses, end to end.
    let kvcache = r#"
        class KVCache {
            public KVCache() {
                this.cache = [];
                let entry <string, any> = ({"k", []}, {"v", []});
                let layer = [entry];
                this.cache.push(layer);
            }
            public void store(int layer, int head, int k) {
                this.cache[layer][head]["k"].push(k);
            }
            public int seqLen(int layer, int head) {
                return this.cache[layer][head]["k"].length();
            }
        }
        let kv = new KVCache();
        kv.store(0, 0, 11);
        kv.store(0, 0, 22);
        if (kv.seqLen(0, 0) != 2) { throw "KV cache did not accumulate"; }
    "#;
    assert!(matches!(evaluate(kvcache), ProgramOutcome::Value(_)));

    // Reading a nested value into a binding still copies: writeback is about
    // calling a mutator *on a place*, not about making containers shared.
    let still_copies = r#"
        let a = [[1]];
        let taken = a[0];
        taken.push(2);
        if (a[0].length() != 1) { throw "a read must still copy"; }
        if (taken.length() != 2) { throw "the copy must still be mutable"; }
    "#;
    assert!(matches!(evaluate(still_copies), ProgramOutcome::Value(_)));
}

#[test]
fn every_type_reports_an_unknown_member_the_same_way() {
    // `kind` is only useful for classifying a failure if it means the same
    // thing everywhere. Twelve native namespaces reported an unknown member as
    // `TypeError`, so a caller could not tell "there is no such member" from
    // "you called it wrongly" — the same inconsistency Set had.
    let cases = [
        ("[1].nope();", "array"),
        (r#""a".nope();"#, "string"),
        (r#"let d <string, int> = ({"a", 1}); d.nope();"#, "dict"),
        ("let s = new Set([1]); s.nope();", "set"),
        ("let n = 1.5m; n.nope();", "dec"),
        ("Dec.nope();", "Dec"),
        ("Math.nope();", "Math"),
        ("JSON.nope();", "JSON"),
        ("Memory.nope();", "Memory"),
        ("Binary.nope();", "Binary"),
        ("GPU.nope();", "GPU"),
        (r#"Regex.nope("a", "b");"#, "Regex"),
        ("Random.nope();", "Random"),
        ("enum E { A } E.nope();", "enum"),
    ];

    for (src, label) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4001", "{label}: {src}");
                assert_eq!(error.kind, "ReferenceError", "{label}: {src}");
            }
            other => panic!("{label}: expected SZ4001 ReferenceError, got {other:?}"),
        }
    }

    // The permission-gated namespaces reach the same answer once the
    // permission is declared; without it the gate legitimately answers first.
    for (src, permission) in [
        ("File.nope();", "File"),
        ("Socket.nope();", "Socket"),
        ("Time.nope();", "Time"),
        ("Env.nope();", "Env"),
        ("OS.nope();", "OS"),
        ("Terminal.nope();", "Terminal"),
        ("System.nope();", "System"),
    ] {
        match evaluate_with_permissions(src, &[permission]) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4001", "{src}");
                assert_eq!(error.kind, "ReferenceError", "{src}");
            }
            other => panic!("{src}: expected SZ4001 ReferenceError, got {other:?}"),
        }
    }
}

#[test]
fn exact_decimal_method_failures_are_structured_and_catchable() {
    let cases = [
        (
            "let d = 1.5m; d.round();",
            "SZ4002",
            "TypeError",
            "takes 1 or 2 arguments",
        ),
        (
            "let d = 1.5m; d.round(99);",
            "SZ4000",
            "RangeError",
            "scale must be 0..=28",
        ),
        (
            r#"let d = 1.5m; d.round(2, "nope");"#,
            "SZ4000",
            "RangeError",
            "unknown rounding mode 'nope'",
        ),
        (
            r#"let d = 1.5m; d.round("x");"#,
            "SZ4002",
            "TypeError",
            "scale must be an int",
        ),
        (
            "let d = 1.5m; d.nope();",
            "SZ4001",
            "ReferenceError",
            "Unknown dec method",
        ),
        (
            "let d = 1.5m; d.min();",
            "SZ4002",
            "TypeError",
            "requires 1 argument",
        ),
        (
            r#"let d = 1.5m; d.min("x");"#,
            "SZ4002",
            "TypeError",
            "must be a dec or an int",
        ),
        ("Dec.parse();", "SZ4002", "TypeError", "requires 1 argument"),
        (
            r#"Dec.parse("zzz");"#,
            "SZ4000",
            "RangeError",
            "invalid decimal",
        ),
        (
            "Dec.fromInt(1);",
            "SZ4002",
            "TypeError",
            "requires 2 integers",
        ),
        (
            "Dec.fromInt(1, 99);",
            "SZ4000",
            "RangeError",
            "scale must be 0..=28",
        ),
        (
            "Dec.nope();",
            "SZ4001",
            "ReferenceError",
            "Unknown Dec method",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected a structured dec diagnostic, got {other:?}"),
        }
    }

    // Nested outcomes survive argument evaluation.
    match evaluate("let d = 1.5m; d.round(1 / 0);") {
        ProgramOutcome::RuntimeError(error) => assert_eq!(error.code, "SZ4004"),
        other => panic!("expected the nested runtime error, got {other:?}"),
    }
    assert!(matches!(
        evaluate(r#"fn int boom() { throw "dec-boom"; return 0; } let d = 1.5m; d.round(boom());"#),
        ProgramOutcome::UncaughtException { message } if message == "dec-boom"
    ));

    // Valid results are unchanged.
    let valid = r#"
        let d = 1.555m;
        if (d.round(2).toString() != "1.56") { throw "round changed"; }
        if (Dec.fromInt(155, 2).toString() != "1.55") { throw "fromInt changed"; }
        if (Dec.parse("3.14").toString() != "3.14") { throw "parse changed"; }
        if ((2.5m).min(1.5m).toString() != "1.5") { throw "min changed"; }
        let caught = false;
        try { d.nope(); } catch (e) { caught = e.code == "SZ4001"; }
        if (!caught) { throw "dec errors were not catchable"; }
    "#;
    assert!(matches!(evaluate(valid), ProgramOutcome::Value(_)));
}

#[test]
fn callback_and_patch_dispatch_diagnostics_are_structured() {
    let cases = [
        (
            "[1].map(fn(a, b, c) { return a; });",
            "SZ4002",
            "TypeError",
            "Callback expected 3 argument(s), got 2",
        ),
        (
            r#"interface P { edad: int; } let p = new P({edad: 1}); p = {edad: "x"};"#,
            "SZ4002",
            "TypeError",
            "Field 'edad' expects 'int' but got 'string'",
        ),
        (
            "let x = 5; x = {edad: 1};",
            "SZ4002",
            "TypeError",
            "is not an interface instance",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected a structured diagnostic, got {other:?}"),
        }
    }

    // Catchable, and the interface instance is left as it was.
    let caught = r#"
        interface P { edad: int; }
        let p = new P({edad: 1});
        let caught = false;
        try { p = {edad: "x"}; } catch (e) { caught = e.code == "SZ4002"; }
        if (!caught) { throw "patch type failure was not catchable"; }
        if (p.edad != 1) { throw "a rejected patch must not change the instance"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn a_value_too_deep_to_copy_stops_the_program_instead_of_truncating_it() {
    // `extract` bounds its own recursion at MAX_VALUE_DEPTH. Past that it
    // replaced the subtree with null, printed one line per truncated site and
    // let the program run to completion: corrupted data, flooded stderr, exit 0.
    let src = r#"
        fn any nest(int n) {
            let v = [1];
            for (let i = 0; i < n; i = i + 1) { v = [v]; }
            return v;
        }
        let deep = nest(600);
        out deep;
    "#;
    match evaluate(src) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ6002", "{error:?}");
            assert_eq!(error.kind, "ResourceError", "{error:?}");
            assert!(error.message.contains("500"), "{error:?}");
        }
        other => panic!("a value too deep to copy must fail, got {other:?}"),
    }

    // Fatal, not catchable: the value has already lost a subtree, so a handler
    // that carried on would be working with corrupted data.
    let attempt_to_catch = r#"
        fn any nest(int n) {
            let v = [1];
            for (let i = 0; i < n; i = i + 1) { v = [v]; }
            return v;
        }
        try { let deep = nest(600); } catch (e) { out "caught"; }
        out "kept going";
    "#;
    assert!(
        matches!(evaluate(attempt_to_catch), ProgramOutcome::RuntimeError(_)),
        "a resource ceiling must cross try/catch"
    );

    // Ordinary nesting is untouched.
    match evaluate("let a = [[[[1]]]]; a;") {
        ProgramOutcome::Value(_) => {}
        other => panic!("shallow nesting must still work, got {other:?}"),
    }
}

#[test]
fn statement_level_diagnostics_are_structured() {
    let cases = [
        (
            "fn int f() { yield 1; return 0; } f();",
            "SZ4002",
            "TypeError",
            "'yield' used outside of a generator",
        ),
        (
            // The `unsafe` gate fires first on a bare pointer write, so the
            // type check below it is only reachable inside the block.
            "let x = 5; unsafe { *x = 1; }",
            "SZ4002",
            "TypeError",
            "Left side of '*ptr = val' is not a pointer",
        ),
        (
            "for (let item in nosuchcollection) { out item; }",
            "SZ4001",
            "ReferenceError",
            "Variable not found: nosuchcollection",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected a structured diagnostic, got {other:?}"),
        }
    }
}

#[test]
fn a_module_that_cannot_be_loaded_is_told_apart_from_one_that_is_missing() {
    use std::io::Write;

    let dir = std::env::temp_dir().join(format!("sz_import_slice_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);

    // A module that exists but does not parse is SZ5002 — a different failure
    // from "there is no such module", which stays a catchable user exception
    // carrying "ModuleNotFound" (pinned by tests/unit_sec_import.sz).
    let broken = dir.join("broken_module.sz");
    let mut file = std::fs::File::create(&broken).expect("fixture must be writable");
    writeln!(file, "let = ;").expect("fixture must be writable");
    drop(file);

    let import_path = broken.display().to_string().replace('\\', "/");
    let src = format!("import \"{import_path}\";");
    match evaluate(&src) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ5002", "{error:?}");
            assert_eq!(error.kind, "ImportError", "{error:?}");
            assert!(error.message.contains("parse errors"), "{error:?}");
        }
        other => panic!("expected SZ5002 for an unparsable module, got {other:?}"),
    }

    // Missing modules keep their historical shape.
    match evaluate("import \"absolutely_nonexistent_xyz_module\";") {
        ProgramOutcome::UncaughtException { message } => {
            assert!(message.contains("ModuleNotFound"), "{message}");
        }
        other => panic!("a missing module must stay a user exception, got {other:?}"),
    }

    let _ = std::fs::remove_file(&broken);
    let _ = std::fs::remove_dir(&dir);
}

#[test]
fn a_missing_import_is_reported_once_not_twice() {
    // The module paths printed the failure themselves and *also* threw it, so a
    // single missing import produced two lines on stderr: an "❌ ERROR:" from
    // the import and an "❌ UNCAUGHT EXCEPTION:" from the program boundary.
    let detailed = run_source_detailed(
        "import \"absolutely_nonexistent_xyz_module\";".to_string(),
        "<runtime-outcome>",
        RunOpts::default(),
    );
    assert_eq!(detailed.exit_code, 1);
    assert!(
        matches!(detailed.failure, Some(RunFailure::UncaughtException { .. })),
        "a missing module stays a user exception: {:?}",
        detailed.failure
    );
}

#[test]
fn core_expression_diagnostics_are_structured_and_catchable() {
    // These are the diagnostics an ordinary program hits: a wrong argument, a
    // wrong return, a literal that violates its own declared type. Every one of
    // them used to print to stderr and return an untyped sentinel, so none was
    // catchable and none could be classified without reading English.
    let cases = [
        (
            r#"fn int f(int n) { return n; } f("x");"#,
            "SZ4002",
            "TypeError",
            "Parameter 'n' expected 'int' but received 'string'",
        ),
        (
            r#"fn int f() { return "s"; } f();"#,
            "SZ4002",
            "TypeError",
            "expected to return 'int' but returned 'string'",
        ),
        (
            r#"let a [int] = [1, "x"];"#,
            "SZ4002",
            "TypeError",
            "Array declared as [int] but element has type 'string'",
        ),
        (
            r#"let d <string, int> = ({1, 2});"#,
            "SZ4002",
            "TypeError",
            "Dict key does not match declared key type 'string'",
        ),
        (
            r#"let d <string, int> = ({"a", "b"});"#,
            "SZ4002",
            "TypeError",
            "Dict value does not match declared value type 'int'",
        ),
        (
            "enum E { A } E.B();",
            "SZ4001",
            "ReferenceError",
            "'B' is not a variant of enum 'E'",
        ),
        (
            "enum E { A } let v = E.A; v.nope();",
            "SZ4001",
            "ReferenceError",
            "Enum variant has no method 'nope'",
        ),
        (
            "true.nope();",
            "SZ4002",
            "TypeError",
            "'.' method call not supported for type 'bool'",
        ),
        (
            "let p = &nosuchvar;",
            "SZ4001",
            "ReferenceError",
            "Cannot take address of undeclared variable 'nosuchvar'",
        ),
        (
            "let p = &(1 + 2);",
            "SZ4002",
            "TypeError",
            "'&' can only be applied to a named variable",
        ),
        (
            "let x = 5; let d = *x;",
            "SZ4002",
            "TypeError",
            "Cannot dereference a non-pointer value",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected a structured diagnostic, got {other:?}"),
        }
    }

    // Catchable, and the program keeps running afterwards.
    let caught = r#"
        fn int doble(int n) { return n * 2; }
        let caughtCount = 0;
        try { doble("x"); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { let bad [int] = [1, "x"]; }
        catch (e) { if (e.code == "SZ4002") { caughtCount++; } }
        try { let p = &nosuchvar; }
        catch (e) { if (e.code == "SZ4001") { caughtCount++; } }
        if (caughtCount != 3) { throw "core diagnostics were not catchable"; }
        if (doble(21) != 42) { throw "the evaluator did not recover"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn a_typed_parameter_failure_still_reports_the_call_stack() {
    // The stack used to be printed by a side-effecting `print_call_stack()`
    // call next to the eprintln. It now travels in the structured payload, so
    // an embedder gets the frames instead of having to scrape stderr.
    let src = r#"
        fn int doble(int n) { return n * 2; }
        fn int outer(string s) { return doble(s); }
        outer("hola");
    "#;
    match evaluate(src) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4002");
            assert!(
                error.stack.iter().any(|frame| frame.name == "outer"),
                "the failing call's caller must appear in the stack: {error:?}"
            );
        }
        other => panic!("expected a structured diagnostic, got {other:?}"),
    }
}

#[test]
fn dict_and_set_failures_are_structured_and_catchable() {
    let cases = [
        (
            r#"let d <string, int> = ({"a", 1}); d.Add();"#,
            "SZ4002",
            "TypeError",
            "Add expects",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.Add(1);"#,
            "SZ4002",
            "TypeError",
            "entry literal",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.Add({1, 2});"#,
            "SZ4002",
            "TypeError",
            "key type",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.Add({"b", "x"});"#,
            "SZ4002",
            "TypeError",
            "value type",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.Remove();"#,
            "SZ4002",
            "TypeError",
            "Remove expects",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.clear(1);"#,
            "SZ4002",
            "TypeError",
            "0 arguments",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.keys(1);"#,
            "SZ4002",
            "TypeError",
            "0 arguments",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.values(1);"#,
            "SZ4002",
            "TypeError",
            "0 arguments",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.toArray(1);"#,
            "SZ4002",
            "TypeError",
            "0 arguments",
        ),
        (
            r#"let d <string, int> = ({"a", 1}); d.missing();"#,
            "SZ4001",
            "ReferenceError",
            "Unknown dict method",
        ),
        ("new Set(5);", "SZ4002", "TypeError", "array"),
        (
            "let s = new Set([1]); s.size(1);",
            "SZ4002",
            "TypeError",
            "0 arguments",
        ),
        (
            "let s = new Set([1]); s.toArray(1);",
            "SZ4002",
            "TypeError",
            "0 arguments",
        ),
        (
            "let s = new Set([1]); s.clear(1);",
            "SZ4002",
            "TypeError",
            "0 arguments",
        ),
        (
            "let s = new Set([1]); s.missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown Set method",
        ),
        (
            "let s = new Set([1]); s.union(5);",
            "SZ4002",
            "TypeError",
            "Set argument",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected a structured collection error, got {other:?}"),
        }
    }

    // Catchable, and the receiver still works afterwards.
    let caught = r#"
        let caughtCount = 0;
        let d <string, int> = ({"a", 1});
        try { d.Add(); }
        catch (e) { if (e.code == "SZ4002" && e.kind == "TypeError") { caughtCount++; } }
        try { d.missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        let s = new Set([1]);
        try { s.missing(); }
        catch (e) { if (e.code == "SZ4001" && e.kind == "ReferenceError") { caughtCount++; } }
        if (caughtCount != 3) { throw "collection errors were not catchable"; }
        d.Add({"b", 2});
        if (d.keys().length() != 2) { throw "dict did not recover"; }
        s.add(2);
        if (s.size() != 2) { throw "set did not recover"; }
    "#;
    assert!(matches!(evaluate(caught), ProgramOutcome::Value(_)));
}

#[test]
fn dict_and_set_preserve_nested_outcomes_and_validate_arity_first() {
    let runtime_cases = [
        r#"let d <string, int> = ({"a", 1}); d.Add({"k", 1 / 0});"#,
        r#"let d <string, int> = ({"a", 1}); d.Remove(1 / 0);"#,
        "let s = new Set([1]); s.add(1 / 0);",
        "let s = new Set([1]); s.has(1 / 0);",
        "let s = new Set([1]); s.delete(1 / 0);",
        "let s = new Set([1]); s.union(1 / 0);",
        "let s = new Set([1]); s.intersection(1 / 0);",
        "new Set(1 / 0);",
    ];
    for src in runtime_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4004", "{src}");
                assert_eq!(error.kind, "DivisionByZero", "{src}");
            }
            other => panic!("{src}: expected the nested runtime error, got {other:?}"),
        }
    }

    let throw_cases = [
        r#"fn string boom() { throw "coll-boom"; return ""; } let d <string, int> = ({"a", 1}); d.Remove(boom());"#,
        r#"fn int boom() { throw "coll-boom"; return 0; } let d <string, int> = ({"a", 1}); d.Add({"k", boom()});"#,
        r#"fn int boom() { throw "coll-boom"; return 0; } let s = new Set([1]); s.add(boom());"#,
        r#"fn int boom() { throw "coll-boom"; return 0; } let s = new Set([1]); s.has(boom());"#,
        r#"fn [int] boom() { throw "coll-boom"; return [1]; } new Set(boom());"#,
    ];
    for src in throw_cases {
        assert!(
            matches!(
                evaluate(src),
                ProgramOutcome::UncaughtException { message } if message == "coll-boom"
            ),
            "{src}: the user exception must survive"
        );
    }

    // Arity is rejected before the arguments run.
    let ordering = r#"
        let touched = 0;
        fn int touch() { touched++; return 1; }
        let d <string, int> = ({"a", 1});
        try { d.clear(touch()); } catch (e) {}
        try { d.keys(touch()); } catch (e) {}
        try { d.toArray(touch()); } catch (e) {}
        let s = new Set([1]);
        try { s.size(touch()); } catch (e) {}
        try { s.clear(touch()); } catch (e) {}
        if (touched != 0) { throw "collections evaluated arguments after an arity failure"; }
    "#;
    assert!(matches!(evaluate(ordering), ProgramOutcome::Value(_)));
}

#[test]
fn shared_argument_helpers_preserve_nested_outcomes() {
    // `eval_str_arg` / `eval_int_arg` used to collapse every outcome that was
    // not a value into a bare `EvalResult::Error`, so a user `throw` raised
    // while evaluating an argument vanished and the caller reported its own
    // generic message instead. Array reaches these helpers through
    // remove/join/slice/flat; Crypto and Regex are the other two consumers.
    let throw_cases = [
        r#"fn int boom() { throw "arg-boom"; return 0; } Crypto.randomBytes(boom());"#,
        r#"fn string boom() { throw "arg-boom"; return ""; } Regex.test(boom(), "x");"#,
        r#"fn string boom() { throw "arg-boom"; return ""; } Regex.test("x", boom());"#,
        r#"fn string boom() { throw "arg-boom"; return ""; } Regex.replace("x", "x", boom());"#,
    ];
    for src in throw_cases {
        assert!(
            matches!(
                evaluate(src),
                ProgramOutcome::UncaughtException { message } if message == "arg-boom"
            ),
            "{src}: the user exception must survive argument evaluation"
        );
    }

    let runtime_cases = ["Crypto.randomBytes(1 / 0);", "Regex.test(1 / 0, \"x\");"];
    for src in runtime_cases {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4004", "{src}");
                assert_eq!(error.kind, "DivisionByZero", "{src}");
            }
            other => panic!("{src}: expected the nested runtime error, got {other:?}"),
        }
    }

    // A well-formed value of the wrong type still names the call that rejected
    // it, instead of the old context-free "Expected int argument".
    match evaluate("Crypto.randomBytes(\"nope\");") {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4002");
            assert_eq!(error.kind, "TypeError");
            assert!(
                error.message.contains("Crypto.randomBytes"),
                "message must name the call: {error:?}"
            );
        }
        other => panic!("expected a structured TypeError, got {other:?}"),
    }
}

#[test]
fn task_api_failures_are_structured_and_preserve_argument_outcomes() {
    let cases = [
        ("Task.run();", "SZ4002", "TypeError", "requires 2"),
        (
            "Task.run(1, \"\");",
            "SZ4002",
            "TypeError",
            "script_path must be a string",
        ),
        ("Task.message(1);", "SZ4002", "TypeError", "requires 0"),
        ("Task.reply();", "SZ4002", "TypeError", "requires 1"),
        (
            "Task.reply(1);",
            "SZ4002",
            "TypeError",
            "result must be a string",
        ),
        ("Task.poll();", "SZ4002", "TypeError", "requires 1"),
        (
            "Task.poll(\"bad\");",
            "SZ4002",
            "TypeError",
            "taskId must be an integer",
        ),
        (
            "Task.poll(-9223372036854775807);",
            "SZ4001",
            "ReferenceError",
            "not found",
        ),
        ("Task.isDone();", "SZ4002", "TypeError", "requires 1"),
        (
            "Task.isDone(\"bad\");",
            "SZ4002",
            "TypeError",
            "taskId must be an integer",
        ),
        (
            "Task.isDone(-9223372036854775807);",
            "SZ4001",
            "ReferenceError",
            "not found",
        ),
        (
            "Task.missing();",
            "SZ4001",
            "ReferenceError",
            "Unknown Task method",
        ),
    ];

    for (src, expected_code, expected_kind, expected_message) in cases {
        match evaluate_with_permissions(src, &["Task"]) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, expected_code, "{src}");
                assert_eq!(error.kind, expected_kind, "{src}");
                assert!(error.message.contains(expected_message), "{src}: {error:?}");
            }
            other => panic!("{src}: expected structured Task error, got {other:?}"),
        }
    }

    let user_throw = r#"
        fn string taskBoom() { throw "task-boom"; return ""; }
        Task.run(taskBoom(), "");
    "#;
    assert!(matches!(
        evaluate_with_permissions(user_throw, &["Task"]),
        ProgramOutcome::UncaughtException { message } if message == "task-boom"
    ));

    match evaluate_with_permissions("Task.poll(1 / 0);", &["Task"]) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4004");
            assert_eq!(error.kind, "DivisionByZero");
        }
        other => panic!("expected original nested Task runtime error, got {other:?}"),
    }
}

#[test]
fn task_workers_inherit_lockdown_instead_of_reopening_host_capabilities() {
    let src = r#"
        let id = Task.run("tests/task_worker_lockdown_escape.sz", "");
        while (!Task.isDone(id)) {}
        let result = Task.poll(id);
        if (result == "escaped") { throw "worker escaped lockdown"; }
        if (!result.includes("PermissionError")) { throw "worker lost the denial diagnostic"; }
    "#;

    assert!(matches!(
        evaluate_with_permissions_and_lockdown(src, &["Task"]),
        ProgramOutcome::Value(_)
    ));
}

#[test]
fn task_reply_is_not_published_before_worker_success_is_known() {
    let src = r#"
        use permissions { Task, Time }
        let id = Task.run("tests/task_worker_reply_then_fail.sz", "");
        while (!Task.isDone(id)) { Time.sleep(1); }
        let result = Task.poll(id);
        if (!result.includes("SZ4004")) { throw "reply hid the later worker failure"; }
    "#;

    assert!(matches!(evaluate(src), ProgramOutcome::Value(_)));
}

#[test]
fn detailed_pipeline_exposes_runtime_failure_without_changing_exit_code() {
    let detailed = run_source_detailed(
        "out(1 / 0);".to_string(),
        "<runtime-outcome>",
        RunOpts::default(),
    );

    assert_eq!(detailed.exit_code, 1);
    assert!(!detailed.is_success());
    match detailed.failure {
        Some(RunFailure::Runtime(error)) => assert_eq!(error.code, "SZ4004"),
        other => panic!("expected structured pipeline runtime failure, got {other:?}"),
    }

    // The existing entry point remains the compact, source-compatible API.
    assert_eq!(
        run_source(
            "out(1 / 0);".to_string(),
            "<runtime-outcome>",
            RunOpts::default(),
        )
        .exit_code,
        1
    );
}

#[test]
fn detailed_pipeline_exposes_frontend_diagnostics() {
    let detailed = run_source_detailed(
        "let = ;".to_string(),
        "<runtime-outcome>",
        RunOpts::default(),
    );

    assert_eq!(detailed.exit_code, 1);
    match detailed.failure {
        Some(RunFailure::Frontend(errors)) => {
            assert!(!errors.is_empty());
            assert!(errors.iter().all(|error| error.code.starts_with("SZ2")));
        }
        other => panic!("expected frontend diagnostics, got {other:?}"),
    }
}

/// `ProgramOutcome::Value` carries an arena ref, not a comparable value, so a
/// fixture states its expectation in Serez: it throws when the expectation
/// fails, and success is simply "the program finished".
fn expect_ok(src: &str, what: &str) {
    match evaluate(src) {
        ProgramOutcome::Value(_) => {}
        other => panic!("{what}: expected the program to finish, got {other:?}"),
    }
}

#[test]
fn a_constructor_enforces_its_declared_parameter_types() {
    // Arity was checked on the constructor path; the declared type was not. So
    // `new Point("x")` bound a string into an `int` field and the program only
    // failed later, wherever that field was used as a number — or never, if it
    // was just read back out.
    match evaluate(
        r#"
        class Point { public Point(int x) { this.x = x; } }
        new Point("not an int");
        "#,
    ) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4002", "{error:?}");
            assert_eq!(error.kind, "TypeError", "{error:?}");
            assert!(
                error.message.contains("constructor 'Point'")
                    && error.message.contains("expected 'int'")
                    && error.message.contains("received 'string'"),
                "{error:?}"
            );
        }
        other => panic!("a mistyped constructor argument must fail, got {other:?}"),
    }

    // The failure is catchable, like every other parameter mismatch.
    expect_ok(
        r#"
        class Point { public Point(int x) { this.x = x; } }
        let caught = "";
        try { new Point("nope"); } catch (e) { caught = e.kind; }
        if (caught != "TypeError") { throw "expected a caught TypeError, got " + caught; }
        "#,
        "constructor mismatch is catchable",
    );

    // A correctly typed argument still constructs, and an untyped parameter
    // still accepts anything.
    expect_ok(
        r#"
        class Point { public Point(int x, any tag) { this.x = x; this.tag = tag; } }
        let p = new Point(1, "free");
        if (p.x != 1 || p.tag != "free") { throw "well-typed construction broke"; }
        "#,
        "well-typed construction",
    );
}

#[test]
fn an_enum_value_satisfies_a_parameter_declared_with_its_enum() {
    // `type_matches` had no arm for an enum variant, so a `Priority` parameter
    // rejected `Priority.Low` and reported it in the one way nobody can act on:
    // "expected 'Priority' but received 'Priority'". `x is Priority` was false
    // for the same reason. Enum-typed parameters were simply unusable.
    expect_ok(
        r#"
        enum Priority { Low, High }
        fn string rank(Priority p) { if (p == Priority.Low) { return "low"; } return "high"; }
        if (rank(Priority.High) != "high") { throw "an enum must satisfy its own enum type"; }
        if (!(Priority.Low is Priority)) { throw "`is` must see an enum's own type"; }
        "#,
        "enum satisfies its own type",
    );

    // It stays a type: a different enum and a plain value are still rejected.
    for (src, what) in [
        (
            r#"
            enum Priority { Low }
            enum Other { A }
            fn string rank(Priority p) { return "ok"; }
            rank(Other.A);
            "#,
            "a different enum",
        ),
        (
            r#"
            enum Priority { Low }
            fn string rank(Priority p) { return "ok"; }
            rank(1);
            "#,
            "an int",
        ),
    ] {
        match evaluate(src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4002", "{what}: {error:?}");
                assert_eq!(error.kind, "TypeError", "{what}: {error:?}");
            }
            other => panic!("{what} must still be rejected, got {other:?}"),
        }
    }
}

#[test]
fn a_declared_type_matches_exactly_and_never_a_subclass() {
    // Recorded, not endorsed. Inheritance drives method dispatch, but a declared
    // class name is an exact match: a `Base` parameter rejects a `Derived`, and
    // `d is Base` is false. Writing a function over a class hierarchy means
    // typing the parameter `any`.
    //
    // Pinned so the behavior cannot change in either direction by accident; see
    // spec/types.md, which records it as an open inconsistency.
    let hierarchy = r#"
        class Base { public Base() { this.v = 1; } }
        class Derived : Base { public Derived() { this.v = 2; } }
    "#;

    expect_ok(
        &format!(
            "{hierarchy}
             if (new Derived() is Base) {{ throw \"`is` started honouring inheritance\"; }}"
        ),
        "`is` ignores inheritance",
    );

    match evaluate(&format!(
        "{hierarchy} fn any take(Base b) {{ return 1; }} take(new Derived());"
    )) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4002", "{error:?}");
            assert!(error.message.contains("expected 'Base'"), "{error:?}");
        }
        other => panic!("a Base parameter must reject a Derived today, got {other:?}"),
    }

    // `any` is the escape hatch that makes a hierarchy usable across a call.
    expect_ok(
        &format!(
            "{hierarchy} fn any take(any b) {{ return b.v; }}
             if (take(new Derived()) != 2) {{ throw \"`any` must accept a subclass\"; }}"
        ),
        "any accepts a subclass",
    );
}

#[test]
fn a_type_name_that_names_nothing_rejects_every_value() {
    // A misspelled type in an annotation parses. It then matches nothing, so the
    // function can never be called successfully — and nothing says so until a
    // call happens. Pinned because the alternative (rejecting unknown names at
    // parse time) is a change worth making deliberately, not by drift.
    match evaluate(
        r#"
        fn any f(Frobnicate x) { return "reached"; }
        f(1);
        "#,
    ) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ4002", "{error:?}");
            assert!(error.message.contains("expected 'Frobnicate'"), "{error:?}");
        }
        other => panic!("an unknown type name must reject its argument, got {other:?}"),
    }
}

#[test]
fn lockdown_denials_split_into_catchable_and_fatal() {
    // spec/security.md used to say every lockdown refusal surfaced as a
    // catchable `PermissionError` / `SZ6001`. Three of the four do. The fourth
    // does not: `use permissions` is the gate that would *grant* capability, so
    // it is a `SecurityError` / `SZ6004` that `try/catch` cannot consume — which
    // is what errors.md said all along. A security document that under-states a
    // gate's strength is worse than one that says nothing, so the split is
    // pinned here.

    // Catchable: refusing an action. Catching records the denial and the
    // program continues without the capability.
    let catchable = [
        ("File", r#"File.read("nope.txt");"#, "File"),
        ("import", r#"import "nope";"#, "import"),
        (
            "Autodiff weights",
            r#"Autodiff.saveWeights("w.szw");"#,
            "Autodiff",
        ),
    ];
    for (what, call, _hint) in catchable {
        let src = format!(
            r#"
                let kind = "";
                let code = "";
                try {{ {call} }} catch (e) {{ kind = e.kind; code = e.code; }}
                if (kind != "PermissionError") {{ throw "{what}: kind was " + kind; }}
                if (code != "SZ6001") {{ throw "{what}: code was " + code; }}
            "#
        );
        match evaluate_with_permissions_and_lockdown(&src, &[]) {
            ProgramOutcome::Value(_) => {}
            other => panic!("{what} must be a catchable SZ6001 under lockdown, got {other:?}"),
        }
    }

    // Fatal: refusing to *grant*. `try/catch` cannot turn it into control flow.
    let src = r#"
        try { use permissions { Time } } catch (e) { out("unreachable: the gate was caught"); }
        out("unreachable: execution continued");
    "#;
    match evaluate_with_permissions_and_lockdown(src, &[]) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ6004", "{error:?}");
            assert_eq!(error.kind, "SecurityError", "{error:?}");
        }
        other => panic!("`use permissions` under lockdown must be fatal, got {other:?}"),
    }

    // And lockdown starts with an empty permission set, so a guarded namespace
    // is still refused the ordinary fatal way — lockdown does not grant.
    match evaluate_with_permissions_and_lockdown(r#"Terminal.getSize();"#, &[]) {
        ProgramOutcome::RuntimeError(error) => {
            assert_eq!(error.code, "SZ6001", "{error:?}");
            assert_eq!(error.kind, "PermissionError", "{error:?}");
        }
        other => panic!("a guarded namespace must stay fatally denied, got {other:?}"),
    }
}

#[test]
fn the_string_and_crypto_ceilings_are_the_ones_the_document_names() {
    // Memory, GPU, Tensor and call-depth ceilings are covered above. These four
    // were only pinned by `err_*`/`sec_*` fixtures, which assert "non-zero exit
    // and a ❌ line" — enough to catch a crash, not enough to catch a limit
    // changing its code, its kind, or whether a program may catch it.

    // Fatal: try/catch cannot turn a resource ceiling into control flow.
    let fatal = [
        ("string repetition", r#""x" * 10000001;"#),
        ("string padding", r#""x".padStart(10000001, "y");"#),
    ];
    for (what, call) in fatal {
        let src = format!(
            r#"
                try {{ {call} }} catch (e) {{ out("unreachable: the limit was caught"); }}
                out("unreachable: execution continued");
            "#
        );
        match evaluate(&src) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ6002", "{what}: {error:?}");
                assert_eq!(error.kind, "ResourceError", "{what}: {error:?}");
            }
            other => panic!("{what} must be a fatal SZ6002, got {other:?}"),
        }
    }

    // Crypto.randomBytes bounds a request at 1 MiB and reports it as a plain
    // catchable string — the one ceiling in limits.md with no kind and no code.
    // Pinned as the gap it is: making it structured should be a deliberate
    // change that updates this test, limits.md and errors.md together.
    match evaluate(
        r#"
        let shape = "";
        try { Crypto.randomBytes(1048577); } catch (e) { shape = type_of(e); }
        if (shape != "string") { throw "randomBytes reported as " + shape; }
        "#,
    ) {
        ProgramOutcome::Value(_) => {}
        other => panic!("over the cap must throw a catchable string, got {other:?}"),
    }

    // And the cap really is 1 MiB, not merely "large".
    match evaluate(r#"Crypto.randomBytes(1048576).length();"#) {
        ProgramOutcome::Value(_) => {}
        other => panic!("exactly 1 MiB must be accepted, got {other:?}"),
    }
}

#[test]
fn implicit_constructor_chaining_reaches_exactly_one_level() {
    // spec/classes.md said each constructor in a multi-level chain "must itself
    // call super(...), or rely on the compatibility rule above when invoked
    // through ordinary construction". The second half is false past the first
    // level: the implicit call happens only at the outermost `new`. A
    // constructor reached *as a parent* gets no implicit call of its own, so a
    // grandparent's field initialization silently does not happen and surfaces
    // wherever that field is first read.
    let hierarchy = r#"
        class G { public G() { this.a = "G"; } }
    "#;

    // Two levels: the implicit call runs the parent.
    match evaluate(&format!(
        "{hierarchy}
         class Mid : G {{ public Mid() {{ this.b = \"b\"; }} }}
         if (new Mid().a != \"G\") {{ throw \"two levels must chain\"; }}"
    )) {
        ProgramOutcome::Value(_) => {}
        other => panic!("two-level implicit chaining must work, got {other:?}"),
    }

    // Three levels without an explicit super() in the middle: the grandparent
    // never runs, so its field is missing.
    for (what, mid) in [
        (
            "a middle constructor with no super()",
            "class Mid : G { public Mid() { this.b = \"b\"; } }",
        ),
        (
            "a middle class with no constructor at all",
            "class Mid : G { }",
        ),
    ] {
        match evaluate(&format!(
            "{hierarchy}
             {mid}
             class Leaf : Mid {{ public Leaf() {{ this.c = \"c\"; }} }}
             new Leaf().a;"
        )) {
            ProgramOutcome::RuntimeError(error) => {
                assert_eq!(error.code, "SZ4001", "{what}: {error:?}");
                assert_eq!(error.kind, "ReferenceError", "{what}: {error:?}");
            }
            other => panic!("{what} must leave the grandparent unrun, got {other:?}"),
        }
    }

    // An explicit super() in the middle carries the chain up.
    match evaluate(&format!(
        "{hierarchy}
         class Mid : G {{ public Mid() {{ super(); this.b = \"b\"; }} }}
         class Leaf : Mid {{ public Leaf() {{ this.c = \"c\"; }} }}
         let leaf = new Leaf();
         if (leaf.a != \"G\" || leaf.b != \"b\" || leaf.c != \"c\") {{
             throw \"an explicit super() must carry the chain\";
         }}"
    )) {
        ProgramOutcome::Value(_) => {}
        other => panic!("explicit super() must chain, got {other:?}"),
    }
}
