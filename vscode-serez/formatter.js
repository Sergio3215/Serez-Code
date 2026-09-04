'use strict';

// ---------------------------------------------------------------------------
// Pure formatting functions (no vscode dependency) — testable with plain node.
//
//   formatSz(text)   — .sz  : brace indenter
//   formatSzx(text)  — .szx : braces + JSX tag depth (fragments, self-closing,
//                             multi-line attribute lists, dict annotations
//                             `<string, any>` and comparisons are NOT tags)
//   formatSzs(text)  — .szs : brace indenter for style sheets; does NOT rewrite
//                             declarations (keeps one-line rules exactly as
//                             written)
//
// All three re-indent whole lines. That is only safe if the formatter knows
// which characters are *code*, so the lexical scanner below is shared: getting
// this wrong does not produce ugly output, it produces different programs.
// ---------------------------------------------------------------------------

const TAB = '    '; // 4 spaces — the default; callers may pass their own

// ── lexical state, carried across lines ─────────────────────────────────────
//
// Measured against `src/lexer.rs`, not assumed from another language. Serez has:
//
//   //  line comment                      to end of line
//   /*  block comment  */                 spans lines, does NOT nest
//   "…" string                            spans lines, has escapes, and `{…}`
//                                         interpolation whose blocks may contain
//                                         further "…" strings
//   r"…" raw string                       spans lines, NO escapes — the first
//                                         `"` closes it, so `r"C:\dir\"` is the
//                                         four characters `C:\dir\`
//
// Every one of those was mishandled before, and each mishandling changed a
// program rather than its appearance:
//
//   `/* } */` inside a block              dedented the following code
//   `/* { */` inside a block              indented it and moved the closing brace
//   a multi-line string                   had its continuation lines trimmed and
//                                         re-indented — literal content edited
//   `r"a\"`                               swallowed the rest of the line
//
// The last one matters most by frequency: 175 of the 493 corpus `.sz` files use
// `r"…"`.

const CODE = 0;
const BLOCK_COMMENT = 1;
const STRING = 2;
const RAW_STRING = 3;

/**
 * The state a line begins in, plus what a scan of it produced.
 * @typedef {{ mode: number, depth: number }} ScanState
 *   mode  — CODE | BLOCK_COMMENT | STRING | RAW_STRING
 *   depth — interpolation brace depth inside a STRING, so that a `"` inside
 *           `"{ f("x") }"` opens a nested string instead of closing the outer
 *           one. That is what `Lexer::read_string` does.
 */

/** A fresh scanner state: code, no open construct. */
function initialState() {
    return { mode: CODE, depth: 0, group: 0 };
}

/**
 * Is a line that begins in `state` *literal text* — a multi-line string, raw
 * string or block comment?
 *
 * Such a line is emitted byte-for-byte, trailing whitespace included: inside a
 * string those spaces are content. Measured before this existed:
 * `"line one<newline>line two"` came back with four spaces inside it, and the
 * program printed them.
 */
function isLiteral(state) {
    return state.mode !== CODE;
}

/**
 * Is a line the middle of an expression rather than the start of a statement?
 *
 * Two signals, neither of them a guess:
 *
 *   1. `state.group > 0` — a `(` or `[` opened on an earlier line is still
 *      open, so this line is inside an argument list, an array literal or a
 *      parenthesised expression;
 *   2. the line begins with a token that **cannot start a statement** — a
 *      member access `.`, or an unambiguously infix operator. A line starting
 *      with `&&`, `|>` or `,` is always the continuation of the expression
 *      above it. That is a fact about the grammar, not a formatting guess.
 *
 * `+`, `-` and `!` are deliberately **not** in that set: they are also prefix
 * operators, so `-x;` is a statement on its own and the formatter cannot tell
 * the two apart without a parser. Excluding them costs a little — a line
 * starting with `+` is still re-indented — and including them would risk
 * leaving a real statement at whatever column it happened to have. `<` and `>`
 * are excluded for the same reason in `.szx`, where they may open a tag.
 *
 * Both keep the indentation the author gave them. The formatter indents
 * **blocks**; it does not reflow expressions, and pretending otherwise did this
 * to 40 of the 493 corpus files:
 *
 *     let r = nums          let r = nums
 *         .filter(f)    ->  .filter(f)
 *         .map(g);          .map(g);
 *
 * and would have rewritten **327 of the 347** real `.szx` files in the
 * ecosystem, every one of them by pulling its JSX one level left, because the
 * tree sits inside `return (`.
 *
 * Whether continuations should instead be *actively* re-indented is a real
 * product choice with more than one defensible answer — DEC-FMT-001. Leaving
 * them alone is the option that cannot damage correct code.
 */

