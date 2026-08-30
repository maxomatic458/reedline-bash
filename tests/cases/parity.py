"""History behaviour, checked against the same bash without the plugin.

Each case runs twice -- once through reedline, once through readline -- and
the two have to agree. Native bash is the specification.
"""

import os

from harness import TAB, UP, ctrl, shell, test


def both(script, **kw):
    """Run `script` under reedline and under readline, and hand back both."""
    results = []
    for attach in (True, False):
        with shell(attach=attach, **kw) as sh:
            results.append(script(sh))
    return results


def recall(sh, presses):
    """What walking back through the history offers, one line per press."""
    seen = []
    for _ in range(presses):
        sh.press(UP)
        seen.append(sh.line)
    return seen


@test
def test_the_two_sides_really_are_different_line_editors():
    """Guards the rest of the file.

    If both sides ran the same editor every comparison here would pass for
    the wrong reason, so one known difference has to show through."""

    def script(sh):
        sh.run("echo searchable")
        sh.press(ctrl("r"))
        sh.type("search")
        return "reverse-i-search" in sh.screen

    ours, native = both(script)
    assert native, "readline's own search prompt did not appear"
    assert not ours, "the plugin was loaded on the native side too"


@test
def test_recall_walks_back_in_the_same_order():
    def script(sh):
        for word in ("one", "two", "three"):
            sh.run(f"echo {word}")
        return recall(sh, 4)

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_a_repeated_command_is_walked_the_same_number_of_times():
    def script(sh):
        for command in ("echo first", "echo dup", "echo dup", "echo other"):
            sh.run(command)
        return recall(sh, 5)

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_clearing_the_history_leaves_the_same_thing_behind():
    def script(sh):
        sh.run("echo cleared_away")
        sh.run("history -c")
        return recall(sh, 3)

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_deleting_an_entry_leaves_the_same_thing_behind():
    def script(sh):
        sh.run("echo doomed")
        sh.run("echo spared")
        sh.run("history -d 1")
        return recall(sh, 4)

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_a_history_file_read_mid_session_reaches_recall_the_same_way():
    def script(sh):
        with open(os.path.join(sh.dir, "extra"), "w") as fh:
            fh.write("echo read_in_late\n")
        # Through $HOME, so the recorded command text does not carry the
        # scratch path and the two runs stay comparable.
        sh.run("history -r $HOME/extra")
        return recall(sh, 2)

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_the_history_builtin_lists_the_same_commands():
    def script(sh):
        for word in ("one", "two"):
            sh.run(f"echo {word}")
        return [row.split(None, 1)[-1] for row in sh.run("history")]

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_histsize_drops_the_oldest_the_same_way():
    def script(sh):
        for word in ("one", "two", "three", "four"):
            sh.run(f"echo {word}")
        return [row.split(None, 1)[-1] for row in sh.run("history")]

    ours, native = both(script, rc="HISTSIZE=3\n")
    assert ours == native, (ours, native)


@test
def test_histcontrol_ignoredups_keeps_the_same_entries():
    def script(sh):
        for command in ("echo same", "echo same", "echo other"):
            sh.run(command)
        return recall(sh, 4)

    ours, native = both(script, rc="HISTCONTROL=ignoredups\n")
    assert ours == native, (ours, native)


@test
def test_histcontrol_ignorespace_keeps_the_same_entries():
    def script(sh):
        sh.run("echo kept")
        sh.run(" echo hidden")
        return recall(sh, 3)

    ours, native = both(script, rc="HISTCONTROL=ignorespace\n")
    assert ours == native, (ours, native)


@test
def test_the_history_file_is_written_the_same_way():
    def script(sh):
        sh.run("HISTFILE=$HOME/histfile")
        sh.run("echo persisted")
        sh.send("exit\r")
        sh.wait_exit()
        with open(os.path.join(sh.dir, "histfile")) as fh:
            return fh.read().splitlines()

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_a_space_prefixed_command_without_histcontrol():
    """Bash records it when HISTCONTROL is unset, so recall should offer it."""

    def script(sh):
        sh.run("echo kept")
        sh.run(" echo hidden")
        return recall(sh, 3)

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_completing_a_variable_leaves_the_dollar_alone():
    def script(sh):
        sh.type("echo $HISTFIL")
        sh.press(TAB)
        return sh.line

    ours, native = both(script)
    assert ours == native, (ours, native)


@test
def test_completing_a_variable_that_is_a_directory_marks_it():
    def script(sh):
        sh.type("echo $HOM")
        sh.press(TAB)
        return sh.line

    ours, native = both(script)
    assert ours == native, (ours, native)
