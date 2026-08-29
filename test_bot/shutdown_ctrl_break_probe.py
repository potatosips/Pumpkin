"""Exercise Pumpkin's Windows console shutdown handler without killing its parent shell."""

from __future__ import annotations

import argparse
import os
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path


def wait_for_port(port: int, process: subprocess.Popen[str], timeout: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Pumpkin exited before listening (code {process.returncode})")
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return
        except OSError:
            time.sleep(0.1)
    raise TimeoutError(f"Pumpkin did not listen on port {port} within {timeout}s")


def run_once(executable: Path, workdir: Path, port: int, timeout: float, trigger: str) -> tuple[bool, str]:
    process = subprocess.Popen(
        [str(executable)],
        cwd=workdir,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
    )
    try:
        wait_for_port(port, process, timeout)
        if trigger == "command":
            assert process.stdin is not None
            process.stdin.write("stop\n")
            process.stdin.flush()
        elif trigger == "ctrl-c":
            os.kill(process.pid, signal.CTRL_C_EVENT)
        else:
            os.kill(process.pid, signal.CTRL_BREAK_EVENT)
        try:
            output, _ = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            process.kill()
            tail, _ = process.communicate(timeout=5)
            captured = (error.output or "") + (tail or "")
            return False, f"timeout>{timeout}s\n{captured}"
    except BaseException:
        process.kill()
        process.wait(timeout=5)
        raise

    clean = process.returncode == 0 and "overflowed its stack" not in output
    return clean, f"exit={process.returncode} overflow={'overflowed its stack' in output}\n{output}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--port", type=int, default=25565)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--trigger", choices=("command", "ctrl-c", "ctrl-break"), default="command")
    parser.add_argument(
        "--executable",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "pumpkin" / "target" / "debug" / "pumpkin.exe",
    )
    args = parser.parse_args()
    executable = args.executable.resolve()
    workdir = executable.parents[2]

    passed = 0
    for index in range(1, args.runs + 1):
        clean, detail = run_once(executable, workdir, args.port, args.timeout, args.trigger)
        print(f"RUN {index}/{args.runs}: {'PASS' if clean else 'FAIL'} {detail.splitlines()[0]}")
        if not clean:
            print(detail)
        passed += int(clean)
    print(f"WINDOWS_{args.trigger.upper().replace('-', '_')}_CLEAN_SHUTDOWN={passed}/{args.runs}")
    return 0 if passed == args.runs else 1


if __name__ == "__main__":
    sys.exit(main())
