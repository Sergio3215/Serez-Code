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
    return {
        mode: CODE,
        depth: 0,
        group: 0,
        /** Open delimiters, innermost last: '{', '(' or '['. */
        stack: [],
        /** A closer that did not match its opener, or closed nothing. */
        mismatch: null,
        /** The last code character seen, for continuation classification. */
        lastCode: '',
        /** The one before it, so two-character operators can be recognised. */
        prevCode: '',
    };
}

/** The opener each closer must match. */
const OPENER_OF = { '}': '{', ')': '(', ']': '[' };

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
        if (c === '{' || c === '(' || c === '[') {
            state.stack.push(c);
            if (c !== '{') state.group++;
            // Reported for every delimiter, not only braces: the caller samples
            // the stack depth here, and a `)` that is never reported leaves a
            // line printing at the level it was about to leave.
            if (onDelta) onDelta(1);
            remember(state, c);
            i++;
            continue;
        }
        if (c === '}' || c === ')' || c === ']') {
            // A closer that matches nothing, or matches the wrong opener, means
            // the document contradicts itself. Recorded rather than guessed
            // around: see DEC-FMT-002.
            const top = state.stack[state.stack.length - 1];
            if (top === undefined || top !== OPENER_OF[c]) {
                if (!state.mismatch) {
                    state.mismatch = top === undefined
                        ? `a '${c}' closes nothing`
                        : `a '${top}' is closed by a '${c}'`;
                }
            } else {
                state.stack.pop();
            }
            if (c !== '}' && state.group > 0) state.group--;
            if (onDelta) onDelta(-1);
            remember(state, c);
            i++;
            continue;
        }
        if (c !== ' ' && c !== '\t') remember(state, c);
        if (onChar) {
            const jump = onChar(i, c, state);
            if (typeof jump === 'number' && jump > i) { i = jump + 1; continue; }
        }
        i++;
    }
}

function isIdentChar(c) {
    return c !== undefined && /[A-Za-z0-9_]/.test(c);
}

/** Record a code character, keeping the previous one for two-char operators. */
function remember(state, c) {
    state.prevCode = state.lastCode;
    state.lastCode = c;
}

// ── continuation classification ─────────────────────────────────────────────
//
// A line continues the expression above it when either end of the join says so,
// and both ends are read from tokens rather than guessed from a symbol:
//
//   * the PREVIOUS line ends with something that cannot end a statement — an
//     operator, an `=`, a comma. Serez does not require semicolons (`let a = 1`
//     then `out a` is two statements), so this is the only reliable signal that
//     the previous line is unfinished;
//   * this line STARTS with a token that cannot begin a statement.
//
// `+` and `-` are the interesting case. They are both prefix and infix, and the
// symbol alone cannot say which. The previous token can: `let a = 1` followed by
// `+ 2;` parses as `1 + 2` — measured, it prints 3 — so a leading `+` after a
// token that ends an expression is a continuation. After a `;` or a `}` it
// starts a statement.
//
// `!` is prefix only, so it is never a continuation on its own account; it can
// still be one when the previous line ended open (`let x =` / `!flag;`).

/** Tokens that cannot start a statement, longest first. */
const INFIX_STARTS = [
    '|>', '&&', '||', '??', '==', '!=', '<=', '>=', '=>', '::',
    '.', ',', '?', ':', '*', '%',
];

/**
 * Trailing characters that leave an expression unfinished.
 *
 * `<` and `>` are deliberately absent. A line ending in `>` is overwhelmingly a
 * JSX tag, not a dangling comparison, and treating it as unfinished indented
 * every child of every element one level too deep in `.szx`.
 */
const OPEN_TAIL = new Set(['=', '+', '-', '*', '/', '%', ',', '&', '|', '^',
    '?', ':', '!', '~', '.']);

/** Trailing characters that complete an expression. */
function endsExpression(c) {
    return c !== '' && (isIdentChar(c) || c === '"' || c === ')' || c === ']');
}

/**
 * Does `trimmed` continue the expression the scanner has been reading?
 *
 * `state` must be the state *before* the line is scanned.
 */
