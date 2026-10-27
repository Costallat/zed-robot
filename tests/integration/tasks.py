"""Runs the tasks from languages/robot/tasks.json the way Zed does (through
the shell, with Zed's task variables filled in) and checks:

  - the run button's task runs exactly the test it is next to, also when the
    name contains Robot Framework pattern characters,
  - `robot: open log.html` finds the log in robot.toml's output-dir.

    tasks.py PYTHON ROBOTCODE_LAUNCHER WORKDIR

`robotcode` on the PATH runs ROBOTCODE_LAUNCHER with PYTHON, like a project
with RobotCode installed. With `-` as ROBOTCODE_LAUNCHER, the `robotcode` and
`python` already on the PATH are used (an activated virtualenv with
`pip install robotframework "robotcode[runner]"`), which also works on
Windows.

Tasks run through `sh -c` (Zed on Linux and macOS runs them in the user's
shell), or on Windows through PowerShell: TASK_SHELL=powershell (Windows
PowerShell 5.1) or pwsh (PowerShell 7, the default). Exits non-zero if a
check fails.
"""

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

PYTHON, LAUNCHER, WORKDIR = sys.argv[1:4]
ROOT = Path(__file__).resolve().parents[2]
failures = 0


def check(name, ok, detail=""):
    global failures
    print(f"{'ok  ' if ok else 'FAIL'}  {name}" + ("" if ok else f": {detail}"))
    failures += not ok


def load_tasks():
    text = (ROOT / "languages" / "robot" / "tasks.json").read_text(encoding="utf-8")
    text = re.sub(r"(?m)^\s*//.*$", "", text)
    return {task["label"].split(" $")[0]: task for task in json.loads(text)}


def run(task, variables, env):
    """Runs a task like Zed: the command and its arguments go to the shell."""
    def fill(text):
        for name, value in variables.items():
            text = text.replace(f"${name}", value)
        return text

    command = " ".join([task["command"], *(fill(arg) for arg in task["args"])])
    if os.name == "nt":
        shell = [os.environ.get("TASK_SHELL", "pwsh"), "-NoProfile", "-NonInteractive", "-Command", command]
    else:
        shell = ["sh", "-c", command]
    return subprocess.run(shell, cwd=fill(task["cwd"]), env=env,
                          capture_output=True, text=True, timeout=180)


project = Path(WORKDIR) / "tasks-project"
shutil.rmtree(project, ignore_errors=True)
(project / "bin").mkdir(parents=True)

if LAUNCHER != "-":
    # `robotcode` and `python`, as an activated virtualenv provides them.
    robotcode = project / "bin" / "robotcode"
    robotcode.write_text(f'#!/bin/sh\nexec "{PYTHON}" "{LAUNCHER}" "$@"\n')
    robotcode.chmod(0o755)
    os.symlink(PYTHON, project / "bin" / "python")

# Stands in for the browser: prints the URL it is asked to open.
printer = project / "bin" / "print_url.py"
printer.write_text("import sys\nprint('BROWSER', sys.argv[1])\n")

(project / "robot.toml").write_text('output-dir = "results"\n')
suite = project / "my.suite.robot"
suite.write_text(
    "*** Test Cases ***\n"
    "Login [smoke] works\n    Log    one\n"
    "Login * other\n    Log    two\n"
    "Login and other\n    Log    matched by the pattern `Login * other`\n"
    "Name.with.dots\n    Log    three\n"
)

env = dict(os.environ)
env["PATH"] = f"{project / 'bin'}{os.pathsep}{env['PATH']}"
# Keep a user-level robot.toml out of the way, and print instead of opening a browser.
env["HOME"] = env["XDG_CONFIG_HOME"] = str(project / "home")
# Forward slashes: webbrowser splits BROWSER like a POSIX shell, also on Windows.
env["BROWSER"] = f"{Path(PYTHON).as_posix()} {printer.as_posix()} %s"

tasks = load_tasks()
variables = {"ZED_FILE": str(suite), "ZED_WORKTREE_ROOT": str(project)}

for name in ["Login [smoke] works", "Login * other", "Name.with.dots"]:
    result = run(tasks["robot: test"], {**variables, "ZED_CUSTOM_robot_test_name": name}, env)
    output = result.stdout + result.stderr
    ran = re.findall(r"^(.+?)\s+\| PASS \|$", output, re.MULTILINE)
    check(f"run button runs only `{name}`", result.returncode == 0 and name in ran and len(ran) == 2,
          output[-600:])

result = run(tasks["robot: open log.html"], variables, env)
expected = (project / "results" / "log.html").as_uri()
check("`robot: open log.html` opens the log in output-dir", f"BROWSER {expected}" in result.stdout,
      result.stdout + result.stderr)

sys.exit(1 if failures else 0)
