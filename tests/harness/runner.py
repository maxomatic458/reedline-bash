"""Registering tests, and running them one per core.

Tests run in worker *processes*, not threads. `pty.fork` and `subprocess` both
duplicate the whole process, and doing that from several threads at once lets
each child inherit descriptors another was midway through setting up -- which
showed up as a completion that intermittently answered nothing.
"""

import atexit
import importlib
import itertools
import multiprocessing
import os
import signal
import shutil
import sys
import tempfile
import time
import traceback
from concurrent import futures

REGISTRY = []
KNOWN = {}
_SCRATCH = []


def known_issue(reason):
    """Mark a test as a documented gap: reported, but not a failure."""

    def mark(fn):
        KNOWN[fn.__name__] = reason
        return fn

    return mark


def scratch():
    """A directory that lives until the process ends."""
    path = tempfile.mkdtemp(prefix="rlb-")
    _SCRATCH.append(path)
    return path


@atexit.register
def _clean_scratch():
    for path in _SCRATCH:
        shutil.rmtree(path, ignore_errors=True)


def test(*args, **params):
    """Register a test, optionally once per combination of `params`.

    @test
    def test_plain(): ...

    @test(mode=["emacs", "vi", "helix"])
    def test_per_mode(mode): ...
    """

    def register(fn):
        if not params:
            REGISTRY.append((fn.__module__, fn.__name__, {}, fn.__name__))
            return fn
        names = list(params)
        for combo in itertools.product(*(params[name] for name in names)):
            kwargs = dict(zip(names, combo))
            label = ",".join(f"{k}={v}" for k, v in kwargs.items())
            REGISTRY.append((fn.__module__, fn.__name__, kwargs, f"{fn.__name__}[{label}]"))
        return fn

    if args and callable(args[0]):
        return register(args[0])
    return register


TIMEOUT = 60


def _deadline(_signum, _frame):
    raise TimeoutError(f"test exceeded {TIMEOUT}s")


def _execute(spec):
    module, name, kwargs, label = spec
    from .shell import Skipped

    fn = getattr(importlib.import_module(module), name)
    started = time.time()
    signal.signal(signal.SIGALRM, _deadline)
    signal.alarm(TIMEOUT)
    try:
        fn(**kwargs)
    except Skipped as why:
        return label, "skipped", time.time() - started, str(why)
    except BaseException:
        return label, "failed", time.time() - started, traceback.format_exc()
    finally:
        signal.alarm(0)
    return label, "passed", time.time() - started, None


def require_container():
    """Refuse to run anywhere but inside `tests/sandboxed.sh`.

    A test is arbitrary Python driving a real shell, so it runs where it can do
    no harm or it does not run. The check is what the container *is* -- a pid
    namespace whose first process is this runner -- not a flag saying so.
    """
    try:
        with open("/proc/1/cmdline", "rb") as fh:
            if b"e2e" in fh.read():
                return
    except OSError:
        pass
    sys.exit(
        "Run the end-to-end suite through tests/sandboxed.sh, which builds a\n"
        "container image and runs these tests inside it: no network, nothing of\n"
        "this machine, and a filesystem they cannot write to.\n\n"
        "    tests/sandboxed.sh                # everything\n"
        "    tests/sandboxed.sh completion     # a subset\n"
        "    tests/sandboxed.sh -j1            # one at a time\n"
    )


def main(argv):
    require_container()

    jobs = os.cpu_count() or 4
    wanted = ""
    for arg in argv:
        if arg.startswith("-j"):
            jobs = int(arg[2:] or 0) or os.cpu_count() or 1
        else:
            wanted = arg

    selected = [spec for spec in REGISTRY if wanted in spec[3]]
    if not selected:
        # Exiting 0 here would report a green run that tested nothing.
        print(f"no test matches {wanted!r}, of {len(REGISTRY)} registered")
        return 1
    jobs = max(1, min(jobs, len(selected)))

    if jobs > 1:
        context = multiprocessing.get_context("forkserver")
        with futures.ProcessPoolExecutor(max_workers=jobs, mp_context=context) as pool:
            results = list(pool.map(_execute, selected))
    else:
        results = [_execute(spec) for spec in selected]

    passed, failed, skipped, known, fixed = [], [], [], [], []
    for label, outcome, elapsed, detail in results:
        base = label.split("[")[0]
        if base in KNOWN:
            if outcome == "failed":
                known.append(label)
                note = f"KNOWN -- {KNOWN[base]}"
            else:
                fixed.append(label)
                note = "FIXED -- drop the known_issue marker"
            print(f"  {label} ... {note} ({elapsed:.1f}s)")
            continue
        if outcome == "failed":
            failed.append((label, detail))
            note = "FAIL"
        elif outcome == "skipped":
            skipped.append(label)
            note = f"SKIPPED -- {detail}"
        else:
            passed.append(label)
            note = "ok"
        print(f"  {label} ... {note} ({elapsed:.1f}s)")

    print(
        f"\n{len(passed)} passed, {len(failed)} failed, {len(skipped)} skipped, "
        f"{len(known)} known, of {len(selected)}" + (f", {jobs} at a time" if jobs > 1 else "")
    )
    for label in fixed:
        print(f"  {label} now passes -- remove its known_issue marker")
    for label, detail in failed:
        print(f"\n=== {label} ===\n{detail}")
    return 1 if failed or fixed else 0