function continuesExpression(state, trimmed) {
    // A comment is never an operator, whatever it starts with.
    if (trimmed.startsWith('//') || trimmed.startsWith('/*')) return false;

    // The previous line ended mid-expression.
    if (OPEN_TAIL.has(state.lastCode)) return true;

    // An infix token only continues something if there IS something: the
    // previous line has to have ended with an expression. Without that check a
    // CSS selector `.a {` reads as a continuation of whatever preceded it, and
    // `.szs` indented every rule in the file.
    if (!endsExpression(state.lastCode)) return false;

    for (const token of INFIX_STARTS) {
        if (trimmed.startsWith(token)) return true;
    }

    // Ambiguous prefix/infix: decided by what came before, not by the symbol.
    // `let a = 1` then `+ 2;` parses as `1 + 2` — measured, it prints 3.
    if (trimmed.startsWith('+') || trimmed.startsWith('-') || trimmed.startsWith('/')) {
        return true;
    }

    return false;
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

/**
 * How many open delimiters this line closes before it says anything else.
 *
 * `}`, `)`, `]` and a JSX `</Tag>` all count; anything else stops the run.
 */
function leadingClosers(trimmed) {
    let closed = 0;
    for (let i = 0; i < trimmed.length; i++) {
        const c = trimmed[i];
        if (c === ' ' || c === '	') continue;
        if (c === '}' || c === ')' || c === ']') { closed++; continue; }
        if (c === '<' && trimmed[i + 1] === '/') {
            const gt = trimmed.indexOf('>', i + 2);
            if (gt === -1) break;
            closed++;
            i = gt;
            continue;
        }
        break;
    }
    return closed;
}

// ── indentation levels ──────────────────────────────────────────────────────
//
// One level per *line* that left a delimiter open, not one per delimiter. That
// distinction is the whole model:
//
//     foo({            <- opens two delimiters, one level
//         a: 1         <- level 1, not 2
//     })
//
// `Levels` holds the delimiter depth at which each open line started. A line
// prints at the number of entries still standing once its own leading closers
// have popped, which is the generalised form of "a line starting with `}`
// dedents" and works identically for `)` and `]`.

function newLevels() {
    return [];
}

/**
 * The level this line prints at, after popping what its leading closers close.
 * `depths` is mutated: the pops are real, because the closers really closed.
 */
function levelFor(levels, minDepth) {
    while (levels.length && minDepth < levels[levels.length - 1]) levels.pop();
    return levels.length;
}

/** Record that this line left something open. */
function pushLevel(levels, endDepth, minDepth) {
    if (endDepth > minDepth && (!levels.length || endDepth > levels[levels.length - 1])) {
        levels.push(endDepth);
    }
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

/**
 * `.sz` — indent by delimiter level, with active continuation indentation.
 *
 * `report`, when given, receives `{ uncertain, reason }` if the document
 * contradicts itself — see DEC-FMT-002 and `indentDocument`.
 */
function formatSz(text, options, report) {
    return indentDocument(text, options, report, null);
}

/**
 * The shared line loop.
 *
 * `tagScanner`, when given, is called for every character in code position and
 * may push or pop `state.stack` itself — that is how `.szx` counts JSX tags on
 * the same stack as `{`, `(` and `[`, so one level model covers both.
 *
 * # DEC-FMT-002
 *
 * If the scanner meets a closer that matches nothing, or the wrong opener, the
 * document contradicts itself and its structure cannot be determined. The
 * original text is returned untouched and `report.uncertain` is set, so the
 * editor can say a syntax error is likely. An *incomplete* document — an
 * unclosed `(`, an open block, an unterminated string — is not that: it is
 * determinable and is formatted normally, without a warning.
 */
function indentDocument(text, options, report, tagScanner) {
    const eol = dominantEol(text);
    const unit = indentUnit(options);
    const lines = text.split(/\r?\n/);
    const out = [];
    const state = initialState();
    const levels = newLevels();
    let prevBlank = false;

    for (const raw of lines) {
        // Checked BEFORE scanning: the scan may close the string on this very
        // line, and by then `state.mode` no longer says how the line *began*.
        if (isLiteral(state)) {
            scanLine(raw, state, null, tagScanner ? tagScanner(raw) : null);
            out.push(raw); // byte-for-byte: in a string, spaces are content
            prevBlank = false;
            continue;
        }

        const trimmed = raw.trim();

        if (trimmed === '') {
            if (!prevBlank) out.push('');
            prevBlank = true;
            continue;
        }
        prevBlank = false;

        // Decided before the line is scanned: it is about the join between this
        // line and the one above it.
        // Only outside a group: inside `(` or `[` the level already comes from
        // the open delimiter, and adding a bonus for the trailing comma of the
        // line above would indent each argument one deeper than the last.
        const closers = leadingClosers(trimmed);
        // A line that starts by closing something is never the middle of an
        // expression, whatever the line above it dangled. Without this,
        // `return a +` followed by `}` pushed the brace in a level — caught by
        // the differential run over the fixture corpus, not by a unit case.
        const continuation = closers === 0 && state.group === 0 &&
            continuesExpression(state, trimmed);

        // A line prints at the level its LEADING closers leave it at. Only
        // leading ones: `, b);` closes a group at its end and still belongs to
        // that group, while `)` on its own line does not. Taking the minimum
        // over the whole line would dedent the first of those.
        const minDepth = Math.max(0, state.stack.length - closers);
        const onChar = tagScanner ? tagScanner(trimmed, null) : null;
        scanLine(trimmed, state, null, onChar);

        const level = levelFor(levels, minDepth);
        const printLevel = level + (continuation ? 1 : 0);

        const body = emitBody(raw, trimmed, state);
        out.push(printLevel > 0 ? unit.repeat(printLevel) + body : body);

        pushLevel(levels, state.stack.length, minDepth);
    }

    while (out.length && out[out.length - 1] === '') out.pop();

    if (state.mismatch) {
        if (report) {
            report.uncertain = true;
            report.reason = state.mismatch;
        }
        // Nothing is rewritten: the structure the indentation would come from is
        // the thing that is contradictory.
        return text;
    }

    // No trailing newline, ever.
    //
    // The formatter's output ends at the last character of the last line:
    // `out 1;`, `out 1;\n` and `out 1;\r\n` all come back as `out 1;`. Trailing
    // blank lines were already dropped above; this drops the last line break
    // too, so "how the document ends" has one answer instead of depending on
    // what the document happened to arrive as.
    //
    // This is about the *end* of the document only. Line endings *inside* it
    // are untouched — `eol` is still the document's dominant ending, so a CRLF
    // file stays CRLF and format-on-save does not rewrite every line.
    //
    // A document returned early under DEC-FMT-002 keeps whatever it had,
    // including its final newline: that path does not format at all, and
    // stripping a byte off a document the formatter just declined to read
    // would be the one edit it is not entitled to make.
    return out.join(eol);
}

// ── .szx — braces, groups and JSX tag depth on one stack ────────────────────
//
// `<Tag …>` / `<>` push, `</Tag>` / `</>` and `… />` pop, on the SAME stack the
// delimiters use, so a JSX tree and a block indent by the same rule.
//
// A `<` only starts a tag when the previous char is not alphanumeric and the
// next is a letter, `>` (fragment) or `/` — so `a < b` and the dict annotation
// `<string, any>` (name followed by `,`) are left alone. An open tag may span
// lines: `inTag` carries across, and its head ends at a `>` outside any brace,
// so the `>` of `=>` inside an attribute never terminates it.

function isAlnum(c) {
    return /[A-Za-z0-9_]/.test(c);
}
function isAlpha(c) {
    return /[A-Za-z]/.test(c);
}

function jsxScanner() {
    let inTag = false;
    // The delimiter depth the open tag started at. A `>` only ends the tag head
    // when the stack is back to it, so the `>` of an `=>` inside
    // `onChange={(v) => …}` does not terminate the tag.
    //
    // This reads the shared stack rather than counting braces privately, because
    // `scanLine` consumes `{` and `}` as delimiters before `onChar` ever sees
    // them: a private counter stayed at zero, every attribute arrow closed its
    // tag early, and the orphaned tag was later \closed\ by the enclosing `)`.
    // That produced a spurious mismatch on 103 of 347 real .szx files.
    let tagDepth = 0;

    return function forLine(trimmed, sample) {
        return function onChar(i, c, state) {
            if (inTag) {
                if (c === '>' && state.stack.length === tagDepth) {
                    if (trimmed[i - 1] === '/') {
                        pop(state, sample); // self-closing: cancel the tag's push
                    }
                    inTag = false;
                }
                return;
            }

            if (c !== '<') return;
            const prev = i > 0 ? trimmed[i - 1] : ' ';
            const next = trimmed[i + 1];
            if (next === undefined) return;

            if (next === '/') {
                const after = trimmed[i + 2];
                if (after === '>' || (after !== undefined && isAlpha(after))) {
                    pop(state, sample);
                    const gt = trimmed.indexOf('>', i + 2);
                    return gt === -1 ? trimmed.length : gt;
                }
                return;
            }
            if (isAlnum(prev)) return;
            if (next === '>') { push(state, sample); return i + 1; }

            if (isAlpha(next)) {
                let j = i + 1;
                while (j < trimmed.length && /[A-Za-z0-9_-]/.test(trimmed[j])) j++;
                const afterName = trimmed[j];
                if (afterName !== undefined && afterName !== '>' && afterName !== '/' &&
                    afterName !== ' ' && afterName !== '\t') {
                    return j - 1; // `<string, any>` and friends: not a tag
                }
                push(state, sample);
                if (afterName === undefined) { inTag = true; tagDepth = state.stack.length; return j - 1; }
                if (afterName === '>') return j;
                if (afterName === '/') {
                    if (trimmed[j + 1] === '>') { pop(state, sample); return j + 1; }
                    return;
                }
                inTag = true;
                tagDepth = state.stack.length;
                return j - 1;
            }
        };
    };
}

/** A tag counts as a delimiter, so it shares the level model. */
function push(state, sample) {
    state.stack.push('<');
    if (sample) sample();
}
function pop(state, sample) {
    if (state.stack.length && state.stack[state.stack.length - 1] === '<') state.stack.pop();
    if (sample) sample();
}

function formatSzx(text, options, report) {
    return indentDocument(text, options, report, jsxScanner());
}

// ── .szs — the same model; declarations are never rewritten ────────────────

function formatSzs(text, options, report) {
    return indentDocument(text, options, report, null);
}


module.exports = {
    formatSz, formatSzx, formatSzs, countBraces,
    scanLine, initialState, isLiteral, continuesExpression,
    dominantEol, indentUnit,
};
