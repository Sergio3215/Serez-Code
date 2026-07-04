# Smoke test for sz-lsp: drives the real binary over stdio with an LSP
# session (initialize, didOpen with a broken file, completion, hover,
# definition, didChange fix, shutdown/exit) and checks each answer.
#
#   python tools/lsp_smoke.py [path-to-sz-lsp]
import json
import subprocess
import sys
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT = os.path.join(ROOT, "target", "debug", "sz-lsp.exe")
URI = "file:///E%3A/proyecto/app.sz"

BROKEN = "if (true {\n    out 1;\n}\n"
FIXED = "fn int suma(int a, int b) {\n    return a + b;\n}\nout suma(1, 2);\n"
# same file mid-typing: `File.` dangling (invalid, but completion must work)
TYPING = FIXED + "File.\n"


class Client:
    def __init__(self, path):
        self.proc = subprocess.Popen(
            [path], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL)
        self.next_id = 0

    def send(self, method, params, is_request=True):
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        if is_request:
            self.next_id += 1
            msg["id"] = self.next_id
        body = json.dumps(msg).encode("utf-8")
        self.proc.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.proc.stdin.flush()
        return msg.get("id")

    def read(self):
        headers = {}
        while True:
            line = self.proc.stdout.readline()
            if not line:
                return None
            line = line.strip()
            if not line:
                break
            k, _, v = line.partition(b":")
            headers[k.strip().lower()] = v.strip()
        length = int(headers[b"content-length"])
        return json.loads(self.proc.stdout.read(length))

    def wait_for(self, pred, what):
        for _ in range(20):
            msg = self.read()
            if msg is None:
                raise AssertionError("EOF waiting for " + what)
            if pred(msg):
                return msg
        raise AssertionError("never saw " + what)


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else DEFAULT
    c = Client(path)
    checks = []

    def check(name, cond):
        checks.append((name, bool(cond)))
        print(("PASS  " if cond else "FAIL  ") + name)

    rid = c.send("initialize", {"processId": None, "capabilities": {}})
    reply = c.wait_for(lambda m: m.get("id") == rid, "initialize reply")
    check("initialize: server info", reply["result"]["serverInfo"]["name"] == "sz-lsp")

    c.send("initialized", {}, is_request=False)
    c.send("textDocument/didOpen", {"textDocument": {
        "uri": URI, "languageId": "serez-code", "version": 1, "text": BROKEN}},
        is_request=False)
    diags = c.wait_for(lambda m: m.get("method") == "textDocument/publishDiagnostics",
                       "diagnostics")["params"]["diagnostics"]
    check("didOpen: broken file has diagnostics", len(diags) >= 1)
    check("didOpen: severity error", diags[0]["severity"] == 1)

    c.send("textDocument/didChange", {
        "textDocument": {"uri": URI, "version": 2},
        "contentChanges": [{"text": FIXED}]}, is_request=False)
    diags = c.wait_for(lambda m: m.get("method") == "textDocument/publishDiagnostics",
                       "diagnostics")["params"]["diagnostics"]
    check("didChange: fixed file has no diagnostics", len(diags) == 0)

    # user keeps typing: `File.` (dangling dot) — completion must still answer
    c.send("textDocument/didChange", {
        "textDocument": {"uri": URI, "version": 3},
        "contentChanges": [{"text": TYPING}]}, is_request=False)
    c.wait_for(lambda m: m.get("method") == "textDocument/publishDiagnostics",
               "diagnostics")
    rid = c.send("textDocument/completion", {
        "textDocument": {"uri": URI}, "position": {"line": 4, "character": 5}})
    items = c.wait_for(lambda m: m.get("id") == rid, "completion")["result"]
    labels = [i["label"] for i in items]
    check("completion: File. lists read/write", "read" in labels and "write" in labels)

    rid = c.send("textDocument/hover", {
        "textDocument": {"uri": URI}, "position": {"line": 3, "character": 5}})
    hover = c.wait_for(lambda m: m.get("id") == rid, "hover")["result"]
    check("hover: suma shows signature",
          "fn int suma(int a, int b)" in hover["contents"]["value"])

    rid = c.send("textDocument/definition", {
        "textDocument": {"uri": URI}, "position": {"line": 3, "character": 5}})
    defn = c.wait_for(lambda m: m.get("id") == rid, "definition")["result"]
    check("definition: suma points at line 0 char 7",
          defn["range"]["start"] == {"line": 0, "character": 7})

    rid = c.send("textDocument/documentSymbol", {"textDocument": {"uri": URI}})
    syms = c.wait_for(lambda m: m.get("id") == rid, "documentSymbol")["result"]
    check("documentSymbol: suma present", any(s["name"] == "suma" for s in syms))

    rid = c.send("shutdown", None)
    c.wait_for(lambda m: m.get("id") == rid, "shutdown reply")
    c.send("exit", None, is_request=False)
    code = c.proc.wait(timeout=10)
    check("exit: clean exit code 0", code == 0)

    failed = [n for n, ok in checks if not ok]
    print("\n%d/%d checks passed" % (len(checks) - len(failed), len(checks)))
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
