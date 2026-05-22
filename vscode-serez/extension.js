'use strict';

const vscode = require('vscode');

// ---------------------------------------------------------------------------
// Brace counting — ignores content inside string literals and line comments
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Formatter
// ---------------------------------------------------------------------------

function formatDocument(text) {
    const lines = text.split(/\r?\n/);
    const out = [];
    const TAB = '    '; // 4 spaces
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

// ---------------------------------------------------------------------------
// Extension entry points
// ---------------------------------------------------------------------------

function activate(context) {
    const selector = { language: 'serez', scheme: 'file' };

    const provider = vscode.languages.registerDocumentFormattingEditProvider(selector, {
        provideDocumentFormattingEdits(document) {
            const original = document.getText();
            const formatted = formatDocument(original);

            if (formatted === original) return [];

            const start = document.positionAt(0);
            const end   = document.positionAt(original.length);
            return [vscode.TextEdit.replace(new vscode.Range(start, end), formatted)];
        }
    });

    context.subscriptions.push(provider);
}

function deactivate() {}

module.exports = { activate, deactivate };
