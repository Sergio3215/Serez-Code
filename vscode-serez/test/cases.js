'use strict';

// Formatter cases, as data. Each is `{ name, fn, input, expect }`:
//
//   fn      — 'sz' | 'szx' | 'szs'
//   input   — the document as the user typed it
//   expect  — the exact expected output, or `INPUT` when the formatter must
//             leave the document alone, or `VERBATIM` for the one case where
//             it must not touch a single byte
//
// **Formatted output never ends with a newline.** The runner applies that to
// every expectation, so the strings below can be written the way a document
// reads — ending in `\n` — without every one of them restating the policy. The
// EOF cases assert it directly, and nothing else has to.
//
// `INPUT` and `VERBATIM` differ only there. `INPUT` means "the formatter
// changes nothing except the final newline it always drops"; `VERBATIM` means
// "byte for byte, newline included", which is what DEC-FMT-002 requires of a
// document the formatter declined to read.
//
// Every case is also checked for idempotence by the runner, so none of them has
// to assert that separately.
//
// The syntax here is real Serez, checked against `src/lexer.rs`, `src/parser/`
// and the corpus — not carried over from another language. In particular class
// members carry `public`/`private` and there is no bare `fn` inside a class
// body.

const INPUT = Symbol('unchanged');
const VERBATIM = Symbol('untouched, final newline included');

