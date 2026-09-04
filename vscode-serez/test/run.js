'use strict';

// Formatter test runner. Plain node, no VS Code, no dependencies:
//
//   node test/run.js
//
// Four suites, because they answer different questions:
//
//   cases        — input → formatter → exact expected output
//   idempotence  — F(F(x)) == F(x) for every case and every corpus file
//   line endings — CRLF in, CRLF out; LF in, LF out
//   corpus       — every real `.sz` in the repository, read-only
//
// The corpus suite reads the repository's own fixtures and never writes to
// them. It is the difference between "the cases I thought of pass" and "1 MB of
// real code survives".

const fs = require('fs');
const path = require('path');
const { cases, INPUT } = require('./cases');
const formatter = require('../formatter');

const FORMATTERS = {
    sz: formatter.formatSz,
    szx: formatter.formatSzx,
    szs: formatter.formatSzs,
};

let passed = 0;
const failures = [];

function fail(suite, name, detail) {
    failures.push({ suite, name, detail });
}

function show(s) {
    return JSON.stringify(s);
}

// ── 1. cases ────────────────────────────────────────────────────────────────

for (const c of cases) {
    const format = FORMATTERS[c.fn];
    if (!format) {
        fail('cases', c.name, `unknown formatter '${c.fn}'`);
        continue;
    }

    let out;
    try {
        out = format(c.input);
    } catch (e) {
        fail('cases', c.name, `threw: ${e && e.message}`);
        continue;
    }

    // `INPUT` means "leave it alone", up to the documented trailing-newline
    // policy — a file that does not end in a newline gets one.
    const expected = c.expect === INPUT
        ? (c.input.endsWith('\n') || c.input === '' ? c.input : c.input + '\n')
        : c.expect;

    if (out !== expected) {
        fail('cases', c.name, `\n      in       ${show(c.input)}\n      expected ${show(expected)}\n      actual   ${show(out)}`);
        continue;
    }

    // Every case is an idempotence case too. A formatter that produced the
    // right answer once and a different one on save-again would pass the
    // assertion above and still be unusable.
    const twice = format(out);
    if (twice !== out) {
        fail('idempotence', c.name, `\n      once  ${show(out)}\n      twice ${show(twice)}`);
        continue;
    }

    passed++;
}

// ── 2. line endings ─────────────────────────────────────────────────────────

{
    const body = 'fn void f() {\nout 1;\n}\n';
    const crlf = body.split('\n').join('\r\n');

    const lfOut = formatter.formatSz(body);
    if (/\r/.test(lfOut)) fail('line endings', 'LF stays LF', show(lfOut));
    else passed++;

    const crlfOut = formatter.formatSz(crlf);
    if (!/\r\n/.test(crlfOut) || /\r(?!\n)/.test(crlfOut)) {
        fail('line endings', 'CRLF stays CRLF', show(crlfOut));
    } else if (formatter.formatSz(crlfOut) !== crlfOut) {
        fail('line endings', 'CRLF is idempotent', show(crlfOut));
    } else {
        passed++;
    }

    // The two differ only in their endings — the same document either way.
    if (crlfOut.split('\r\n').join('\n') !== lfOut) {
        fail('line endings', 'CRLF and LF format to the same document', show(crlfOut));
    } else {
        passed++;
    }
}

// ── 3. editor options ───────────────────────────────────────────────────────

{
    const src = 'fn void f() {\nout 1;\n}\n';
    const checks = [
        ['default is 4 spaces', undefined, 'fn void f() {\n    out 1;\n}\n'],
        ['tabSize 2', { tabSize: 2, insertSpaces: true }, 'fn void f() {\n  out 1;\n}\n'],
        ['tabSize 8', { tabSize: 8, insertSpaces: true }, 'fn void f() {\n        out 1;\n}\n'],
        ['tabs', { insertSpaces: false }, 'fn void f() {\n\tout 1;\n}\n'],
    ];
    for (const [name, options, expected] of checks) {
        const out = formatter.formatSz(src, options);
        if (out !== expected) fail('options', name, `expected ${show(expected)} got ${show(out)}`);
        else if (formatter.formatSz(out, options) !== out) fail('options', name + ' idempotent', show(out));
        else passed++;
    }
}

// ── 4. corpus ───────────────────────────────────────────────────────────────

const ROOT = path.resolve(__dirname, '..', '..');

function collect(dir, ext, out) {
    let entries;
    try {
        entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch (e) {
        return out;
    }
    for (const entry of entries) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
            if (entry.name === 'node_modules' || entry.name === 'target') continue;
            collect(full, ext, out);
        } else if (entry.name.endsWith(ext)) {
            out.push(full);
        }
    }
    return out;
}

{
    const files = [];
    for (const d of ['tests', 'std', 'examples']) collect(path.join(ROOT, d), '.sz', files);

    let unstable = 0;
    let crashed = 0;
    const started = Date.now();

    for (const file of files) {
        const src = fs.readFileSync(file, 'utf8');
        let once, twice;
        try {
            once = formatter.formatSz(src);
            twice = formatter.formatSz(once);
        } catch (e) {
            crashed++;
            fail('corpus', path.relative(ROOT, file), `threw: ${e && e.message}`);
            continue;
        }
        if (twice !== once) {
            unstable++;
            fail('corpus', path.relative(ROOT, file), 'not idempotent');
        }
    }

    const ms = Date.now() - started;
    if (!files.length) {
        fail('corpus', 'corpus is present', 'no .sz files found — the sweep proves nothing');
    } else if (!crashed && !unstable) {
        passed++;
        console.log(`corpus: ${files.length} .sz files, idempotent, no crashes, ${ms} ms`);
    }

    const szx = collect(path.join(ROOT, 'tests'), '.szx', []);
    for (const file of szx) {
        const src = fs.readFileSync(file, 'utf8');
        try {
            const once = formatter.formatSzx(src);
            if (formatter.formatSzx(once) !== once) {
                fail('corpus', path.relative(ROOT, file), 'szx not idempotent');
            }
        } catch (e) {
            fail('corpus', path.relative(ROOT, file), `szx threw: ${e && e.message}`);
        }
    }
    if (szx.length) {
        passed++;
        console.log(`corpus: ${szx.length} .szx files, idempotent`);
    }
}

// ── report ──────────────────────────────────────────────────────────────────

console.log('');
if (failures.length === 0) {
    console.log(`ok — ${passed} checks passed, ${cases.length} cases`);
    process.exit(0);
}
console.log(`FAILED — ${failures.length} failure(s), ${passed} passed\n`);
for (const f of failures) {
    console.log(`  [${f.suite}] ${f.name}`);
    console.log(`      ${f.detail}`);
}
process.exit(1);
