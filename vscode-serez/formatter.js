'use strict';

// ---------------------------------------------------------------------------
// Pure formatting functions (no vscode dependency) — testable with plain node.
//
//   formatSz(text)   — .sz  : brace-count indenter (original behavior, intact)
//   formatSzx(text)  — .szx : braces + JSX tag depth (fragments, self-closing,
//                             multi-line attribute lists, dict annotations
//                             `<string, any>` and comparisons are NOT tags)
//   formatSzs(text)  — .szs : brace indenter aware of /* */ block comments;
//                             does NOT rewrite declarations (keeps one-line
//                             rules `sel { prop: v; }` exactly as written)
// ---------------------------------------------------------------------------

const TAB = '    '; // 4 spaces

// ── .sz — original brace counting (ignores strings and // comments) ─────────

function countBraces(line) {
    let opens = 0;
    let closes = 0;
    let inString = false;
    let escape = false;

    for (let i = 0; i < line.length; i++) {
        const c = line[i];

        if (inString) {
            if (escape) { escape = false; continue; }
            if (c === '\\') { escape = true; continue; }
            if (c === '"') inString = false;
            continue;
        }

        // Stop at line comment
        if (c === '/' && line[i + 1] === '/') break;

        if (c === '"') { inString = true; continue; }
        if (c === '{') opens++;
        else if (c === '}') closes++;
    }

    return { opens, closes };
}

function formatSz(text) {
    const lines = text.split(/\r?\n/);
    const out = [];
    let indent = 0;
    let prevBlank = false;

    for (const raw of lines) {
        const trimmed = raw.trim();

        // Collapse consecutive blank lines to one
        if (trimmed === '') {
            if (!prevBlank) out.push('');
            prevBlank = true;
            continue;
        }
        prevBlank = false;

        const { opens, closes } = countBraces(trimmed);
        const leadingClose = trimmed.startsWith('}');

        // If line starts with }, dedent before printing
        if (leadingClose && indent > 0) indent--;

        out.push(TAB.repeat(indent) + trimmed);

        // Calculate net indent change for the NEXT line.
        // leadingClose was already applied above, so add it back to the net
        // so it is not double-counted (e.g. "} else {" → net = 0 + 1 = 1).
        const net = opens - closes + (leadingClose ? 1 : 0);
        indent = Math.max(0, indent + net);
    }

    // Strip trailing blank lines, ensure file ends with a single newline
    while (out.length && out[out.length - 1] === '') out.pop();
    return out.join('\n') + '\n';
}

// ── .szx — braces + JSX tag depth ────────────────────────────────────────────
//
// One indent accumulator fed by two token kinds scanned in source order:
//   { / }                    → ±1 (plain Serez code, lambda bodies, JSX exprs)
//   <Tag …> / <>             → +1   </Tag> / </>  → −1   <Tag …/> → 0
// A `<` only starts a tag when the previous char is not alphanumeric and the
// next is a letter, `>` (fragment) or `/` (closer) — so `a < b` and the dict
// annotation `<string, any>` (name followed by `,`) are left alone.
// An open tag may span lines (`<Tabs` + one attr per line): state carries
// `inTag`; its head terminator is a `>` at attribute brace-depth 0 (so the
// `>` of `=>` inside an attribute expression never terminates the tag), with
// `/>` cancelling the indent (self-closing).
// The line prints at `indent + min(0, lowest running net on the line)` — the
// generalized version of the old "leading }" rule; it also covers `</div>`
// and `/>` lines starting with a dedent.

function isAlnum(c) {
    return /[A-Za-z0-9_]/.test(c);
}
function isAlpha(c) {
    return /[A-Za-z]/.test(c);
}

