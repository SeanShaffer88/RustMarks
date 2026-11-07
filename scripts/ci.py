#!/usr/bin/env python3
"""Run a build gate without masking its exit status; retain accessible diagnostics.

Usage: python scripts/ci.py test cargo test --all-targets
No shell is used. Full output goes to artifacts/<gate>.log and the console;
failed gates emit a bounded tail as a GitHub check annotation and step summary.
"""
from __future__ import annotations

import html
import os
from pathlib import Path
import re
import subprocess
import sys
from collections import deque


def escape_annotation(value: str) -> str:
    return value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def run(gate: str, command: list[str]) -> int:
    if not re.fullmatch(r"[a-z0-9_-]+", gate) or not command:
        raise ValueError("Provide a simple gate name and a command to execute")
    directory = Path("artifacts")
    directory.mkdir(exist_ok=True)
    tail: deque[str] = deque(maxlen=100)
    exit_code = 1
    # Cargo color is disabled so control codes cannot hide the diagnostic text.
    env = {**os.environ, "CARGO_TERM_COLOR": "never"}
    with (directory / f"{gate}.log").open("w", encoding="utf-8") as log:
        try:
            with subprocess.Popen(
                command,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                encoding="utf-8",
                errors="replace",
                env=env,
            ) as process:
                assert process.stdout is not None
                for line in process.stdout:
                    log.write(line)
                    # Never let child output issue workflow commands.
                    if os.environ.get("GITHUB_ACTIONS") == "true" and line.startswith("::"):
                        print("[output] " + line, end="", flush=True)
                    else:
                        print(line, end="", flush=True)
                    tail.append(line.rstrip("\r\n"))
                exit_code = process.wait()
        except OSError as error:
            message = f"Unable to start {command[0]}: {error}"
            log.write(message + "\n")
            tail.append(message)
            print(message, file=sys.stderr)

    # A negative subprocess code represents a signal and is still a failure.
    if exit_code < 0:
        exit_code = 1
    status = "passed" if exit_code == 0 else f"failed (exit {exit_code})"
    excerpt = "\n".join(tail)[-12000:]
    if exit_code and os.environ.get("GITHUB_ACTIONS") == "true":
        message = f"{gate} {status}\nLast output lines:\n{excerpt}"
        print(f"::error title={gate} failed::{escape_annotation(message)}", flush=True)
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with Path(summary).open("a", encoding="utf-8") as output:
            output.write(f"\n## {gate}: {status}\n")
            output.write(f"<details><summary>Last output lines</summary><pre>{html.escape(excerpt)}</pre></details>\n")
            output.write(f"\nFull output: CI diagnostics artifact, `{gate}.log`.\n")
    return exit_code


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    try:
        return run(sys.argv[1], sys.argv[2:])
    except ValueError as error:
        print(error, file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
