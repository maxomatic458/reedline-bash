from harness import ctrl, shell, test
from harness.shell import LIB


@test
def test_the_exit_status_of_the_previous_command_survives():
    with shell() as sh:
        sh.run("false")
        assert sh.run("echo $?") == ["1"]
        sh.run("true")
        assert sh.run("echo $?") == ["0"]


@test
def test_shell_state_persists_across_commands():
    with shell() as sh:
        sh.run("export KEPT=value")
        assert sh.run("echo $KEPT") == ["value"]
        sh.run("cd /tmp")
        assert sh.run("pwd") == ["/tmp"]


@test
def test_pipelines_and_redirections_work_normally():
    with shell() as sh:
        assert sh.run("printf 'a\\nb\\nc\\n' | grep -c .") == ["3"]
        sh.run("echo redirected > /tmp/rlb-redirect")
        assert sh.run("cat /tmp/rlb-redirect") == ["redirected"]


@test
def test_aliases_still_expand():
    with shell(rc="alias hi='echo aliased'") as sh:
        assert sh.run("hi") == ["aliased"]


@test
def test_a_background_job_can_be_started_and_waited_for():
    with shell() as sh:
        sh.run("sleep 0.1 &")
        assert sh.run("wait; echo done") == ["done"]


@test
def test_ctrl_z_suspends_the_command_not_the_shell():
    with shell() as sh:
        sh.type("sleep 30")
        sh.press("\r", settle=False)
        sh.settle()
        sh.press(ctrl("z"))
        sh.wait_prompt()
        assert sh.run("echo alive") == ["alive"]


@test
def test_a_finished_job_is_reported_without_disturbing_the_prompt():
    with shell() as sh:
        sh.run("sleep 0.05 &")
        sh.run("sleep 0.2")
        sh.type("echo intact")
        # Bash's job notice belongs above the prompt, not inside the line.
        assert sh.line == "echo intact", sh.line
        assert sh.run("") == ["intact"]


@test
def test_a_function_defined_at_the_prompt_can_be_called():
    with shell() as sh:
        sh.run("greet() { echo greeted; }")
        assert sh.run("greet") == ["greeted"]


@test
def test_it_can_be_enabled_in_an_already_running_shell():
    """Typed at a live prompt rather than loaded from an rc file, which is a
    different path: no startup stream is in flight, so `bash_input` itself is
    what gets taken over."""
    with shell(attach=False) as sh:
        sh.run(f"enable -f {LIB} reedline")
        sh.attached = True
        sh.type("echo taken_over")
        # The editor is drawing now: readline does not highlight.
        assert sh.style_of("echo").fg == "bright_green", sh.style_of("echo")
        assert sh.run("") == ["taken_over"]


@test
def test_it_loads_from_a_startup_file_that_sources_another():
    """VS Code launches bash with its own init file, which sources `~/.bashrc`
    -- so the enable happens a level deeper than usual.

    `stream_list` is a stack of the streams bash will pop back to, and the
    terminal is at the bottom of it. Taking the top gives the outer script.
    """
    with shell(attach="nested") as sh:
        sh.type("echo nested")
        # Readline does not highlight, so a colour here means the editor is ours.
        assert sh.style_of("echo").fg == "bright_green", sh.style_of("echo")
        assert sh.run("") == ["nested"]


@test
def test_unloading_hands_the_shell_back_to_readline():
    with shell() as sh:
        sh.type("enable -d reedline")
        sh.send("\r")
        # Readline never asks where the cursor is, so that readiness signal
        # goes away with the editor -- before the prompt this command produces.
        sh.attached = False
        sh.wait_prompt()
        assert sh.run("echo after_unload") == ["after_unload"]


@test
def test_the_shell_stays_single_threaded():
    """The `static mut` globals bound in src/bash are sound only because
    nothing here ever runs on a second thread."""
    with shell() as sh:
        out = sh.run("ls /proc/$$/task | wc -l")
        assert out == ["1"], f"the shell has {out} threads"


@test
def test_the_terminal_is_left_usable_after_the_editor_runs():
    with shell() as sh:
        sh.run("echo one")
        out = sh.run("stty -a | head -1")
        assert out and "rows" in out[0], out


@test
def test_a_command_that_reads_stdin_gets_the_terminal():
    with shell() as sh:
        sh.type("read -r answer")
        sh.press("\r", settle=False)
        sh.settle()
        sh.type("typed_in")
        sh.press("\r")
        sh.wait_prompt()
        assert sh.run("echo $answer") == ["typed_in"]
