#!/usr/bin/env python3
"""Exercise terminal cleanup in real PTYs using only Python's standard library."""
import errno
import fcntl
import json
import os
import pty
import select
import struct
import subprocess
import sys
import termios
import time


def fixture_binary():
    result = subprocess.run(
        ["cargo", "test", "--locked", "--test", "terminal", "--no-run", "--message-format=json"],
        stdout=subprocess.PIPE, text=True, check=False,
    )
    if result.returncode:
        print(result.stdout, file=sys.stderr)
        raise SystemExit(result.returncode)
    for line in result.stdout.splitlines():
        artifact = json.loads(line)
        if artifact.get("reason") == "compiler-artifact" and artifact["target"]["name"] == "terminal" and artifact.get("executable"):
            return artifact["executable"]
    raise RuntimeError("Cargo did not report the terminal fixture executable")


def check(binary, mode):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
    before = termios.tcgetattr(slave)

    def session():
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    process = None
    output = bytearray()
    try:
        process = subprocess.Popen(
            [binary, "--exact", "terminal_lifecycle_fixture", "--ignored", "--nocapture", "--skip", mode],
            stdin=slave, stdout=slave, stderr=slave, preexec_fn=session,
            env={**os.environ, "TERM": "xterm-256color"},
        )
        deadline = time.monotonic() + 10
        while True:
            if select.select([master], [], [], 0.05)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        break
                    raise
                if not chunk:
                    break
                output.extend(chunk)
            elif process.poll() is not None:
                break
            if time.monotonic() >= deadline:
                raise TimeoutError(f"terminal fixture hung: {mode}")
        assert process.wait(timeout=2) == 0, output.decode(errors="replace")
        assert termios.tcgetattr(slave) == before, f"terminal attributes changed: {mode}"
        assert output.count(b"\x1b[?1049l") == 1, f"expected exactly one alternate-screen restoration: {mode}"
        print(f"terminal PTY passed: {mode}")
    finally:
        if process is not None and process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)


if __name__ == "__main__":
    binary = fixture_binary()
    for mode in sys.argv[1:] or ["success", "io-error", "ui-panic", "worker-panic", "after-session-panic"]:
        check(binary, mode)