const cases = [
    // ── 1–3: the smallest documents ─────────────────────────────────────────
    { name: 'empty file', fn: 'sz', input: '', expect: '' },
    // The blank lines go, and nothing is left to end — not even a line break.
    { name: 'only blank lines', fn: 'sz', input: '\n\n\n', expect: '' },
    { name: 'one statement', fn: 'sz', input: 'out 1;\n', expect: INPUT },

    // ── indentation ─────────────────────────────────────────────────────────
    {
        name: 'a flat block is indented',
        fn: 'sz',
        input: 'fn void f() {\nout 1;\n}\n',
        expect: 'fn void f() {\n    out 1;\n}\n',
    },
    {
        name: 'an over-indented block is corrected',
        fn: 'sz',
        input: 'fn void f() {\n            out 1;\n}\n',
        expect: 'fn void f() {\n    out 1;\n}\n',
    },
    {
        name: 'nested blocks',
        fn: 'sz',
        input: 'fn void f() {\nif (true) {\nwhile (false) {\nout 1;\n}\n}\n}\n',
        expect:
            'fn void f() {\n    if (true) {\n        while (false) {\n' +
            '            out 1;\n        }\n    }\n}\n',
    },
    {
        name: 'else on the closing brace line',
        fn: 'sz',
        input: 'fn void f() {\nif (true) {\nout 1;\n} else {\nout 2;\n}\n}\n',
        expect:
            'fn void f() {\n    if (true) {\n        out 1;\n    } else {\n' +
            '        out 2;\n    }\n}\n',
    },
    {
        name: 'a class with public members',
        fn: 'sz',
        input:
            'public class Point {\npublic Point(int x) {\nthis.x = x;\n}\n' +
            'public int get() {\nreturn this.x;\n}\n}\n',
        expect:
            'public class Point {\n    public Point(int x) {\n        this.x = x;\n    }\n' +
            '    public int get() {\n        return this.x;\n    }\n}\n',
    },
    {
        name: 'a nested fn inside a function body',
        fn: 'sz',
        input: 'fn int outer(int n) {\nfn int inner(int m) {\nreturn m;\n}\nreturn inner(n);\n}\n',
        expect:
            'fn int outer(int n) {\n    fn int inner(int m) {\n        return m;\n    }\n' +
            '    return inner(n);\n}\n',
    },
    {
        name: 'a lambda body',
        fn: 'sz',
        input: 'let f = () => {\nout 1;\n};\n',
        expect: 'let f = () => {\n    out 1;\n};\n',
    },
    {
        name: 'try / catch',
        fn: 'sz',
        input: 'try {\nout 1;\n} catch (e) {\nout 2;\n}\n',
        expect: 'try {\n    out 1;\n} catch (e) {\n    out 2;\n}\n',
    },
    {
        name: 'an empty block stays on one line',
        fn: 'sz',
        input: 'fn void f() { }\nout 1;\n',
        expect: INPUT,
    },
    {
        name: 'a one-line block is not split',
        fn: 'sz',
        input: 'fn int f() { return 1; }\nout f();\n',
        expect: INPUT,
    },

    // ── spacing is NOT touched: the formatter indents, it does not respace ──
    {
        name: 'unary minus is left alone',
        fn: 'sz',
        input: 'let a = -x;\nlet b = !flag;\n',
        expect: INPUT,
    },
    {
        name: 'member access is left alone',
        fn: 'sz',
        input: 'out obj.field.method();\n',
        expect: INPUT,
    },
    {
        name: 'calls, indexing and chains are left alone',
        fn: 'sz',
        input: 'out list[0].map(f).filter(g);\n',
        expect: INPUT,
    },
    {
        name: 'binary and assignment spacing is the author\u2019s',
        fn: 'sz',
        input: 'let a = 1+2*3;\nlet b = 1 + 2 * 3;\na += b;\n',
        expect: INPUT,
    },
    {
        name: 'a dict type annotation is not a tag',
        fn: 'sz',
        input: 'let d <string, any> = ();\nout d;\n',
        expect: INPUT,
    },

    // ── strings ─────────────────────────────────────────────────────────────
    {
        name: 'a brace inside a string does not open a block',
        fn: 'sz',
        input: 'let s = "{";\nlet t = 1;\nout t;\n',
        expect: INPUT,
    },
    {
        name: 'a closing brace inside a string does not close a block',
        fn: 'sz',
        input: 'fn void f() {\n    let s = "}";\n    out s;\n}\n',
        expect: INPUT,
    },
    {
        name: 'escapes do not end the string early',
        fn: 'sz',
        input: 'fn void f() {\n    let s = "a\\"b}";\n    out s;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a multi-line string keeps its own indentation',
        fn: 'sz',
        input: 'fn void f() {\n    let s = "line one\nline two";\n    out s;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a multi-line string containing braces',
        fn: 'sz',
        input: 'fn void f() {\n    let s = "a {\nb }";\n    out s;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a raw string ending in a backslash closes there',
        fn: 'sz',
        input: 'fn void f() {\n    let p = r"C:\\dir\\";\n    if (true) {\n        out 1;\n    }\n}\n',
        expect: INPUT,
    },
    {
        // Discriminating on purpose: the input is BADLY indented, so the
        // formatter has to re-indent it. If `r"` were treated as a normal
        // string, the `\\"` would read as an escaped quote, the string would
        // never close, and every following line would be held verbatim at
        // column 0. A well-indented input cannot tell the two apart.
        name: 'a raw string closes at the backslash-quote and code resumes',
        fn: 'sz',
        input: 'fn void f() {\nlet p = r"a\\";\nout 1;\n}\n',
        expect: 'fn void f() {\n    let p = r"a\\";\n    out 1;\n}\n',
    },
    {
        name: 'a raw string containing a brace',
        fn: 'sz',
        input: 'fn void f() {\n    let p = r"a{b";\n    out p;\n}\n',
        expect: INPUT,
    },
    {
        name: 'an identifier ending in r before a string is not a raw string',
        fn: 'sz',
        input: 'fn void f() {\n    let manager = "x";\n    out manager;\n}\n',
        expect: INPUT,
    },
    {
        name: 'interpolation containing a nested string',
        fn: 'sz',
        input: 'fn void f() {\n    let n = "x";\n    let s = "{ n + "!" }";\n    out s;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a // inside a string is not a comment',
        fn: 'sz',
        input: 'fn void f() {\n    let s = "http://x";\n    if (true) {\n        out 1;\n    }\n}\n',
        expect: INPUT,
    },

    // ── comments ────────────────────────────────────────────────────────────
    {
        name: 'a brace in a line comment does not open a block',
        fn: 'sz',
        input: 'let a = 1; // {\nlet b = 2;\nout a + b;\n',
        expect: INPUT,
    },
    {
        name: 'a closing brace in a line comment does not close a block',
        fn: 'sz',
        input: 'fn void f() {\n    // }\n    out 1;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a closing brace in a block comment does not close a block',
        fn: 'sz',
        input: 'fn void f() {\n    /* } */\n    out 1;\n}\n',
        expect: INPUT,
    },
    {
        name: 'an opening brace in a block comment does not open a block',
        fn: 'sz',
        input: 'fn void f() {\n    /* { */\n    out 1;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a multi-line block comment keeps its alignment',
        fn: 'sz',
        input: 'fn void f() {\n    /*\n     * Title\n     *   detail\n     */\n    out 1;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a comment before a declaration stays put',
        fn: 'sz',
        input: '// what this does\nfn void f() {\n    out 1;\n}\n',
        expect: INPUT,
    },
    {
        name: 'a trailing comment after a statement stays on its line',
        fn: 'sz',
        input: 'fn void f() {\n    out 1; // why\n}\n',
        expect: INPUT,
    },
    {
        name: 'a comment beside a closing brace',
        fn: 'sz',
        input: 'fn void f() {\n    out 1;\n} // done\n',
        expect: INPUT,
    },

    // ── whitespace policy ───────────────────────────────────────────────────
    {
        name: 'trailing whitespace is removed',
        fn: 'sz',
        input: 'out 1;   \nout 2;\t\n',
        expect: 'out 1;\nout 2;\n',
    },
    {
        name: 'runs of blank lines collapse to one',
        fn: 'sz',
        input: 'out 1;\n\n\n\nout 2;\n',
        expect: 'out 1;\n\nout 2;\n',
    },
    {
        name: 'trailing blank lines are dropped',
        fn: 'sz',
        input: 'out 1;\n\n\n',
        expect: 'out 1;',
    },

    // ── the end of the document ─────────────────────────────────────────────
    //
    // Formatted output has no trailing newline. The three inputs a document can
    // arrive as — no break, LF, CRLF — all leave as the same bytes, which is
    // the point: how the document ends stops depending on how it arrived.
    //
    // These spell the expectation out as a literal rather than leaning on the
    // runner's rule, so the policy is asserted somewhere that does not move
    // when the runner does.
    {
        name: 'EOF: a document with no final newline does not gain one',
        fn: 'sz',
        input: 'out 1;',
        expect: 'out 1;',
    },
    {
        name: 'EOF: an LF final newline is removed',
        fn: 'sz',
        input: 'out 1;\n',
        expect: 'out 1;',
    },
    {
        name: 'EOF: a CRLF final newline is removed',
        fn: 'sz',
        input: 'out 1;\r\n',
        expect: 'out 1;',
    },
    {
        name: 'EOF: the rule survives a document with structure',
        fn: 'sz',
        input: 'fn void f() {\nout 1;\n}\n',
        expect: 'fn void f() {\n    out 1;\n}',
    },
    {
        // The distinction that matters: this is about the *end* of the
        // document, not about LF vs CRLF *inside* it. The interior breaks are
        // still CRLF; only the last one is gone.
        name: 'EOF: removing the last break does not touch the interior endings',
        fn: 'sz',
        input: 'fn void f() {\r\nout 1;\r\n}\r\n',
        expect: 'fn void f() {\r\n    out 1;\r\n}',
    },
    {
        name: 'EOF: szx has no trailing newline either',
        fn: 'szx',
        input: 'out 1;\n',
        expect: 'out 1;',
    },
    {
        name: 'EOF: szx, CRLF',
        fn: 'szx',
        input: '<div>\n<p>hi</p>\n</div>\r\n',
        expect: '<div>\n    <p>hi</p>\n</div>',
    },
    {
        name: 'EOF: szs has no trailing newline either',
        fn: 'szs',
        input: '.a {\ncolor: red;\n}\n',
        expect: '.a {\n    color: red;\n}',
    },
    {
        name: 'EOF: szs, CRLF',
        fn: 'szs',
        input: '.a { color: red; }\r\n',
        expect: '.a { color: red; }',
    },

    // ── incomplete documents: the formatter runs while you type ─────────────
    {
        name: 'an unclosed brace loses nothing',
        fn: 'sz',
        input: 'fn void f() {\nout 1;\n',
        expect: 'fn void f() {\n    out 1;\n',
    },
    {
        name: 'an unclosed paren loses nothing',
        fn: 'sz',
        input: 'out foo(1,\n',
        expect: INPUT,
    },
    {
        name: 'an unclosed bracket loses nothing',
        fn: 'sz',
        input: 'let a = [1,\n',
        expect: INPUT,
    },
    {
        name: 'an unterminated string is left verbatim',
        fn: 'sz',
        input: 'fn void f() {\n    let s = "not closed\n    out 1;\n}\n',
        expect: 'fn void f() {\n    let s = "not closed\n    out 1;\n}\n',
    },
    {
        name: 'an unterminated block comment is left verbatim',
        fn: 'sz',
        input: 'fn void f() {\n    /* not closed\n    out 1;\n}\n',
        expect: 'fn void f() {\n    /* not closed\n    out 1;\n}\n',
    },
    {
        name: 'a stray closing brace does not go negative',
        fn: 'sz',
        input: '}\n}\nout 1;\n',
        expect: VERBATIM,
    },
    {
        name: 'a half-written declaration is left alone',
        fn: 'sz',
        input: 'fn void\n',
        expect: INPUT,
    },

    // A line starting with an unambiguously infix token continues the
    // expression above it and keeps its own indentation.
    {
        name: 'a logical operator continuation is indented one level',
        fn: 'sz',
        input: 'let ok = a\n    && b\n    || c;\n',
        expect: INPUT,
    },
    {
        name: 'a pipe continuation is indented one level',
        fn: 'sz',
        input: 'let r = xs\n    |> f\n    |> g;\n',
        expect: INPUT,
    },
    {
        name: 'a comma continuation sits at the group level',
        fn: 'sz',
        input: 'let t = f(a\n    , b);\n',
        expect: INPUT,
    },
    {
        name: 'a leading minus is NOT treated as a continuation',
        fn: 'sz',
        // `-x;` is a statement, so the formatter cannot assume a leading `-`
        // continues the line above. Documented gap, asserted so it stays a
        // decision rather than becoming an accident.
        input: 'fn void f() {\nlet a = -x;\n}\n',
        expect: 'fn void f() {\n    let a = -x;\n}\n',
    },
    // Continuation lines: the formatter indents blocks, not expressions.
    {
        name: 'a correctly indented chain is left alone',
        fn: 'sz',
        input: 'let r = nums\n    .filter(f)\n    .map(g);\n',
        expect: INPUT,
    },
    {
        // DEC-FMT-001: running the formatter normalises indentation. An
        // over-indented chain is brought back to one level, not preserved.
        name: 'an over-indented chain is normalised',
        fn: 'sz',
        input: 'let r = nums\n        .filter(f)\n        .map(g);\n',
        expect: 'let r = nums\n    .filter(f)\n    .map(g);\n',
    },
    {
        name: 'multi-line call arguments are indented one level',
        fn: 'sz',
        input: 'let xs = [\n    new P(1),\n    new P(2),\n];\n',
        expect: INPUT,
    },
    {
        name: 'a block after a closed call is still indented',
        fn: 'sz',
        input: 'let x = f(1);\nfn void g() {\nout 1;\n}\n',
        expect: 'let x = f(1);\nfn void g() {\n    out 1;\n}\n',
    },
    {
        name: 'trailing whitespace goes on a continuation line too',
        fn: 'sz',
        input: 'let r = nums\n    .filter(f)   \n    .map(g);\n',
        expect: 'let r = nums\n    .filter(f)\n    .map(g);\n',
    },
    {
        name: 'trailing whitespace inside a multi-line string is content',
        fn: 'sz',
        input: 'let s = "a   \nb";\nout s;\n',
        expect: INPUT,
    },

    // DEC-FMT-002: a document that contradicts itself comes back exactly as
    // it went in — `VERBATIM`, so the final newline stays too. That path does
    // not format at all, and stripping a byte off a document the formatter just
    // declined to read would be the one edit it is not entitled to make. These
    // assert the preservation; the warning is provider-side and is covered by
    // test/provider.js.
    {
        name: 'a group closed by a brace leaves the document untouched',
        fn: 'sz',
        input: 'fn void f() {\nfoo(}\n}\n',
        expect: VERBATIM,
    },
    {
        name: 'a bracket closed by a paren leaves the document untouched',
        fn: 'sz',
        input: 'let a = [1, 2);\n',
        expect: VERBATIM,
    },
    {
        name: 'a closer that closes nothing leaves the document untouched',
        fn: 'sz',
        input: 'out 1;\n}\n',
        expect: VERBATIM,
    },
    {
        // The control: badly indented AND contradictory. If the formatter
        // pressed on it would re-indent this; it must not.
        name: 'a contradictory document is not partially reformatted',
        fn: 'sz',
        input: 'fn void f() {\n            out 1;\nfoo(}\n}\n',
        expect: VERBATIM,
    },
    {
        name: 'szx: a contradictory document is left untouched',
        fn: 'szx',
        input: 'fn void f() {\nfoo(}\n}\n',
        expect: VERBATIM,
    },

    // Incomplete but determinable: formatted normally, no warning.
    {
        name: 'an open group is still indented',
        fn: 'sz',
        input: 'foo(\na,\nb\n',
        expect: 'foo(\n    a,\n    b\n',
    },
    {
        name: 'an open block is still indented',
        fn: 'sz',
        input: 'fn void f() {\nout 1;\n',
        expect: 'fn void f() {\n    out 1;\n',
    },

    // ── .szx ────────────────────────────────────────────────────────────────
    {
        name: 'szx: a JSX tree is indented by tag depth',
        fn: 'szx',
        input: '<div>\n<span>hi</span>\n</div>\n',
        expect: '<div>\n    <span>hi</span>\n</div>\n',
    },
    {
        // The idiom every serez-ui component uses. The formatter used to
        // pull the whole tree one level left, because `(` was not tracked.
        name: 'szx: JSX inside return ( ) is indented by group and tag depth',
        fn: 'szx',
        input:
            'public class C {\n    public C() { this.n = 0; }\n' +
            '    public render() {\n        return (\n' +
            '            <div>\n                <h1>hi</h1>\n' +
            '            </div>\n        );\n    }\n}\n',
        expect: INPUT,
    },
    {
        name: 'szx: a self-closing tag does not indent',
        fn: 'szx',
        input: '<div>\n<hr />\n<span>x</span>\n</div>\n',
        expect: '<div>\n    <hr />\n    <span>x</span>\n</div>\n',
    },
    {
        name: 'szx: a comparison is not a tag',
        fn: 'szx',
        input: 'fn void f() {\n    if (a < b) {\n        out 1;\n    }\n}\n',
        expect: INPUT,
    },
    {
        name: 'szx: a dict annotation is not a tag',
        fn: 'szx',
        input: 'fn void f() {\n    let d <string, any> = ();\n    out d;\n}\n',
        expect: INPUT,
    },
    {
        name: 'szx: a raw string is respected',
        fn: 'szx',
        input: 'fn void f() {\n    let p = r"a\\";\n    out p;\n}\n',
        expect: INPUT,
    },
    {
        name: 'szx: a block comment brace is not a block',
        fn: 'szx',
        input: 'fn void f() {\n    /* } */\n    out 1;\n}\n',
        expect: INPUT,
    },

    // ── .szs ────────────────────────────────────────────────────────────────
    {
        name: 'szs: a rule is indented',
        fn: 'szs',
        input: '.a {\ncolor: red;\n}\n',
        expect: '.a {\n    color: red;\n}\n',
    },
    {
        name: 'szs: a one-line rule is left as written',
        fn: 'szs',
        input: '.a { color: red; }\n',
        expect: INPUT,
    },
    {
        name: 'szs: a brace in a block comment is not a block',
        fn: 'szs',
        input: '.a {\n    /* } */\n    color: red;\n}\n',
        expect: INPUT,
    },

    // ── dictionary access, both forms ───────────────────────────────────────
    //
    // `dic.name` and `dic["name"]` read the same key, and a program may mix
    // them in one expression. The formatter is an indenter: on one line it has
    // nothing to do to any of these, and the cases exist to prove that a `.`
    // between identifiers is never mistaken for the leading `.` of a
    // continuation, and that a `[` after an identifier is indexing rather than
    // an array literal opening a level.
    {
        name: 'access: dot access is left as written',
        fn: 'sz',
        input: 'out dic.name;\n',
        expect: INPUT,
    },
    {
        name: 'access: bracket access is left as written',
        fn: 'sz',
        input: 'out dic["name"];\n',
        expect: INPUT,
    },
    {
        name: 'access: a chain of dots is left as written',
        fn: 'sz',
        input: 'out dic.user.name;\n',
        expect: INPUT,
    },
    {
        name: 'access: brackets then dot',
        fn: 'sz',
        input: 'out dic["user"].name;\n',
        expect: INPUT,
    },
    {
        name: 'access: dot then brackets',
        fn: 'sz',
        input: 'out dic.user["name"];\n',
        expect: INPUT,
    },
    {
        name: 'access: a declaration and both forms of read',
        fn: 'sz',
        input: 'let dic <string, any> = ({"name", "Sergio"});\nout dic["name"];\nout dic.name;\n',
        expect: INPUT,
    },
    {
        // The dict type annotation is `<string, any>`; the `<` and `>` in it
        // must not be read as JSX tags, which is what would push the following
        // lines a level in.
        name: 'access: the dict annotation does not open a level',
        fn: 'sz',
        input: 'let dic <string, any> = ({"name", "Sergio"});\nout 1;\n',
        expect: INPUT,
    },
    {
        name: 'access: indexing a value reached through a dot',
        fn: 'sz',
        input: 'out dic.tags[0];\n',
        expect: INPUT,
    },
    {
        name: 'access: inside a block, the access is indented with the line',
        fn: 'sz',
        input: 'fn void f() {\nout dic.user.name;\n}\n',
        expect: 'fn void f() {\n    out dic.user.name;\n}',
    },
    {
        name: 'access: szx, a dict read inside an expression hole',
        fn: 'szx',
        input: '<div>\n<p>{dic.user.name}</p>\n</div>\n',
        expect: '<div>\n    <p>{dic.user.name}</p>\n</div>',
    },

    // ── multi-line access chains — DEC-FMT-001 ──────────────────────────────
    //
    // Running the formatter formats: a chain broken across lines is re-indented
    // one level past the line it continues, like any other continuation. A
    // leading `.` cannot start a statement, so it is recognised without needing
    // to know what the expression means.
    {
        name: 'chain: a dot chain broken across lines is indented one level',
        fn: 'sz',
        input: 'let v = dic\n.user\n.name;\n',
        expect: 'let v = dic\n    .user\n    .name;',
    },
    {
        name: 'chain: badly indented, DEC-FMT-001 normalises it',
        fn: 'sz',
        input: 'let v = dic\n            .user\n  .name;\n',
        expect: 'let v = dic\n    .user\n    .name;',
    },
    {
        name: 'chain: inside a block, one level past the statement',
        fn: 'sz',
        input: 'fn void f() {\nlet v = dic\n.user\n.name;\n}\n',
        expect: 'fn void f() {\n    let v = dic\n        .user\n        .name;\n}',
    },
    {
        name: 'chain: a method call chain',
        fn: 'sz',
        input: 'let v = dic.keys()\n.length();\n',
        expect: 'let v = dic.keys()\n    .length();',
    },
    {
        // Measured, and left as it is on purpose. A line opening with `[` is
        // ambiguous in the same way a leading `+` or `-` is: `["user"]` can
        // index the line above, and `[1, 2]` can open an array literal that
        // starts a statement — Serez does not require semicolons, so the
        // previous token cannot settle it either, since `let v = dic` is
        // already a complete statement.
        //
        // `.name` on the next line *is* recognised, because a leading `.`
        // cannot start a statement. So this comes back half-indented, which is
        // honest about what a character scanner can know. Telling the two apart
        // needs the parser; DEC-FMT-002's principle applies — do not guess —
        // and this is recorded in FORMATTER.md's known limits rather than
        // approximated here.
        name: 'chain: a line opening with a bracket is not a continuation',
        fn: 'sz',
        input: 'let v = dic\n["user"]\n.name;\n',
        expect: 'let v = dic\n["user"]\n    .name;',
    },
];

module.exports = { cases, INPUT, VERBATIM };