/**
 * Line-leading tokens that cannot begin a statement, longest first so that `|>`
 * is tested before `|` would be and `==` before `=`.
 */
const INFIX_STARTS = [
    '|>', '&&', '||', '??', '==', '!=', '<=', '>=', '=>', '::',
    '.', ',', '?', ':', '*', '/', '%',
];

function continuesExpression(state, trimmed) {
    if (state.group > 0) return true;
    for (const token of INFIX_STARTS) {
        if (trimmed.startsWith(token)) return true;
    }
    return false;
}

/**
 * Scan one line, updating `state` in place, and report its brace movement.
 *
 * `onDelta(delta)` is called for each `{` (+1) and `}` (−1) found **in code**,
 * in source order, so a caller can track both the net change and the lowest
 * point reached — the `.szx` and `.szs` formatters need the latter and `.sz`
 * does not, and neither needs a second scanner to get it.
 *
 * `onChar(index, char)` is called for every character in code position, which
 * is how the `.szx` formatter layers JSX tag counting on top without
 * re-implementing string and comment handling.
 */
function scanLine(line, state, onDelta, onChar) {
    let i = 0;

    while (i < line.length) {
        const c = line[i];

        if (state.mode === BLOCK_COMMENT) {
            // Not nested: the first `*/` closes it, which is what
            // `Lexer::skip_block_comment` does.
            if (c === '*' && line[i + 1] === '/') {
                state.mode = CODE;
                i += 2;
                continue;
            }
            i++;
            continue;
        }

        if (state.mode === RAW_STRING) {
            // No escapes at all: the first `"` closes it.
            if (c === '"') {
                state.mode = CODE;
            }
            i++;
            continue;
        }

        if (state.mode === STRING) {
            if (c === '\\') {
                i += 2; // the escaped character, whatever it is
                continue;
            }
            if (c === '{') {
                state.depth++;
                i++;
                continue;
            }
            if (c === '}' && state.depth > 0) {
                state.depth--;
                i++;
                continue;
            }
            if (c === '"') {
                if (state.depth > 0) {
                    // A string inside an interpolation block. The lexer consumes
                    // it wholesale; so does this, without leaving the outer
                    // string.
                    i++;
                    while (i < line.length) {
                        if (line[i] === '\\' && line[i + 1] === '"') { i += 2; continue; }
                        if (line[i] === '"') { i++; break; }
                        i++;
                    }
                    continue;
                }
                state.mode = CODE;
                i++;
                continue;
            }
            i++;
            continue;
        }

        // ── code ────────────────────────────────────────────────────────────
        if (c === '/' && line[i + 1] === '/') {
            return; // line comment: nothing after it is code
        }
        if (c === '/' && line[i + 1] === '*') {
            state.mode = BLOCK_COMMENT;
            i += 2;
            continue;
        }
        if (c === 'r' && line[i + 1] === '"' && !isIdentChar(line[i - 1])) {
            // `r"` only opens a raw string when the `r` is not the tail of an
            // identifier — `result`, `range` and `for` are not raw strings.
            state.mode = RAW_STRING;
            i += 2;
            continue;
        }
        if (c === '"') {
            state.mode = STRING;
            state.depth = 0;
            i++;
            continue;
        }
        if (c === '{') {
            if (onDelta) onDelta(1);
            i++;
            continue;
        }
        if (c === '}') {
            if (onDelta) onDelta(-1);
            i++;
            continue;
        }
        if (c === '(' || c === '[') {
            state.group++;
            i++;
            continue;
        }
        if (c === ')' || c === ']') {
            if (state.group > 0) state.group--;
            i++;
            continue;
        }
        if (onChar) {
            const jump = onChar(i, c);
            if (typeof jump === 'number' && jump > i) { i = jump + 1; continue; }
        }
        i++;
    }
}

function isIdentChar(c) {
    return c !== undefined && /[A-Za-z0-9_]/.test(c);
}

/**
 * The text to emit for a code line, given where the scan of it ended.
 *
 * Leading whitespace always goes — that is what re-indenting means. Trailing
 * whitespace goes too, *unless* the line ended inside a string or a block
 * comment, because then those spaces are inside the literal. `let s = "a   `
 * opening a multi-line string is a code line whose trailing spaces are content.
 */
function emitBody(raw, trimmed, state) {
    return state.mode === CODE ? trimmed : raw.trimStart();
}

// ── line endings and indent unit ─────────────────────────────────────────────

/**
 * The dominant line ending, so format-on-save does not rewrite every line of a
 * CRLF file. Splitting on /\r?\n/ and joining with '\n' turned every save of a
 * CRLF document into a whole-file diff.
 */
