//! Running another program from inside bash.

use std::process::{Command, Stdio};

/// Run a command for its output with LC_ALL=C.
pub fn run(command: &mut Command) -> Option<String> {
    // Since this is called from within bash, we have to block SIGCHLD until the
    // child is reaped.
    let _held = HoldSigchld::new();
    let output = command
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `SIGCHLD` blocked in this process until dropped.
struct HoldSigchld(libc::sigset_t);

impl HoldSigchld {
    fn new() -> Self {
        // SAFETY: The sigset_t values are stack-owned, and we are single threaded.
        unsafe {
            let mut children: libc::sigset_t = std::mem::zeroed();
            let mut previous: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut children);
            libc::sigaddset(&mut children, libc::SIGCHLD);
            libc::sigprocmask(libc::SIG_BLOCK, &children, &mut previous);
            HoldSigchld(previous)
        }
    }
}

impl Drop for HoldSigchld {
    fn drop(&mut self) {
        // Drop the block on SIGCHLD
        unsafe { libc::sigprocmask(libc::SIG_SETMASK, &self.0, std::ptr::null_mut()) };
    }
}
