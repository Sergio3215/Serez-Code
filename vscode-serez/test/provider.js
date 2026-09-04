'use strict';

// The VS Code side of the contract, exercised with a stubbed `vscode` module.
//
// `extension.js` is the only file that touches the editor API, and the parts
// worth pinning are all decisions rather than plumbing: which options reach the
// formatter, what a cancelled request returns, what happens when the formatter
// throws, and — DEC-FMT-002 — when the user is warned and how often.
//
// Stubbing `require('vscode')` is what makes this runnable in plain node. The
// alternative is `@vscode/test-electron`, which downloads a VS Code build to
// assert four things about a pure function's caller.

const Module = require('module');

/** Load `extension.js` against a fake editor, and return what it did. */
function loadExtension() {
    const warnings = [];
    const errors = [];
    const registered = [];

    const vscodeStub = {
        languages: {
            registerDocumentFormattingEditProvider(selector, provider) {
                registered.push({ selector, provider });
                return { dispose() {} };
            },
        },
        workspace: {
            isTrusted: true,
            getConfiguration: () => ({ get: (key, fallback) => fallback }),
        },
        window: {
            showWarningMessage(message) { warnings.push(message); },
        },
        TextEdit: { replace: (range, text) => ({ range, text }) },
        Range: function Range(start, end) { this.start = start; this.end = end; },
    };

    const realLoad = Module._load;
    Module._load = function (request, parent, isMain) {
        if (request === 'vscode') return vscodeStub;
        if (request.startsWith('vscode-languageclient')) {
            const e = new Error('not bundled in tests');
            e.code = 'MODULE_NOT_FOUND';
            throw e;
        }
        return realLoad.apply(this, arguments);
    };

    // Fresh each time: the module caches its `vscode` binding.
    delete require.cache[require.resolve('../extension.js')];
    const realError = console.error;
    const realWarn = console.warn;
    console.error = (...a) => errors.push(a.join(' '));
    console.warn = () => {};
    try {
        const extension = require('../extension.js');
        const subscriptions = [];
        extension.activate({ subscriptions });
        return { registered, warnings, errors };
    } finally {
        console.error = realError;
        console.warn = realWarn;
        Module._load = realLoad;
    }
}

/** A document stub good enough for the provider. */
function doc(text) {
    return { getText: () => text, positionAt: n => n };
}

const failures = [];
let passed = 0;

function check(name, condition, detail) {
    if (condition) passed++;
    else failures.push(`${name}${detail ? ' — ' + detail : ''}`);
}

// ── registration ────────────────────────────────────────────────────────────

{
    const { registered } = loadExtension();
    check('one provider per language', registered.length === 3,
        `got ${registered.length}`);
    const languages = registered.map(r => r.selector.language).sort();
    check('the three Serez languages are covered',
        languages.join(',') === 'serez-code,serez-code-jsx,serez-style',
        languages.join(','));
    check('the selector is scoped to files',
        registered.every(r => r.selector.scheme === 'file'));
}

// ── editor options reach the formatter ──────────────────────────────────────

{
    const { registered } = loadExtension();
    const sz = registered.find(r => r.selector.language === 'serez-code').provider;
    const source = 'fn void f() {\nout 1;\n}\n';
    const token = { isCancellationRequested: false };

    const four = sz.provideDocumentFormattingEdits(
        doc(source), { tabSize: 4, insertSpaces: true }, token);
    check('tabSize 4 indents four spaces',
        four[0] && four[0].text === 'fn void f() {\n    out 1;\n}\n',
        four[0] && JSON.stringify(four[0].text));

    const two = sz.provideDocumentFormattingEdits(
        doc(source), { tabSize: 2, insertSpaces: true }, token);
    check('tabSize 2 indents two spaces',
        two[0] && two[0].text === 'fn void f() {\n  out 1;\n}\n',
        two[0] && JSON.stringify(two[0].text));

    const tabs = sz.provideDocumentFormattingEdits(
        doc(source), { insertSpaces: false }, token);
    check('insertSpaces false indents with a tab',
        tabs[0] && tabs[0].text === 'fn void f() {\n\tout 1;\n}\n',
        tabs[0] && JSON.stringify(tabs[0].text));
}