function formatSzx(text) {
    const lines = text.split(/\r?\n/);
    const out = [];
    let indent = 0;
    let prevBlank = false;

    // Cross-line scanner state (multi-line open tags)
    let inTag = false;        // inside `<Tag attr…` before its closing `>`
    let tagBraceDepth = 0;    // brace depth INSIDE the open tag's attributes

    for (const raw of lines) {
        const trimmed = raw.trim();

        if (trimmed === '') {
            if (!prevBlank) out.push('');
            prevBlank = true;
            continue;
        }
        prevBlank = false;

        let net = 0;      // running indent delta over this line
        let minNet = 0;   // lowest running delta (leading dedents)
        let inString = false;
        let escape = false;

        for (let i = 0; i < trimmed.length; i++) {
            const c = trimmed[i];

            if (inString) {
                if (escape) { escape = false; continue; }
                if (c === '\\') { escape = true; continue; }
                if (c === '"') inString = false;
                continue;
            }
            if (c === '/' && trimmed[i + 1] === '/' && !inTag) break; // line comment
            if (c === '"') { inString = true; continue; }

            if (c === '{') {
                net++;
                if (inTag) tagBraceDepth++;
                continue;
            }
            if (c === '}') {
                net--;
                if (net < minNet) minNet = net;
                if (inTag && tagBraceDepth > 0) tagBraceDepth--;
                continue;
            }

            if (inTag) {
                // Head terminator only at attribute brace-depth 0 (skips `=>`)
                if (c === '>' && tagBraceDepth === 0) {
                    if (trimmed[i - 1] === '/') {
                        net--; // self-closing: cancel the tag's +1
                        if (net < minNet) minNet = net;
                    }
                    inTag = false;
                }
                continue;
            }

            if (c === '<') {
                const prev = i > 0 ? trimmed[i - 1] : ' ';
                const next = trimmed[i + 1];
                if (next === undefined) continue;

                // Closing tag or fragment: </Tag> | </> — unambiguous, so it
                // counts even right after text (`Tareas</h1>`).
                if (next === '/') {
                    const after = trimmed[i + 2];
                    if (after === '>' || (after !== undefined && isAlpha(after))) {
                        net--;
                        if (net < minNet) minNet = net;
                        const gt = trimmed.indexOf('>', i + 2);
                        i = gt === -1 ? trimmed.length : gt;
                    }
                    continue;
                }
                // Openers are ambiguous after an identifier/number (`a<b`,
                // `i<n`): require a non-alphanumeric previous char.
                if (isAlnum(prev)) continue;
                // Fragment open: <>
                if (next === '>') {
                    net++;
                    i++;
                    continue;
                }
                // Candidate element: <Name …
                if (isAlpha(next)) {
                    let j = i + 1;
                    while (j < trimmed.length && /[A-Za-z0-9_-]/.test(trimmed[j])) j++;
                    const afterName = trimmed[j];
                    // Not JSX unless the name is followed by attrs, `>`, `/` or EOL
                    // (`<string, any>` — dict annotation — hits the `,` and is skipped)
                    if (afterName !== undefined && afterName !== '>' && afterName !== '/' &&
                        afterName !== ' ' && afterName !== '\t') {
                        i = j - 1;
                        continue;
                    }
                    net++; // tag opened (attrs/children indent)
                    if (afterName === undefined) {
                        // `<Tabs` and the attributes continue on the next lines
                        inTag = true;
                        tagBraceDepth = 0;
                        i = j - 1;
                        continue;
                    }
                    if (afterName === '>') {
                        i = j; // children follow; keep the +1
                        continue;
                    }
                    if (afterName === '/') {
                        // `<br/>`-style immediate self-close
                        if (trimmed[j + 1] === '>') {
                            net--;
                            i = j + 1;
                        }
                        continue;
                    }
                    // whitespace → attribute list (may end on this line or later)
                    inTag = true;
                    tagBraceDepth = 0;
                    i = j - 1;
                    continue;
                }
            }
        }

        const printIndent = Math.max(0, indent + minNet);
        out.push(TAB.repeat(printIndent) + trimmed);
        indent = Math.max(0, indent + net);
    }

    while (out.length && out[out.length - 1] === '') out.pop();
    return out.join('\n') + '\n';
}

// ── .szs — brace indenter aware of /* */ block comments ─────────────────────
// Indentation only: one-line rules (`sel { prop: v; }`) and the author's
// declaration style are preserved verbatim.

function formatSzs(text) {
    const lines = text.split(/\r?\n/);
    const out = [];
    let indent = 0;
    let prevBlank = false;
    let inComment = false; // /* … */ spanning lines

    for (const raw of lines) {
        const trimmed = raw.trim();

        if (trimmed === '') {
            if (!prevBlank) out.push('');
            prevBlank = true;
            continue;
        }
        prevBlank = false;

        let net = 0;
        let minNet = 0;
        let inString = false;

        for (let i = 0; i < trimmed.length; i++) {
            const c = trimmed[i];

            if (inComment) {
                if (c === '*' && trimmed[i + 1] === '/') { inComment = false; i++; }
                continue;
            }
            if (inString) {
                if (c === '"') inString = false;
                continue;
            }
            if (c === '/' && trimmed[i + 1] === '*') { inComment = true; i++; continue; }
            if (c === '/' && trimmed[i + 1] === '/') break;
            if (c === '"') { inString = true; continue; }

            if (c === '{') net++;
            else if (c === '}') {
                net--;
                if (net < minNet) minNet = net;
            }
        }

        const printIndent = Math.max(0, indent + minNet);
        out.push(TAB.repeat(printIndent) + trimmed);
        indent = Math.max(0, indent + net);
    }

    while (out.length && out[out.length - 1] === '') out.pop();
    return out.join('\n') + '\n';
}

module.exports = { formatSz, formatSzx, formatSzs, countBraces };
