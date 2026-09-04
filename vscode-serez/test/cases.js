'use strict';

// Formatter cases, as data. Each is `{ name, fn, input, expect }`:
//
//   fn      — 'sz' | 'szx' | 'szs'
//   input   — the document as the user typed it
//   expect  — the exact expected output, or `INPUT` when the formatter must
//             leave it alone (up to the trailing-newline policy)
//
// Every case is also checked for idempotence by the runner, so none of them has
// to assert that separately.
//
// The syntax here is real Serez, checked against `src/lexer.rs`, `src/parser/`
// and the corpus — not carried over from another language. In particular class
// members carry `public`/`private` and there is no bare `fn` inside a class
// body.

const INPUT = Symbol('unchanged');

const cases = [
    // ── 1–3: the smallest documents ─────────────────────────────────────────
    { name: 'empty file', fn: 'sz', input: '', expect: '' },
    // The blank lines go; the document still ended with a newline, so the
    // output does too.
    { name: 'only blank lines', fn: 'sz', input: '\n\n\n', expect: '\n' },
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
        name: 'trailing blank lines are dropped, and the newline state is kept',
        fn: 'sz',
        input: 'out 1;\n\n\n',
        expect: 'out 1;\n',
    },
    {
        name: 'a missing final newline is NOT added',
        fn: 'sz',
        input: 'out 1;',
        expect: INPUT,
    },
    {
        name: 'an existing final newline is kept',
        fn: 'sz',
        input: 'out 1;\n',
        expect: INPUT,
    },
    {
        name: 'a CRLF final newline is kept as CRLF',
        fn: 'sz',
        input: 'out 1;\r\n',
        expect: INPUT,
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
        expect: INPUT,
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
];

module.exports = { cases, INPUT };