// ── no edit when there is nothing to do, or the request is gone ─────────────

{
    const { registered } = loadExtension();
    const sz = registered.find(r => r.selector.language === 'serez-code').provider;
    const options = { tabSize: 4, insertSpaces: true };

    const cancelled = sz.provideDocumentFormattingEdits(
        doc('fn void f() {\nout 1;\n}\n'), options, { isCancellationRequested: true });
    check('a cancelled request produces no edit', cancelled.length === 0);

    const already = sz.provideDocumentFormattingEdits(
        doc('out 1;\n'), options, { isCancellationRequested: false });
    check('an already-formatted document produces no edit', already.length === 0);
}

// ── DEC-FMT-002: the warning ────────────────────────────────────────────────

{
    const { registered, warnings } = loadExtension();
    const sz = registered.find(r => r.selector.language === 'serez-code').provider;
    const options = { tabSize: 4, insertSpaces: true };
    const token = { isCancellationRequested: false };

    // `(` closed by `}` — the document contradicts itself.
    const edits = sz.provideDocumentFormattingEdits(
        doc('fn void f() {\nfoo(}\n}\n'), options, token);

    check('a contradictory document produces no edit', edits.length === 0);
    check('the user is warned once', warnings.length === 1,
        `${warnings.length} warning(s)`);
    check('the warning names a possible syntax error',
        warnings[0] && /syntax error/i.test(warnings[0]), warnings[0]);
    check('the warning says the document was left alone',
        warnings[0] && /left unchanged/i.test(warnings[0]), warnings[0]);
}

{
    // Incomplete but determinable code must NOT warn: this is what the user is
    // doing most of the time.
    const { registered, warnings } = loadExtension();
    const sz = registered.find(r => r.selector.language === 'serez-code').provider;
    const options = { tabSize: 4, insertSpaces: true };
    const token = { isCancellationRequested: false };

    for (const source of ['foo(\n', 'fn void f() {\nout 1;\n', 'let s = "abc\n', '/* abc\n']) {
        sz.provideDocumentFormattingEdits(doc(source), options, token);
    }
    check('incomplete code does not warn', warnings.length === 0,
        warnings.join(' | '));
}

{
    // One message per run, even though the document has several problems.
    const { registered, warnings } = loadExtension();
    const sz = registered.find(r => r.selector.language === 'serez-code').provider;
    sz.provideDocumentFormattingEdits(
        doc('foo(}\nbar(]\nbaz)\n'),
        { tabSize: 4, insertSpaces: true },
        { isCancellationRequested: false });
    check('several contradictions still warn once', warnings.length === 1,
        `${warnings.length} warning(s)`);
}

// ── a formatter that throws must not take format-on-save down ──────────────

{
    const { registered } = loadExtension();
    const jsx = registered.find(r => r.selector.language === 'serez-code-jsx').provider;
    const exploding = {
        getText() { throw new Error('boom'); },
        positionAt: n => n,
    };
    let threw = false;
    let edits = null;
    try {
        edits = jsx.provideDocumentFormattingEdits(
            exploding, { tabSize: 4, insertSpaces: true }, { isCancellationRequested: false });
    } catch (e) {
        threw = true;
    }
    // getText() throwing is outside the try, so this documents where the guard
    // is: around the format call, which is the part that can fail on input.
    check('a throwing document surfaces rather than corrupting', threw || edits.length === 0);
}

// ── report ──────────────────────────────────────────────────────────────────

if (failures.length === 0) {
    console.log(`provider: ok — ${passed} checks`);
    process.exit(0);
}
console.log(`provider: FAILED — ${failures.length} failure(s), ${passed} passed`);
for (const f of failures) console.log('  ' + f);
process.exit(1);
