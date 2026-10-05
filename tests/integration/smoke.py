"""End-to-end check of RobotCode as the extension runs it.

Usage: smoke.py PYTHON LAUNCHER WORKDIR

Starts `PYTHON LAUNCHER language-server --stdio` (and `debug-launch --stdio`)
the way the extension does, in WORKDIR, and checks that:
  - Robocop diagnostics are published for a document with language ID
    "robotframework" (the ID the extension maps Robot files to),
  - formatting returns Robocop's formatted document,
  - hover resolves a keyword from the standard library,
  - the debugger stops at a breakpoint and reports variables.
"""

import json
import os
import queue
import subprocess
import sys
import threading
import time

PYTHON, LAUNCHER, WORKDIR = sys.argv[1:4]
TIMEOUT = 120

SUITE = """*** Test Cases ***
my test
    log   hello
    ${x}=    Set Variable    42
    Log    value ${x}
"""


class Peer:
    """Minimal LSP/DAP peer over stdio (both use Content-Length framing)."""

    def __init__(self, args):
        self.proc = subprocess.Popen(
            [PYTHON, *args],
            cwd=WORKDIR,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=open(os.path.join(WORKDIR, f"{args[1]}.stderr.log"), "w"),
        )
        self.messages = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        while True:
            headers = {}
            while True:
                line = self.proc.stdout.readline()
                if not line:
                    return
                if line == b"\r\n":
                    break
                key, value = line.decode().split(":", 1)
                headers[key.strip()] = value.strip()
            self.messages.put(json.loads(self.proc.stdout.read(int(headers["Content-Length"]))))

    def send(self, message):
        body = json.dumps(message).encode()
        self.proc.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.proc.stdin.flush()

    def next(self, deadline):
        try:
            return self.messages.get(timeout=max(0.1, deadline - time.time()))
        except queue.Empty:
            return None

    def close(self):
        self.proc.kill()


failures = []


def check(name, ok, detail=""):
    print(f"{'ok  ' if ok else 'FAIL'}  {name}{f': {detail}' if detail and not ok else ''}")
    if not ok:
        failures.append(name)


def language_server():
    path = os.path.join(WORKDIR, "lsp.robot")
    with open(path, "w") as f:
        f.write(SUITE)
    uri = "file://" + path
    root = "file://" + WORKDIR
    lsp = Peer([LAUNCHER, "language-server", "--stdio"])
    lsp.send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "processId": None, "rootUri": root,
        "workspaceFolders": [{"uri": root, "name": "smoke"}],
        "capabilities": {"workspace": {"configuration": True},
                         "textDocument": {"hover": {"contentFormat": ["markdown"]}}}}})

    diagnostics, formatted, hover = None, None, None
    deadline = time.time() + TIMEOUT
    while time.time() < deadline and None in (diagnostics, formatted, hover):
        m = lsp.next(deadline)
        if m is None:
            break
        if m.get("method") == "workspace/configuration":
            lsp.send({"jsonrpc": "2.0", "id": m["id"], "result": [{} for _ in m["params"]["items"]]})
        elif "method" in m and "id" in m:
            lsp.send({"jsonrpc": "2.0", "id": m["id"], "result": None})
        elif m.get("id") == 1:
            lsp.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
            lsp.send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {
                "uri": uri, "languageId": "robotframework", "version": 1, "text": SUITE}}})
            lsp.send({"jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting", "params": {
                "textDocument": {"uri": uri}, "options": {"tabSize": 4, "insertSpaces": True}}})
            lsp.send({"jsonrpc": "2.0", "id": 3, "method": "textDocument/hover", "params": {
                "textDocument": {"uri": uri}, "position": {"line": 3, "character": 18}}})
        elif m.get("id") == 2:
            formatted = m.get("result") or []
        elif m.get("id") == 3:
            hover = m.get("result") or {}
        elif m.get("method") == "textDocument/publishDiagnostics" and m["params"]["diagnostics"]:
            diagnostics = m["params"]["diagnostics"]
    lsp.close()

    sources = {d.get("source") for d in diagnostics or []}
    check("language server publishes Robocop diagnostics", "robocop" in sources, f"sources={sources}")
    new_text = formatted[0]["newText"] if formatted else ""
    check("formatting uses Robocop", "    log    hello" in new_text, repr(new_text[:120]))
    contents = json.dumps(hover)
    check("hover documents `Set Variable`", "Set Variable" in contents, contents[:120])


def debugger():
    path = os.path.join(WORKDIR, "dap.robot")
    with open(path, "w") as f:
        f.write(SUITE)
    dap = Peer(["-u", LAUNCHER, "debug-launch", "--stdio"])
    seq = iter(range(1, 1000))

    def request(command, arguments=None):
        dap.send({"seq": next(seq), "type": "request", "command": command, "arguments": arguments or {}})

    request("initialize", {"adapterID": "RobotCode", "clientID": "zed", "clientName": "Zed",
                           "linesStartAt1": True, "columnsStartAt1": True, "pathFormat": "path"})
    # What the extension sends for a run button: robot arguments plus defaults.
    request("launch", {"request": "launch", "name": "smoke", "cwd": WORKDIR, "args": [path],
                       "console": "internalConsole", "outputMessages": True, "outputLog": True,
                       "python": PYTHON})

    stopped_line, variables, terminated = None, None, False
    deadline = time.time() + TIMEOUT
    while time.time() < deadline and not terminated:
        m = dap.next(deadline)
        if m is None:
            break
        kind, event, command = m.get("type"), m.get("event"), m.get("command")
        if kind == "event" and event == "initialized":
            request("setBreakpoints", {"source": {"path": path}, "breakpoints": [{"line": 5}]})
            request("configurationDone")
        elif kind == "event" and event == "stopped":
            request("stackTrace", {"threadId": m["body"].get("threadId", 1)})
        elif kind == "response" and command == "stackTrace":
            frame = m["body"]["stackFrames"][0]
            stopped_line = frame["line"]
            request("scopes", {"frameId": frame["id"]})
        elif kind == "response" and command == "scopes":
            request("variables", {"variablesReference": m["body"]["scopes"][0]["variablesReference"]})
        elif kind == "response" and command == "variables":
            variables = {v["name"]: v["value"] for v in m["body"]["variables"]}
            request("continue", {"threadId": 1})
        elif kind == "event" and event == "terminated":
            terminated = True
    dap.close()

    check("debugger stops at the breakpoint", stopped_line == 5, f"stopped at {stopped_line}")
    check("debugger shows variables", "42" in str((variables or {}).get("${x}")), str(variables)[:120])
    check("debug run finishes", terminated)


language_server()
debugger()
sys.exit(1 if failures else 0)
