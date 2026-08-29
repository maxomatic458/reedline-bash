import os
import socket

from harness import test
from harness.shell import CRATE


def _blocked(what, action):
    try:
        action()
    except OSError:
        return
    raise AssertionError(f"escaped the sandbox: {what}")


@test
def test_the_sandbox_is_actually_sealed():
    """Check if the test env is sandboxed properly"""
    _blocked("write the root filesystem", lambda: open("/PWNED", "w"))
    _blocked("write the source tree", lambda: open(f"{CRATE}/PWNED", "w"))
    _blocked("reach the network", lambda: socket.create_connection(("1.1.1.1", 443), 3))
    
    assert os.geteuid() != 0, "running as root"
    assert os.listdir(os.environ["HOME"]) == [], os.environ["HOME"]

    # A pid namespace of its own, with the runner as its first process.
    with open("/proc/1/cmdline", "rb") as fh:
        assert b"e2e" in fh.read(), "pid 1 is not the runner"
