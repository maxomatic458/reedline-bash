"""An interactive bash on a PTY, with a terminal that answers back."""

import fcntl
import os
import pty
import select
import shlex
import signal
import struct
import subprocess
import termios
import threading
import time

from .vt import VT

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = os.path.dirname(HERE)
LIB = os.path.join(CRATE, "target", "debug", "libreedline_bash.so")

PROMPT = "RL$ "
CONTINUATION = "..> "

PRELUDE = """
HISTSIZE=1000
HISTFILESIZE=1000
shopt -s expand_aliases
"""


class Timeout(Exception):
    pass


class Skipped(Exception):
    """Raised when a test cannot run here, so it is not reported as passing."""


def clean_env(home, **extra):
    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": home,
        "TERM": "xterm-256color",
        "LC_ALL": "C.UTF-8",
        "REEDLINE_BASH_CONFIG": os.path.join(home, "absent.toml"),
        "HISTFILE": os.path.join(home, "history"),
    }
    env.update(extra)
    return env


class Shell:
    def __init__(
        self,
        rc="",
        config="",
        mode=None,
        rows=24,
        cols=80,
        term="xterm-256color",
        env=None,
        cwd=None,
        attach=True,
        workdir=None,
        ps1=PROMPT,
        ps2=CONTINUATION,
        marker=None,
    ):
        from .runner import scratch

        self.vt = VT(rows, cols)
        self.dir = workdir or scratch()
        self._lock = threading.Lock()
        self._closed = False
        self._raw = bytearray()
        self._reports_at_send = 0
        self._raw_at_send = 0
        self.attached = bool(attach)
        # What a prompt looks like once drawn.
        self.marker = (marker if marker is not None else ps1).rstrip()
        self.continuation = ps2.rstrip()
        # How many rows a drawn prompt occupies.
        self.prompt_height = 1
        self._output = []

        if mode and "mode" not in config:
            config = f'[editor]\nmode = "{mode}"\n' + config

        rcfile = os.path.join(self.dir, "rcfile")
        with open(rcfile, "w") as fh:
            fh.write(f"PS1={shlex.quote(ps1)}\nPS2={shlex.quote(ps2)}\n")
            fh.write(PRELUDE)
            if attach == "nested":
                # What VS Code does: its own init file sources your ~/.bashrc,
                # so the enable happens one startup file deeper.
                inner = os.path.join(self.dir, "bashrc")
                with open(inner, "w") as bashrc:
                    bashrc.write(f"enable -f {LIB} reedline\n")
                fh.write(f"source {inner}\n")
            elif attach:
                fh.write(f"enable -f {LIB} reedline\n")
            fh.write(rc if rc.endswith("\n") or not rc else rc + "\n")

        home = cwd or self.dir
        full_env = clean_env(home, TERM=term)
        if config:
            path = os.path.join(self.dir, "config.toml")
            with open(path, "w") as fh:
                fh.write(config)
            full_env["REEDLINE_BASH_CONFIG"] = path
        if env:
            full_env.update(env)
        self.env = full_env

        pid, master = pty.fork()
        if pid == 0:
            # Before the exec, so nothing ever sees a 0x0 terminal.
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
            if cwd:
                os.chdir(cwd)

            os.execvpe("bash", ["bash", "--noprofile", "--rcfile", rcfile, "-i"], full_env)
            os._exit(127)

        self.pid = pid
        self.master = master
        self._reader = threading.Thread(target=self._pump, daemon=True)
        self._reader.start()

    def send(self, text):
        """Write keystrokes without waiting for anything."""
        with self._lock:
            self._reports_at_send = self.vt.cursor_reports
            self._raw_at_send = len(self._raw)
        os.write(self.master, text.encode())

    def press(self, *keys, settle=True):
        for key in keys:
            self.send(key)
            if settle:
                self.settle()
        return self

    def type(self, text, settle=True):
        self.send(text)
        if settle:
            self.settle()
        return self

    def submit(self, timeout=10.0):
        """Press Enter, wait for the prompt, and record what the command printed."""
        start = self._row_index()
        self.send("\r")
        self.wait_prompt(timeout=timeout)
        end = self._row_index()
        rows = self.rows
        self._output = [
            row for row in rows[start + 1 : end - self.prompt_height + 1] if row.strip()
        ]
        return self

    def run(self, command, timeout=10.0):
        """Type a command, run it, and return the lines it printed.

        Only its output: the echoed command line and the prompt that follows
        are excluded, so an assertion cannot pass on text the test typed.
        """
        self.type(command)
        self.submit(timeout=timeout)
        return self._output

    def _row_index(self):
        """Where the cursor is, counted from the first row ever drawn."""
        with self._lock:
            return len(self.vt.scrollback) + self.vt.cy

    def resize(self, rows, cols):
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        with self._lock:
            self.vt.resize(rows, cols)
        return self

    @property
    def screen(self):
        with self._lock:
            return self.vt.screen()

    @property
    def text(self):
        with self._lock:
            return self.vt.text()

    @property
    def rows(self):
        with self._lock:
            return self.vt.rows_all()

    @property
    def raw(self):
        with self._lock:
            return bytes(self._raw).decode("utf-8", "replace")

    @property
    def cursor(self):
        with self._lock:
            return (self.vt.cy, self.vt.cx)

    @property
    def line(self):
        """The line being edited, without the prompt and joined across wraps."""
        with self._lock:
            rows = self.vt.padded_rows()
            cols = self.vt.cols
        start = next((i for i in range(len(rows) - 1, -1, -1) if self._is_prompt(rows[i])), None)
        if start is None:
            return ""
        marker = self.marker if self.marker in rows[start] else self.continuation
        cut = rows[start].index(marker) + len(marker) if marker else 0
        cut += 1 if rows[start][cut : cut + 1] == " " else 0
        pieces = []
        for y in range(start, len(rows)):
            row = rows[y]
            pieces.append(row[cut:] if y == start else row)
            # A wrapped row fills the width; a short one ends the buffer.
            if row[cols - 1] == " ":
                break
        return "".join(pieces).rstrip()

    def _is_prompt(self, row):
        # `marker` is part of the rendered prompt.
        return self.marker in row or self.continuation in row

    def visible_rows(self):
        with self._lock:
            return [r for r in self.vt.screen_rows() if r.strip()]

    def output(self):
        """The lines the last command printed."""
        return self._output

    def styles_of(self, needle):
        with self._lock:
            return self.vt.styles_of(needle)

    def style_of(self, needle):
        """How `needle` is styled where it appears most recently."""
        found = self.styles_of(needle)
        return found[-1] if found else None

    def wait_prompt(self, timeout=10.0):
        """Wait until a fresh editor holds the terminal."""
        deadline = time.time() + timeout
        while time.time() < deadline:
            with self._lock:
                editor_up = self.vt.cursor_reports > self._reports_at_send
            rows = self.visible_rows()

            waiting = not self.marker or (rows and rows[-1].rstrip().endswith(self.marker))
            if waiting and (editor_up or not self.attached):
                return self
            time.sleep(0.01)
        raise Timeout(f"waiting for a prompt\n--- screen ---\n{self.screen}")

    def settle(self, seconds=0.4, quiet=0.05):
        """Wait for the shell to stop drawing, or timeout"""
        deadline = time.time() + seconds
        with self._lock:
            seen = len(self._raw)
        last_change = time.time()
        while time.time() < deadline:
            time.sleep(0.005)
            with self._lock:
                size = len(self._raw)
                drawn = size > self._raw_at_send
            if size != seen:
                seen, last_change = size, time.time()
            elif drawn and time.time() - last_change >= quiet:
                return self
        return self

    def wait_exit(self, timeout=8.0):
        """True once the shell has terminated."""
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                done, _ = os.waitpid(self.pid, os.WNOHANG)
            except (ChildProcessError, OSError):
                return True
            if done == self.pid:
                return True
            time.sleep(0.02)
        return False

    def _pump(self):
        while not self._closed:
            try:
                ready, _, _ = select.select([self.master], [], [], 0.05)
            except (OSError, ValueError):
                return
            if not ready:
                continue
            try:
                data = os.read(self.master, 65536)
            except OSError:
                return
            if not data:
                return
            with self._lock:
                self._raw.extend(data)
                replies = self.vt.feed(data)
            if replies:
                try:
                    os.write(self.master, replies)
                except OSError:
                    return

    def close(self):
        self._closed = True
        for action in (
            lambda: os.kill(self.pid, signal.SIGKILL),
            lambda: os.waitpid(self.pid, 0),
            lambda: os.close(self.master),
        ):
            try:
                action()
            except OSError:
                pass

    def __enter__(self):
        try:
            self.wait_prompt()
        except BaseException:
            self.close()
            raise
        # How far a prompt advances the cursor (height).
        start = self._row_index()
        self.send("\r")
        self.wait_prompt()
        self.prompt_height = max(1, self._row_index() - start)
        return self

    def __exit__(self, *exc):
        self.close()


def shell(**kw):
    """A shell at a fresh prompt, for `with shell() as sh:`."""
    return Shell(**kw)


BASH_COMPLETION = "/usr/share/bash-completion/bash_completion"


def complete(line, rc="", bash_completion=False):
    """What the completer offers for `line`, with no terminal in the way."""
    from .runner import scratch

    setup = ""
    if bash_completion:
        if not os.path.exists(BASH_COMPLETION):
            raise Skipped("bash-completion is not installed")
        setup = f"source {BASH_COMPLETION}\n"

    script = f"enable -f {LIB} reedline\n{setup}{rc}\nreedline --complete {shlex.quote(line)}\n"
    done = subprocess.run(
        ["bash", "--noprofile", "--norc"],
        input=script,
        capture_output=True,
        text=True,
        timeout=30,
        env=clean_env(scratch()),
    )

    candidates = [l for l in done.stdout.split("\n") if l]
    if done.returncode != 0 or (not candidates and done.stderr.strip()):
        raise AssertionError(
            f"completion shell failed ({done.returncode}) for {line!r}\n"
            f"--- stderr ---\n{done.stderr}--- stdout ---\n{done.stdout}"
        )
    return candidates
