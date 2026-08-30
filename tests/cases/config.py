import os

from harness import ctrl, fkey, scratch, shell, test


@test
def test_no_config_file_at_all_is_the_normal_case():
    with shell(env={"REEDLINE_BASH_CONFIG": "/nonexistent/config.toml"}) as sh:
        assert sh.run("echo defaults") == ["defaults"]


@test(mode=["emacs", "vi", "helix"])
def test_the_edit_mode_is_selected(mode):
    with shell(mode=mode) as sh:
        assert sh.run("echo mode") == ["mode"]


@test
def test_an_unknown_key_is_reported_and_the_rest_still_applies():
    config = '[editor]\nnot_a_setting = true\nhighlight = false\n'
    with shell(config=config) as sh:
        assert "not_a_setting" in sh.text, sh.text
        sh.type("echo hi")
        assert sh.style_of("echo").fg == "", "highlight = false was dropped"


@test
def test_unparseable_config_falls_back_to_defaults_and_says_so():
    with shell(config="[editor\nbroken = ") as sh:
        assert "unreadable" in sh.text, sh.text
        assert sh.run("echo still_works") == ["still_works"]


@test
def test_a_bad_value_names_the_setting_it_came_from():
    with shell(config='[editor]\nmode = "nonsense"\n') as sh:
        assert "editor.mode" in sh.text, sh.text
        assert sh.run("echo ok") == ["ok"]


@test
def test_a_bad_keybinding_is_reported_and_the_shell_still_starts():
    config = '[[keybinding]]\nkey = "hyper-q"\nevent = "ClearScreen"\n'
    with shell(config=config) as sh:
        assert "hyper" in sh.text, sh.text
        assert sh.run("echo ok") == ["ok"]


@test
def test_an_edited_config_applies_without_restarting():
    """One editor serves the whole session, so noticing an edit is deliberate:
    a stat per prompt, not a watcher.

    The off-by-one is real. The prompt on screen ran its check before the file
    was saved, so the change lands on the one after it.
    """
    workdir = scratch()
    path = os.path.join(workdir, "config.toml")
    with open(path, "w") as fh:
        fh.write('[colors]\ncommand = "#010203"\n')
    with shell(env={"REEDLINE_BASH_CONFIG": path}) as sh:
        sh.type("grep x")
        assert sh.style_of("grep").fg == "rgb(1,2,3)"
        sh.press(ctrl("c"))  # abandon: the command would read stdin
        sh.wait_prompt()

        with open(path, "w") as fh:
            fh.write('[colors]\ncommand = "#0a0b0c"\n')

        # This prompt checked before the edit; the next one sees it.
        sh.run("true")
        sh.type("grep x")
        assert sh.style_of("grep").fg == "rgb(10,11,12)", sh.style_of("grep")


@test
def test_a_keybinding_can_be_added():
    config = '[[keybinding]]\nkey = "f5"\nedit = [{ InsertString = "INSERTED" }]\n'
    with shell(config=config) as sh:
        sh.press(fkey(5))
        assert sh.line == "INSERTED"


@test
def test_a_keybinding_can_name_a_reedline_event():
    config = '[[keybinding]]\nkey = "f5"\nevent = "ClearScreen"\n'
    with shell(config=config) as sh:
        sh.run("echo before_clear")
        sh.press(fkey(5))
        assert "before_clear" not in sh.screen


@test
def test_a_custom_binding_leaves_the_defaults_alone():
    config = '[[keybinding]]\nkey = "f5"\nedit = [{ InsertString = "X" }]\n'
    with shell(config=config) as sh:
        sh.type("echo still")
        sh.press(ctrl("w"))
        assert sh.line == "echo"


ABBREV = '[abbreviations]\ngs = "echo expanded"\n'


def typed(sh, word):
    """Type `word` then a space, as two keystrokes."""
    sh.type(word)
    sh.type(" ")


@test
def test_an_abbreviation_expands_on_the_following_space():
    with shell(config=ABBREV) as sh:
        sh.type("gs")
        assert sh.line == "gs", "expanded before the space"
        sh.type(" ")
        assert sh.line == "echo expanded", sh.line
        # Unlike a bash alias, what runs is what is on the line.
        assert sh.run("") == ["expanded"]


