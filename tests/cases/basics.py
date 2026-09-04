from harness import PROMPT, ctrl, shell, test
from harness.shell import LIB

@test
def test_a_typed_command_runs():
    with shell() as sh:
        assert sh.run("echo hello") == ["hello"]


@test
def test_several_commands_in_a_row():
    with shell() as sh:
        assert sh.run("echo one") == ["one"]
        assert sh.run("echo two") == ["two"]
        assert sh.run("echo three") == ["three"]


@test
def test_an_empty_line_gives_a_fresh_prompt():
    with shell() as sh:
        before = sh.text.count(PROMPT.rstrip())
        sh.submit()
        assert sh.run("echo after") == ["after"]
        assert sh.text.count(PROMPT.rstrip()) > before


@test
def test_the_command_line_is_drawn_exactly_once():
    with shell() as sh:
        sh.run("echo once")
        assert sh.text.count("echo once") == 1, sh.text


@test(cols=[40, 80, 200])
def test_a_line_that_wraps_is_drawn_exactly_once(cols):
    with shell(cols=cols) as sh:
        typed = "echo " + "w" * (cols + 20)
        sh.type(typed)
        assert sh.text.count("wwwwwwwwww") >= 1
        sh.submit()
        # The echoed line and the output both hold the run, and no more.
        assert sh.text.count("w" * (cols + 20)) <= 2, sh.text


@test
def test_output_excludes_the_command_that_produced_it():
    """`run` returns what the command printed, never the line typed to get it."""
    with shell() as sh:
        assert sh.run("echo marker") == ["marker"]
        assert sh.run("true") == []


@test
def test_a_command_printing_many_lines_keeps_them_in_order():
    with shell() as sh:
        assert sh.run("printf '%s\\n' a b c d") == ["a", "b", "c", "d"]


@test
def test_a_long_output_scrolls_into_the_scrollback():
    with shell(rows=10) as sh:
        out = sh.run("seq 1 40")
        assert out[0] == "1" and out[-1] == "40", out


@test
def test_unicode_survives_the_round_trip():
    with shell() as sh:
        assert sh.run("echo äöü 日本 🦀") == ["äöü 日本 🦀"]


@test
def test_a_shell_that_could_not_load_the_builtin_still_works():
    """The library is a line editor, not a prerequisite for having a shell."""
    with shell(attach=False) as sh:
        assert sh.run("echo plain") == ["plain"]


@test
def test_ctrl_d_on_an_empty_line_ends_the_session():
    with shell() as sh:
        sh.send(ctrl("d"))
        assert sh.wait_exit(), "the shell is still running"


@test
def test_ctrl_c_abandons_the_line_and_gives_a_fresh_prompt():
    with shell() as sh:
        sh.type("echo abandoned")
        sh.send(ctrl("c"))
        sh.wait_prompt()
        assert sh.run("echo after") == ["after"]
        assert "abandoned" not in " ".join(sh.output())


@test
def test_loading_the_builtin_twice_is_not_an_error():
    with shell() as sh:
        out = sh.run(f"enable -f {LIB} reedline; echo ok")
        assert out[-1] == "ok", out


@test
def test_ctrl_c_sets_the_exit_status_like_bash():
    with shell() as sh:
        sh.type("echo abandoned")
        sh.send(ctrl("c"))
        sh.wait_prompt()
        assert sh.run("echo $?") == ["130"]
