"""End-to-end check of RobotCode as the extension runs it.

Usage: smoke.py PYTHON LAUNCHER WORKDIR

Starts `PYTHON LAUNCHER language-server --stdio` (and `debug-launch --stdio`)
the way the extension does, in WORKDIR, and checks that:
  - Robocop diagnostics are published for a document with language ID
    "robotframework" (the ID the extension maps Robot files to),
  - formatting returns Robocop's formatted document,
  - hover resolves a keyword from the standard library,
  - every token type in languages/robot/semantic_token_rules.json is one
    RobotCode still sends,
  - the debugger stops at a breakpoint and reports variables,
  - with Python debugging on (as the extension configures it), a debugpy
    client can attach and stop at a breakpoint inside a Python keyword.
"""

import json
import os
import queue
import re
import socket
import subprocess
import sys
import threading
import time

PYTHON, LAUNCHER, WORKDIR = sys.argv[1:4]
TIMEOUT = 120
RULES = os.path.join(os.path.dirname(__file__), "..", "..", "languages", "robot", "semantic_token_rules.json")

SUITE = """*** Test Cases ***
my test
    log   hello
    ${x}=    Set Variable    42
    Log    value ${x}
"""


class Peer:
    """Minimal LSP/DAP peer (both use Content-Length framing), over the stdio
    of a process it starts, or over an existing socket."""

    def __init__(self, args=None, sock=None):
        self.proc = None
        if sock is not None:
            stream = sock.makefile("rwb")
            self.reader, self.writer = stream, stream
        else:
            self.proc = subprocess.Popen(
                [PYTHON, *args],
                cwd=WORKDIR,
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=open(os.path.join(WORKDIR, f"{args[1]}.stderr.log"), "w"),
            )
            self.reader, self.writer = self.proc.stdout, self.proc.stdin
        self.messages = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        while True:
            headers = {}
            while True:
                try:
                    line = self.reader.readline()
                except OSError:
                    return
                if not line:
                    return
                if line == b"\r\n":
                    break
                key, value = line.decode().split(":", 1)
                headers[key.strip()] = value.strip()
            self.messages.put(json.loads(self.reader.read(int(headers["Content-Length"]))))

    def send(self, message):
        body = json.dumps(message).encode()
        self.writer.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.writer.flush()

    def next(self, deadline):
        try:
            return self.messages.get(timeout=max(0.1, deadline - time.time()))
        except queue.Empty:
            return None

    def close(self):
        if self.proc:
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
                         "textDocument": {"hover": {"contentFormat": ["markdown"]},
                                          "semanticTokens": {"requests": {"full": True}, "tokenTypes": [],
                                                             "tokenModifiers": [], "formats": ["relative"]}}}}})

    diagnostics, formatted, hover, legend = None, None, None, None
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
            legend = m["result"]["capabilities"].get("semanticTokensProvider", {}).get("legend", {})
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

    with open(RULES) as f:
        rules = json.loads(re.sub(r"^\s*//.*$", "", f.read(), flags=re.M))
    types = set((legend or {}).get("tokenTypes", []))
    modifiers = set((legend or {}).get("tokenModifiers", []))
    unknown = sorted({r["token_type"] for r in rules} - types)
    unknown += sorted({m for r in rules for m in r.get("token_modifiers", [])} - modifiers)
    check("semantic token rules match RobotCode's token types", types and not unknown, f"unknown: {unknown}")


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


def debugger_python():
    """Python debugging the way the extension sets it up: attachPython plus
    waiting for the Python debugger, which here is a debugpy client like
    Zed's "Debugpy" attach configuration."""
    with open(os.path.join(WORKDIR, "PyLib.py"), "w") as f:
        f.write("def compute_value(x):\n    doubled = int(x) * 2\n    return doubled\n")
    path = os.path.join(WORKDIR, "pylib.robot")
    with open(path, "w") as f:
        f.write("*** Settings ***\nLibrary    PyLib.py\n\n*** Test Cases ***\nT\n"
                "    ${v}=    Compute Value    21\n    Log    ${v}\n")
    dap = Peer(["-u", LAUNCHER, "debug-launch", "--stdio"])
    seq = iter(range(1, 1000))

    def request(peer, command, arguments=None):
        peer.send({"seq": next(seq), "type": "request", "command": command, "arguments": arguments or {}})

    request(dap, "initialize", {"adapterID": "RobotCode", "clientID": "zed", "linesStartAt1": True,
                                "columnsStartAt1": True, "pathFormat": "path"})
    request(dap, "launch", {"request": "launch", "cwd": WORKDIR, "target": path, "console": "internalConsole",
                            "python": PYTHON, "attachPython": True, "attachPythonPort": 5678,
                            "debuggerTimeout": 120, "debuggerArgs": ["--debugpy-wait-for-client"]})

    py, py_stop, py_locals, terminated = None, None, {}, False
    deadline = time.time() + TIMEOUT
    while time.time() < deadline and not terminated:
        m = dap.next(time.time() + 0.2) if py else dap.next(deadline)
        if m is not None:
            if m.get("event") == "initialized":
                request(dap, "configurationDone")
            elif m.get("event") == "debugpyStarted":
                sock = socket.create_connection(("127.0.0.1", m["body"]["port"]), timeout=10)
                sock.settimeout(None)
                py = Peer(sock=sock)
                request(py, "initialize", {"adapterID": "debugpy", "clientID": "zed", "linesStartAt1": True,
                                           "columnsStartAt1": True, "pathFormat": "path"})
                request(py, "attach", {"justMyCode": False})
            elif m.get("event") == "terminated":
                terminated = True
        n = py.next(time.time() + 0.2) if py else None
        if n is None:
            continue
        if n.get("event") == "initialized":
            request(py, "setBreakpoints", {"source": {"path": os.path.join(WORKDIR, "PyLib.py")},
                                           "breakpoints": [{"line": 3}]})
            request(py, "configurationDone")
        elif n.get("event") == "stopped":
            thread = n["body"]["threadId"]
            request(py, "stackTrace", {"threadId": thread})
        elif n.get("command") == "stackTrace":
            frame = n["body"]["stackFrames"][0]
            py_stop = (os.path.basename(frame["source"]["path"]), frame["line"])
            request(py, "scopes", {"frameId": frame["id"]})
        elif n.get("command") == "scopes":
            request(py, "variables", {"variablesReference": n["body"]["scopes"][0]["variablesReference"]})
        elif n.get("command") == "variables":
            py_locals = {v["name"]: v["value"] for v in n["body"]["variables"]}
            request(py, "continue", {"threadId": thread})
    dap.close()

    check("Python debugger stops inside a Python keyword", py_stop == ("PyLib.py", 3), f"stopped at {py_stop}")
    check("Python debugger shows the keyword's locals", py_locals.get("doubled") == "42", str(py_locals)[:120])
    check("debug run with Python debugging finishes", terminated)


language_server()
debugger()
debugger_python()
sys.exit(1 if failures else 0)