function dominantEol(text) {
    let crlf = 0;
    let lf = 0;
    for (let i = 0; i < text.length; i++) {
        if (text[i] === '\n') {
            if (i > 0 && text[i - 1] === '\r') crlf++;
            else lf++;
        }
    }
    return crlf > lf ? '\r\n' : '\n';
}

/** The indent unit for `options`, defaulting to the previous hard-coded 4 spaces. */
function indentUnit(options) {
    if (!options) return TAB;
    if (options.insertSpaces === false) return '\t';
    const size = Number(options.tabSize);
    return ' '.repeat(Number.isFinite(size) && size > 0 ? Math.floor(size) : 4);
}

// ── .sz — brace indenter over the shared scanner ────────────────────────────

/**
 * Count `{` and `}` in code position on one line, from a clean state.
 *
 * Kept because it is exported and tested; the formatters themselves use
 * `scanLine` directly so that state carries across lines, which is the thing a
 * per-line count cannot do.
 */
function countBraces(line) {
    let opens = 0;
    let closes = 0;
    scanLine(line, initialState(), d => { if (d > 0) opens++; else closes++; });
    return { opens, closes };
}

function formatSz(text, options) {
    const eol = dominantEol(text);
    const unit = indentUnit(options);
    const lines = text.split(/\r?\n/);
    const out = [];
    let indent = 0;
    let prevBlank = false;
    const state = initialState();

    for (const raw of lines) {
        // Checked BEFORE scanning: the scan may close the string on this
        // very line, and by then `state.mode` no longer says how the line
        // *began*.
        const literal = isLiteral(state);
        if (literal) {
            scanLine(raw, state, delta => {
                indent = Math.max(0, indent + delta);
            });
            out.push(raw); // byte-for-byte: in a string, spaces are content
            prevBlank = false;
            continue;
        }

        const trimmed = raw.trim();

        if (trimmed !== '' && continuesExpression(state, trimmed)) {
            scanLine(trimmed, state, delta => {
                indent = Math.max(0, indent + delta);
            });
            // The author's indentation, minus trailing whitespace, which is
            // invisible and never meaningful in code position.
            out.push(raw.replace(/[ \t]+$/, ''));
            prevBlank = false;
            continue;
        }

        // Collapse consecutive blank lines to one
        if (trimmed === '') {
            if (!prevBlank) out.push('');
            prevBlank = true;
            continue;
        }
        prevBlank = false;

        let opens = 0;
        let closes = 0;
        scanLine(trimmed, state, d => { if (d > 0) opens++; else closes++; });
        const leadingClose = trimmed.startsWith('}');

        // If line starts with }, dedent before printing
        if (leadingClose && indent > 0) indent--;

        const body = emitBody(raw, trimmed, state);
        out.push(indent > 0 ? unit.repeat(indent) + body : body);

        // Calculate net indent change for the NEXT line.
        // leadingClose was already applied above, so add it back to the net
        // so it is not double-counted (e.g. "} else {" → net = 0 + 1 = 1).
        const net = opens - closes + (leadingClose ? 1 : 0);
        indent = Math.max(0, indent + net);
    }

    // Strip trailing blank lines, ensure file ends with a single newline
    while (out.length && out[out.length - 1] === '') out.pop();
    return out.join(eol) + eol;
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

function formatSzx(text, options) {
    const eol = dominantEol(text);
    const unit = indentUnit(options);
    const lines = text.split(/\r?\n/);
    const out = [];
    let indent = 0;
    let prevBlank = false;

    // Cross-line lexical state (strings, raw strings, block comments) and the
    // cross-line JSX state (an open tag whose attributes span lines).
    const state = initialState();
    let inTag = false;        // inside `<Tag attr…` before its closing `>`
    let tagBraceDepth = 0;    // brace depth INSIDE the open tag's attributes

    for (const raw of lines) {
        // Checked BEFORE scanning: the scan may close the string on this
        // very line, and by then `state.mode` no longer says how the line
        // *began*.
        const literal = isLiteral(state);
        if (literal) {
            scanLine(raw, state, delta => {
                indent = Math.max(0, indent + delta);
            });
            out.push(raw); // byte-for-byte: in a string, spaces are content
            prevBlank = false;
            continue;
        }

        const trimmed = raw.trim();

        if (trimmed !== '' && continuesExpression(state, trimmed)) {
            scanLine(trimmed, state, delta => {
                indent = Math.max(0, indent + delta);
            });
            // The author's indentation, minus trailing whitespace, which is
            // invisible and never meaningful in code position.
            out.push(raw.replace(/[ \t]+$/, ''));
            prevBlank = false;
            continue;
        }

        if (trimmed === '') {
            if (!prevBlank) out.push('');
            prevBlank = true;
            continue;
        }
        prevBlank = false;

        let net = 0;      // running indent delta over this line
        let minNet = 0;   // lowest running delta (leading dedents)

        scanLine(
            trimmed,
            state,
            delta => {
                net += delta;
                if (delta < 0 && net < minNet) minNet = net;
                if (inTag) {
                    if (delta > 0) tagBraceDepth++;
                    else if (tagBraceDepth > 0) tagBraceDepth--;
                }
            },
            (i, c) => {
                if (inTag) {
                    // Head terminator only at attribute brace-depth 0 (skips `=>`)
                    if (c === '>' && tagBraceDepth === 0) {
                        if (trimmed[i - 1] === '/') {
                            net--; // self-closing: cancel the tag's +1
                            if (net < minNet) minNet = net;
                        }
                        inTag = false;
                    }
                    return;
                }

                if (c !== '<') return;

                const prev = i > 0 ? trimmed[i - 1] : ' ';
                const next = trimmed[i + 1];
                if (next === undefined) return;

                // Closing tag or fragment: </Tag> | </> — unambiguous, so it
                // counts even right after text (`Tareas</h1>`).
                if (next === '/') {
                    const after = trimmed[i + 2];
                    if (after === '>' || (after !== undefined && isAlpha(after))) {
                        net--;
                        if (net < minNet) minNet = net;
                        const gt = trimmed.indexOf('>', i + 2);
                        return gt === -1 ? trimmed.length : gt;
                    }
                    return;
                }
                // Openers are ambiguous after an identifier/number (`a<b`,
                // `i<n`): require a non-alphanumeric previous char.
                if (isAlnum(prev)) return;
                // Fragment open: <>
                if (next === '>') {
                    net++;
                    return i + 1;
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
                        return j - 1;
                    }
                    net++; // tag opened (attrs/children indent)
                    if (afterName === undefined) {
                        // `<Tabs` and the attributes continue on the next lines
                        inTag = true;
                        tagBraceDepth = 0;
                        return j - 1;
                    }
                    if (afterName === '>') {
                        return j; // children follow; keep the +1
                    }
                    if (afterName === '/') {
                        // `<br/>`-style immediate self-close
                        if (trimmed[j + 1] === '>') {
                            net--;
                            return j + 1;
                        }
                        return;
                    }
                    // whitespace → attribute list (may end on this line or later)
                    inTag = true;
                    tagBraceDepth = 0;
                    return j - 1;
                }
            }
        );

        const printIndent = Math.max(0, indent + minNet);
        const body = emitBody(raw, trimmed, state);
        out.push(printIndent > 0 ? unit.repeat(printIndent) + body : body);
        indent = Math.max(0, indent + net);
    }

    while (out.length && out[out.length - 1] === '') out.pop();
    return out.join(eol) + eol;
}

