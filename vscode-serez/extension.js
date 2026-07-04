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
// Language server (sz-lsp): diagnostics, completion, hover, go-to-definition
// ---------------------------------------------------------------------------

let client = null;

function startLanguageServer(context) {
    // Restricted Mode (untrusted workspace): never spawn a configurable binary
    // on untrusted folder content. Highlighting and the formatter keep working
    // (declared via capabilities.untrustedWorkspaces in package.json); the LSP
    // starts automatically if the user later trusts the workspace.
    if (vscode.workspace.isTrusted === false) {
        const sub = vscode.workspace.onDidGrantWorkspaceTrust?.(() => {
            sub?.dispose();
            startLanguageServer(context);
        });
        if (sub) context.subscriptions.push(sub);
        return;
    }
    const config = vscode.workspace.getConfiguration('serez');
    if (!config.get('lsp.enabled', true)) return;

    let LanguageClient, TransportKind;
    try {
        ({ LanguageClient, TransportKind } = require('vscode-languageclient/node'));
    } catch (e) {
        console.warn('serez: vscode-languageclient not bundled, LSP disabled', e);
        return;
    }

    // serez.lsp.path setting, or `sz-lsp` from PATH (installed next to `sz`)
    const command = config.get('lsp.path', '') || 'sz-lsp';

    const serverOptions = {
        run:   { command, transport: TransportKind.stdio },
        debug: { command, transport: TransportKind.stdio },
    };
    const clientOptions = {
        documentSelector: [
            { language: 'serez-code', scheme: 'file' },
            { language: 'serez-code-jsx', scheme: 'file' },
        ],
    };

    client = new LanguageClient('serez-lsp', 'Serez-Code Language Server',
        serverOptions, clientOptions);
    client.start().catch(() => {
        vscode.window.showWarningMessage(
            'Serez-Code: no se pudo iniciar sz-lsp. Instala el binario junto a sz ' +
            'o configura "serez.lsp.path". (Diagnósticos/autocompletado desactivados; ' +
            'el resaltado y el formatter siguen funcionando.)');
        client = null;
    });
}

// ---------------------------------------------------------------------------
// Extension entry points
// ---------------------------------------------------------------------------

function activate(context) {
    const selector = { language: 'serez-code', scheme: 'file' };

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

    startLanguageServer(context);
}

function deactivate() {
    return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
