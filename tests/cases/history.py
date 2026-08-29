import os

from harness import UP, ctrl, known_issue, scratch, shell, test


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
@known_issue("a space-prefixed command is still recalled in the session that ran it")
def test_the_ignore_prefix_keeps_a_command_out_of_recall():
    """Bash's `HISTCONTROL=ignorespace`, applied to reedline's own history.

    It has to hold for the command just run, which is when it matters."""
    with shell(config='[history]\nignore_prefix = " "\n') as sh:
        sh.run("echo kept")
        sh.run(" echo hidden")
        sh.press(UP)
        assert sh.line == "echo kept", sh.line


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