// ── .szs — brace indenter aware of /* */ block comments ─────────────────────
// Indentation only: one-line rules (`sel { prop: v; }`) and the author's
// declaration style are preserved verbatim.

function formatSzs(text, options) {
    const eol = dominantEol(text);
    const unit = indentUnit(options);
    const lines = text.split(/\r?\n/);
    const out = [];
    let indent = 0;
    let prevBlank = false;
    const state = initialState();

    for (const raw of lines) {
        // Checked BEFORE scanning: the scan may close the string on this
        // very line, and by then `state.mode` no longer says how the line
        // *began*.
        const literal = isLiteral(state);
        if (literal) {
            scanLine(raw, state, delta => {
                indent = Math.max(0, indent + delta);
            });
            out.push(raw); // byte-for-byte: in a string, spaces are content
            prevBlank = false;
            continue;
        }

        const trimmed = raw.trim();

        if (trimmed !== '' && continuesExpression(state, trimmed)) {
            scanLine(trimmed, state, delta => {
                indent = Math.max(0, indent + delta);
            });
            // The author's indentation, minus trailing whitespace, which is
            // invisible and never meaningful in code position.
            out.push(raw.replace(/[ \t]+$/, ''));
            prevBlank = false;
            continue;
        }

        if (trimmed === '') {
            if (!prevBlank) out.push('');
            prevBlank = true;
            continue;
        }
        prevBlank = false;

        let net = 0;
        let minNet = 0;
        scanLine(trimmed, state, delta => {
            net += delta;
            if (delta < 0 && net < minNet) minNet = net;
        });

        const printIndent = Math.max(0, indent + minNet);
        const body = emitBody(raw, trimmed, state);
        out.push(printIndent > 0 ? unit.repeat(printIndent) + body : body);
        indent = Math.max(0, indent + net);
    }

    while (out.length && out[out.length - 1] === '') out.pop();
    return out.join(eol) + eol;
}

module.exports = {
    formatSz, formatSzx, formatSzs, countBraces,
    scanLine, initialState, isLiteral, continuesExpression, dominantEol, indentUnit,
};