@test
def test_enter_expands_an_abbreviation_before_running_it():
    """Accepting the line expands it too, so the key alone is enough."""
    with shell(config=ABBREV) as sh:
        sh.type("gs")
        sh.submit()
        assert sh.output() == ["expanded"], sh.output()


@test
def test_a_word_that_is_not_an_abbreviation_is_left_alone():
    with shell(config=ABBREV) as sh:
        typed(sh, "notgs")
        assert sh.line == "notgs", sh.line
    with shell(config=ABBREV) as sh:
        typed(sh, "a-gs")
        assert sh.line == "a-gs", sh.line


@test
def test_an_abbreviation_expands_anywhere_in_the_line():
    """Any word matching a key, not just the one in command position."""
    with shell(config=ABBREV) as sh:
        sh.type("echo")
        typed(sh, " gs")
        assert sh.line == "echo echo expanded", sh.line


@test
def test_bulk_inserted_text_is_not_expanded():
    """Expansion is a keypress, so a paste comes through as it was written."""
    with shell(config=ABBREV) as sh:
        sh.type("gs ")  # one write: the space is not its own keystroke
        assert sh.line == "gs", sh.line


@test
def test_hints_can_be_turned_off():
    with shell(config="[editor]\nhints = false\n") as sh:
        sh.run("echo hinted_command")
        sh.type("echo hi")
        assert "hinted_command" not in sh.line


@test
def test_a_hint_is_shown_when_enabled():
    with shell() as sh:
        sh.run("echo hinted_command")
        sh.type("echo hi")
        assert "nted_command" in sh.screen, sh.screen


@test
def test_the_buffer_editor_opens_the_line_in_an_external_command():
    workdir = scratch()
    editor = os.path.join(workdir, "fake-editor")
    with open(editor, "w") as fh:
        fh.write("#!/bin/bash\necho 'echo from_editor' > \"$1\"\n")
    os.chmod(editor, 0o755)
    with shell(config=f'[editor]\nbuffer_editor = "{editor}"\n') as sh:
        sh.press(ctrl("o"))
        assert sh.run("") == ["from_editor"]


@test
def test_shell_integration_emits_terminal_markers():
    with shell(config='[editor]\nshell_integration = "osc133"\n') as sh:
        sh.run("echo marked")
        assert "\x1b]133;" in sh.raw, "no OSC 133 markers"


@test
def test_bracketed_paste_is_enabled_only_when_asked_for():
    with shell() as sh:
        assert "\x1b[?2004h" in sh.raw, "bracketed paste is on by default"
    with shell(config="[editor]\nbracketed_paste = false\n") as sh:
        assert "\x1b[?2004h" not in sh.raw, "still enabled after being turned off"


@test
def test_the_cursor_shape_is_set_per_edit_mode():
    config = '[cursor]\nemacs = "blink_block"\n'
    with shell(mode="emacs", config=config) as sh:
        sh.type("x")
        assert "\x1b[1 q" in sh.raw, "no blinking-block cursor requested"


@test
def test_no_cursor_shape_is_requested_by_default():
    """`inherit` means the terminal keeps whatever the user set."""
    with shell() as sh:
        sh.type("x")
        assert " q" not in sh.raw, "a cursor shape was set without being asked for"


@test
def test_click_to_move_needs_both_settings():
    """Two halves: the OSC 133 marker asks a supporting terminal to report
    clicks, and `mouse_click` is what acts on one. Either alone does nothing
    useful."""
    marker = '[editor]\nshell_integration = "osc133_click_events"\n'
    acting = "[editor]\nmouse_click = true\n"
    # Row 2 is the prompt row; column 10 is five characters into the buffer.
    click = ("\x1b[<0;10;2M", "\x1b[<0;10;2m")

    with shell(config=acting + marker.split("\n", 1)[1]) as sh:
        sh.type("echo hello world")
        sh.press(*click)
        sh.type("X")
        assert sh.line == "echo Xhello world", sh.line
        assert "click_events=1" in sh.raw, "the terminal was never asked for clicks"

    with shell(config=marker) as sh:
        sh.type("echo hello world")
        sh.press(*click)
        sh.type("X")
        assert sh.line == "echo hello worldX", "clicked without mouse_click"


@test
def test_the_kitty_protocol_is_not_forced_on():
    with shell() as sh:
        assert ">1u" not in sh.raw, "keyboard enhancement requested without asking"
