from harness import BACKSPACE, ESC, LEFT, RIGHT, UP, arrow, ctrl, end, home, shell, test

MODES = ["emacs", "vi", "helix"]


@test(mode=MODES)
def test_typing_and_submitting(mode):
    with shell(mode=mode) as sh:
        assert sh.run("echo typed") == ["typed"]


@test(mode=MODES)
def test_backspace_removes_the_last_character(mode):
    with shell(mode=mode) as sh:
        sh.type("echo abcX")
        sh.press(BACKSPACE)
        assert sh.line == "echo abc"
        assert sh.run("") == ["abc"]


@test(mode=MODES)
def test_the_arrows_move_within_the_line(mode):
    with shell(mode=mode) as sh:
        sh.type("echo ac")
        sh.press(LEFT)
        sh.type("b")
        assert sh.line == "echo abc"


@test(mode=MODES)
def test_editing_in_the_middle_then_returning_to_the_end(mode):
    with shell(mode=mode) as sh:
        sh.type("echo world")
        sh.press(*[LEFT] * 5)
        sh.type("hello ")
        sh.press(*[RIGHT] * 5)
        sh.type("!")
        assert sh.line == "echo hello world!"


@test(mode=MODES)
def test_a_word_left_and_right(mode):
    with shell(mode=mode) as sh:
        sh.type("echo alpha beta")
        sh.press(arrow("left", ctrl=True))
        sh.type("X")
        assert sh.line == "echo alpha Xbeta"


@test(mode=MODES)
def test_ctrl_right_moves_a_word_forward(mode):
    with shell(mode=mode) as sh:
        sh.type("echo alpha beta")
        sh.press(home())
        sh.press(arrow("right", ctrl=True))
        sh.type("X")
        assert sh.line == "echoX alpha beta", sh.line


@test(mode=MODES)
def test_the_home_and_end_keys_jump_to_the_edges(mode):
    with shell(mode=mode) as sh:
        sh.type("middle")
        sh.press(home())
        sh.type("echo ")
        sh.press(end())
        sh.type("!")
        assert sh.line == "echo middle!"


@test(mode=MODES)
def test_home_and_end(mode):
    with shell(mode=mode) as sh:
        sh.type("middle")
        sh.press(ctrl("a"))
        sh.type("echo ")
        sh.press(ctrl("e"))
        sh.type("!")
        assert sh.line == "echo middle!"


@test
def test_emacs_kill_and_yank():
    with shell(mode="emacs") as sh:
        sh.type("echo alpha beta")
        sh.press(ctrl("w"))
        assert sh.line == "echo alpha"
        sh.press(ctrl("y"))
        assert sh.line == "echo alpha beta"


@test
def test_emacs_ctrl_u_clears_to_the_start():
    with shell(mode="emacs") as sh:
        sh.type("echo discard me")
        sh.press(ctrl("u"))
        assert sh.line == ""


@test
def test_emacs_ctrl_k_kills_to_the_end():
    with shell(mode="emacs") as sh:
        sh.type("echo keep drop")
        sh.press(*[LEFT] * 4, ctrl("k"))
        assert sh.line == "echo keep"


@test
def test_vi_normal_mode_moves_and_deletes():
    with shell(mode="vi") as sh:
        sh.type("echo alpha")
        sh.press(ESC)          # leave insert
        sh.press("0")             # start of line
        sh.press("x")             # delete a character
        assert sh.line == "cho alpha"


@test
def test_vi_insert_mode_is_where_typing_lands():
    with shell(mode="vi") as sh:
        sh.type("echo one")
        sh.press(ESC, "0", "i")
        sh.type("# ")
        assert sh.line == "# echo one"


@test
def test_helix_normal_mode_moves():
    with shell(mode="helix") as sh:
        sh.type("echo alpha")
        sh.press(ESC, "0")
        sh.press("d")
        assert sh.line != "echo alpha", "normal mode did nothing"


@test(mode=MODES)
def test_unicode_editing_counts_characters_not_bytes(mode):
    with shell(mode=mode) as sh:
        sh.type("echo äöü")
        sh.press(BACKSPACE)
        assert sh.line == "echo äö"


@test(cols=[40, 120])
def test_a_line_longer_than_the_terminal_is_still_editable(cols):
    with shell(cols=cols) as sh:
        sh.type("echo " + "x" * (cols + 30))
        sh.press(BACKSPACE)
        assert sh.line.endswith("x" * 10)
        assert len(sh.line) == len("echo ") + cols + 29


@test
def test_the_previous_command_comes_back_with_the_up_arrow():
    with shell() as sh:
        sh.run("echo first")
        sh.press(UP)
        assert sh.line == "echo first"
