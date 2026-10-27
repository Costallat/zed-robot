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
  - completion items look the way src/labels.rs expects (kinds, labels, and
    the library in a keyword's detail),
  - the sample files in tests/fixtures, formatted by Robocop, are written to
    WORKDIR/formatted/ (integration-test.sh checks that they still parse),
  - the debugger, given the arguments the run button's task turns into, runs
    exactly that test,
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
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
RULES = os.path.join(ROOT, "languages", "robot", "semantic_token_rules.json")
FIXTURES = os.path.join(ROOT, "tests", "fixtures")
# LSP CompletionItemKind values.
FUNCTION, VARIABLE, MODULE = 3, 6, 9

COMPLETE = """*** Settings ***
Library    Collections

*** Test Cases ***
T
    ${local}=    Set Variable    1
    Lo
    Log    ${l
"""

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


def apply_edits(text, edits):
    """Applies LSP text edits to `text`."""
    lines = text.split("\n")
    offsets = [0]
    for line in lines:
        offsets.append(offsets[-1] + len(line) + 1)

    def offset(pos):
        return offsets[min(pos["line"], len(lines))] + pos["character"]

    for edit in sorted(edits, key=lambda e: offset(e["range"]["start"]), reverse=True):
        start, end = offset(edit["range"]["start"]), offset(edit["range"]["end"])
        text = text[:start] + edit["newText"] + text[end:]
    return text


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

    complete_uri = "file://" + os.path.join(WORKDIR, "complete.robot")
    with open(os.path.join(WORKDIR, "complete.robot"), "w") as f:
        f.write(COMPLETE)
    fixtures = {}
    for name in sorted(os.listdir(FIXTURES)):
        with open(os.path.join(FIXTURES, name), encoding="utf-8") as f:
            fixtures["file://" + os.path.join(WORKDIR, "fixtures", name)] = f.read()
    os.makedirs(os.path.join(WORKDIR, "fixtures"), exist_ok=True)
    os.makedirs(os.path.join(WORKDIR, "formatted"), exist_ok=True)
    for fixture_uri, text in fixtures.items():
        with open(fixture_uri[len("file://"):], "w", encoding="utf-8") as f:
            f.write(text)
    formatting_ids = {100 + i: u for i, u in enumerate(fixtures)}
    fixture_results = {}

    diagnostics, formatted, hover, legend = None, None, None, None
    keywords, variables = None, None
    deadline = time.time() + TIMEOUT
    while time.time() < deadline and (None in (diagnostics, formatted, hover, keywords, variables)
                                      or len(fixture_results) < len(fixtures)):
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
            lsp.send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {
                "uri": complete_uri, "languageId": "robotframework", "version": 1, "text": COMPLETE}}})
            # After `    Lo`: keywords and libraries; after `${l`: variables.
            lsp.send({"jsonrpc": "2.0", "id": 4, "method": "textDocument/completion", "params": {
                "textDocument": {"uri": complete_uri}, "position": {"line": 6, "character": 6}}})
            lsp.send({"jsonrpc": "2.0", "id": 5, "method": "textDocument/completion", "params": {
                "textDocument": {"uri": complete_uri}, "position": {"line": 7, "character": 14}}})
            for request_id, fixture_uri in formatting_ids.items():
                lsp.send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {
                    "uri": fixture_uri, "languageId": "robotframework", "version": 1,
                    "text": fixtures[fixture_uri]}}})
                lsp.send({"jsonrpc": "2.0", "id": request_id, "method": "textDocument/formatting", "params": {
                    "textDocument": {"uri": fixture_uri}, "options": {"tabSize": 4, "insertSpaces": True}}})
        elif m.get("id") == 2:
            formatted = m.get("result") or []
        elif m.get("id") == 3:
            hover = m.get("result") or {}
        elif m.get("id") in (4, 5):
            result = m.get("result") or []
            items = result.get("items", []) if isinstance(result, dict) else result
            if m["id"] == 4:
                keywords = items
            else:
                variables = items
        elif m.get("id") in formatting_ids:
            fixture_uri = formatting_ids[m["id"]]
            fixture_results[fixture_uri] = apply_edits(fixtures[fixture_uri], m.get("result") or [])
        elif m.get("method") == "textDocument/publishDiagnostics" and m["params"]["diagnostics"]:
            diagnostics = m["params"]["diagnostics"]
    lsp.close()

    sources = {d.get("source") for d in diagnostics or []}
    check("language server publishes Robocop diagnostics", "robocop" in sources, f"sources={sources}")
    new_text = formatted[0]["newText"] if formatted else ""
    check("formatting uses Robocop", "    log    hello" in new_text, repr(new_text[:120]))
    contents = json.dumps(hover)
    check("hover documents `Set Variable`", "Set Variable" in contents, contents[:120])

    def find(items, label):
        return next((i for i in items or [] if i.get("label") == label), None)

    log = find(keywords, "Log")
    check("keyword completions are functions with their library in the detail",
          log is not None and log.get("kind") == FUNCTION and re.fullmatch(r"Keyword \(BuiltIn\)\s*", log.get("detail", "")),
          json.dumps(log)[:200])
    library = find(keywords, "Collections")
    check("library completions are modules", library is not None and library.get("kind") == MODULE,
          json.dumps(library)[:200])
    local = find(variables, "${local}")
    check("variable completions are variables labelled ${name}",
          local is not None and local.get("kind") == VARIABLE,
          json.dumps([i.get("label") for i in variables or []])[:200])

    for fixture_uri, text in fixture_results.items():
        with open(os.path.join(WORKDIR, "formatted", os.path.basename(fixture_uri)), "w", encoding="utf-8") as f:
            f.write(text)
    check("Robocop formats every sample file", len(fixture_results) == len(fixtures),
          f"formatted {sorted(fixture_results)}")

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


