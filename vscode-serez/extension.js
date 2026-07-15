'use strict';

const vscode = require('vscode');
const { formatSz, formatSzx, formatSzs } = require('./formatter');

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
    // One formatter per language: .sz (brace indenter, original behavior),
    // .szx (braces + JSX tag depth), .szs (braces + /* */ comments).
    const formatters = [
        ['serez-code',     formatSz],
        ['serez-code-jsx', formatSzx],
        ['serez-style',    formatSzs],
    ];

    for (const [language, format] of formatters) {
        const provider = vscode.languages.registerDocumentFormattingEditProvider(
            { language, scheme: 'file' },
            {
                provideDocumentFormattingEdits(document) {
                    const original = document.getText();
                    const formatted = format(original);

                    if (formatted === original) return [];

                    const start = document.positionAt(0);
                    const end   = document.positionAt(original.length);
                    return [vscode.TextEdit.replace(new vscode.Range(start, end), formatted)];
                }
            });
        context.subscriptions.push(provider);
    }

    startLanguageServer(context);
}

function deactivate() {
    return client ? client.stop() : undefined;
}

module.exports = { activate, deactivate };
