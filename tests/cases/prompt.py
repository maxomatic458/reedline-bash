from harness import shell, test


@test
def test_a_prompt_expansion_is_bashs_own():
    with shell(ps1=r"[\u] $ ", marker="] $") as sh:
        assert sh.run("echo hi") == ["hi"]
        assert any("[" in r and "] $" in r for r in sh.visible_rows()), sh.screen


@test
def test_a_command_substitution_in_the_prompt_runs():
    with shell(ps1=r"$(echo SUB)$ ", marker="SUB$") as sh:
        assert sh.run("echo hi") == ["hi"]


@test
def test_a_coloured_prompt_keeps_its_colours():
    green = r"\[\e[32m\]GRN\[\e[0m\]$ "
    with shell(ps1=green, marker="GRN$") as sh:
        assert sh.style_of("GRN").fg == "green", sh.style_of("GRN")


@test
def test_a_multiline_prompt_is_not_duplicated():
    with shell(ps1="first\nsecond$ ", marker="second$") as sh:
        sh.run("echo hi")
        # One banner per prompt drawn, and no more.
        assert sh.text.count("first") == sh.text.count("second$"), sh.text


@test
def test_a_multiline_prompt_scrolls_cleanly():
    with shell(ps1="banner\n$ ", marker="$", rows=10) as sh:
        for i in range(8):
            assert sh.run(f"echo line{i}") == [f"line{i}"]


@test
def test_an_empty_prompt_is_allowed():
    with shell(ps1="", marker="") as sh:
        sh.type("echo bare")
        assert "echo bare" in sh.screen


@test
def test_a_prompt_of_wide_characters_lines_up():
    with shell(ps1="日本$ ", marker="日本$") as sh:
        sh.type("echo x")
        # Two columns each, plus "$ ".
        assert sh.cursor[1] == 4 + 2 + len("echo x"), sh.cursor


@test(cols=[30, 60])
def test_a_prompt_longer_than_the_terminal_wraps(cols):
    with shell(ps1="P" * (cols + 10) + "$ ", marker="$", cols=cols) as sh:
        assert sh.run("echo hi") == ["hi"]


@test
def test_a_prompt_that_changes_between_commands_is_redrawn():
    rc = "N=0\nPROMPT_COMMAND='N=$((N+1)); PS1=\"p$N> \"'"
    with shell(rc=rc, marker=">") as sh:
        sh.run("echo one")
        sh.run("echo two")
        assert "p2>" in sh.text, sh.text


@test
def test_prompt_command_writing_to_stdout_does_not_disturb_the_editor():
    with shell(rc="PROMPT_COMMAND='echo noise'") as sh:
        assert sh.run("echo hi") == ["hi"]


@test
def test_the_terminal_can_be_resized_under_the_editor():
    with shell(cols=80) as sh:
        sh.type("echo resized")
        sh.resize(24, 40)
        sh.press(" ")
        assert sh.run("") == ["resized"]