def debugger_single_test():
    """A run button's task started under the debugger: the arguments are the
    ones src/debugger.rs makes from tasks.json's `robot: test` (see the
    run_button_tasks_can_be_debugged test there)."""
    path = os.path.join(WORKDIR, "single.robot")
    with open(path, "w") as f:
        f.write("*** Test Cases ***\nLogin [smoke] works\n    Log    one\n"
                "Login s works\n    Log    matched by the unescaped pattern\n"
                "Other\n    Log    two\n")
    out = os.path.join(WORKDIR, "single-results")
    dap = Peer(["-u", LAUNCHER, "debug-launch", "--stdio"])
    seq = iter(range(1, 1000))

    def request(command, arguments=None):
        dap.send({"seq": next(seq), "type": "request", "command": command, "arguments": arguments or {}})

    request("initialize", {"adapterID": "RobotCode", "clientID": "zed", "linesStartAt1": True,
                           "columnsStartAt1": True, "pathFormat": "path"})
    request("launch", {"request": "launch", "name": "single", "cwd": WORKDIR, "console": "internalConsole",
                       "python": PYTHON, "outputDir": out,
                       "args": ["--name", "Robot", "--test", "Robot.Login [[]smoke] works", path]})
    terminated = False
    deadline = time.time() + TIMEOUT
    while time.time() < deadline and not terminated:
        m = dap.next(deadline)
        if m is None:
            break
        if m.get("event") == "initialized":
            request("configurationDone")
        elif m.get("event") == "terminated":
            terminated = True
    dap.close()

    ran = []
    try:
        with open(os.path.join(out, "output.xml"), encoding="utf-8") as f:
            ran = re.findall(r'<test id="[^"]*" name="([^"]*)"', f.read())
    except OSError as err:
        ran = [str(err)]
    check("debugging the run button's task runs exactly that test", ran == ["Login [smoke] works"], str(ran))


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
debugger_single_test()
debugger_python()
sys.exit(1 if failures else 0)
