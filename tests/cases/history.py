import os

from harness import UP, ctrl, scratch, shell, test


@test
def test_the_previous_command_is_recalled():
    with shell() as sh:
        sh.run("echo first")
        sh.press(UP)
        assert sh.line == "echo first"


@test
def test_recall_walks_back_through_several_commands():
    with shell() as sh:
        for word in ("one", "two", "three"):
            sh.run(f"echo {word}")
        sh.press(UP, UP, UP)
        assert sh.line == "echo one"


@test
def test_history_is_seeded_from_the_shells_history_file():
    workdir = scratch()
    histfile = os.path.join(workdir, "history")
    with open(histfile, "w") as fh:
        fh.write("echo from_the_file\n")
    with shell(env={"HISTFILE": histfile}) as sh:
        sh.press(UP)
        assert sh.line == "echo from_the_file"


@test
def test_this_sessions_commands_reach_the_next_prompt():
    with shell() as sh:
        sh.run("echo session_local")
        sh.press(UP)
        assert sh.line == "echo session_local"


@test
def test_history_size_limits_how_far_back_recall_reaches():
    with shell(config="[history]\nsize = 2\n") as sh:
        for word in ("one", "two", "three"):
            sh.run(f"echo {word}")
        sh.press(UP, UP, UP, UP)
        assert "one" not in sh.line, sh.line


@test
def test_clearing_the_shells_history_clears_recall():
    with shell() as sh:
        sh.run("echo cleared_away")
        sh.run("history -c")
        recalled = []
        for _ in range(4):
            sh.press(UP)
            recalled.append(sh.line)
        assert not any("cleared_away" in line for line in recalled), recalled


@test
def test_clearing_the_shells_history_stops_the_hint():
    """`history -c` used to leave the suggestions coming."""
    with shell() as sh:
        sh.run("echo hinted_away")
        sh.type("echo hi")
        assert sh.line == "echo hinted_away", sh.line

        sh.press(ctrl("u"))
        sh.run("history -c")
        sh.type("echo hi")
        assert sh.line == "echo hi", sh.line


@test
def test_a_repeated_command_is_recalled_once_per_copy_the_shell_kept():
    """Four presses, which is what native bash takes.

    Reedline's own history would skip the second copy and reach `echo first`
    a press sooner."""
    with shell() as sh:
        for command in ("echo first", "echo dup", "echo dup", "echo other"):
            sh.run(command)
        sh.press(UP, UP, UP)
        assert sh.line == "echo dup", sh.line
        sh.press(UP)
        assert sh.line == "echo first", sh.line


@test
def test_a_line_the_shell_reads_mid_session_becomes_recallable():
    with shell() as sh:
        extra = os.path.join(sh.dir, "extra")
        with open(extra, "w") as fh:
            fh.write("echo read_in_late\n")
        sh.run(f"history -r {extra}")
        sh.press(UP)
        assert sh.line == "echo read_in_late", sh.line


@test
def test_a_deleted_entry_is_no_longer_recalled():
    with shell() as sh:
        sh.run("echo doomed")
        sh.run("echo spared")
        # Entries are numbered from the oldest, so 1 is `echo doomed`.
        sh.run("history -d 1")
        recalled = []
        for _ in range(5):
            sh.press(UP)
            recalled.append(sh.line)
        assert "echo spared" in recalled, recalled
        assert "echo doomed" not in recalled, recalled


@test
def test_reverse_search_finds_an_earlier_command():
    with shell() as sh:
        sh.run("echo needle_here")
        sh.run("echo other")
        sh.press(ctrl("r"))
        sh.type("needle")
        assert "needle_here" in sh.screen


@test
def test_the_search_prompt_is_reedlines_not_readlines():
    """Reedline runs the search, so its own prompt is what should appear."""
    with shell() as sh:
        sh.run("echo searchable")
        sh.press(ctrl("r"))
        sh.type("search")
        assert "reverse-search" in sh.screen, sh.screen
        assert "reverse-i-search" not in sh.screen, "readline's prompt leaked"


@test
def test_bang_bang_repeats_the_previous_command():
    with shell() as sh:
        sh.run("echo repeated")
        # Bash echoes what an expansion became before running it.
        assert sh.run("!!") == ["echo repeated", "repeated"]


@test
def test_bang_dollar_takes_the_last_argument():
    with shell() as sh:
        sh.run("echo alpha omega")
        assert sh.run("echo !$") == ["echo omega", "omega"]


@test
def test_a_bang_prefix_recalls_the_most_recent_match():
    with shell() as sh:
        sh.run("echo matched")
        sh.run("true")
        assert sh.run("!echo") == ["echo matched", "matched"]


@test
def test_a_bang_inside_quotes_is_not_mangled():
    with shell() as sh:
        assert sh.run("echo 'not history!'") == ["not history!"]
