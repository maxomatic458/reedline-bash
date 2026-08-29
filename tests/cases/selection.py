from harness import LEFT, arrow, end, fkey, home, kitty, shell, test

MODES = ["emacs", "vi", "helix"]

# reedline binds no key to cut, copy or paste a selection in any edit mode --
# nushell adds those in its own config -- so these bind their own.
CLIPBOARD = """
[[keybinding]]
key = "f5"
edit = [{ CutSelection = { granularity = "CharWise" } }]
[[keybinding]]
key = "f6"
edit = ["CopySelection"]
[[keybinding]]
key = "f7"
edit = ["PasteCutBufferBefore"]
[[keybinding]]
key = "f8"
edit = ["SelectAll"]
"""


def selected(sh):
    return [text for text, style in sh.vt.styled_runs() if style.reverse]


@test
def test_shift_left_selects_one_character_at_a_time():
    with shell() as sh:
        sh.type("echo hello world")
        sh.press(arrow("left", shift=True))
        assert selected(sh) == ["d"], selected(sh)
        sh.press(*[arrow("left", shift=True)] * 4)
        assert selected(sh) == ["world"], selected(sh)


@test(mode=MODES)
def test_ctrl_shift_left_selects_the_word(mode):
    """The nushell behaviour. reedline binds it in `add_common_selection_bindings`,
    which every edit mode pulls in."""
    with shell(mode=mode) as sh:
        sh.type("echo hello world")
        sh.press(arrow("left", shift=True, ctrl=True))
        assert selected(sh) == ["world"], selected(sh)


@test
def test_shift_right_extends_a_selection_forwards():
    with shell() as sh:
        sh.type("echo hello world")
        sh.press(*[LEFT] * 5)
        sh.press(*[arrow("right", shift=True)] * 5)
        assert selected(sh) == ["world"], selected(sh)


@test
def test_shift_end_selects_to_the_end_of_the_line():
    with shell() as sh:
        sh.type("echo hello world")
        sh.press(*[LEFT] * 5)
        sh.press(end(shift=True))
        assert selected(sh) == ["world"], selected(sh)


@test
def test_shift_home_selects_back_to_the_start():
    with shell() as sh:
        sh.type("echo hi")
        sh.press(home(shift=True))
        assert selected(sh) == ["echo hi"], selected(sh)


@test
def test_ctrl_shift_end_and_home_select_the_whole_buffer():
    with shell() as sh:
        sh.type("echo hello world")
        sh.press(home(shift=True, ctrl=True))
        assert selected(sh) == ["echo hello world"], selected(sh)
    with shell() as sh:
        sh.type("echo hello world")
        sh.press(*[LEFT] * 11)
        sh.press(end(shift=True, ctrl=True))
        assert selected(sh) == ["hello world"], selected(sh)


@test
def test_select_all_by_its_default_binding_needs_kitty():
    """`Ctrl+Shift+A` is bound by default, but a plain terminal sends the same
    byte for it as for `Ctrl+A`."""
    with shell(config=CLIPBOARD) as sh:
        sh.type("echo A hello world")
        sh.press(kitty("a", ctrl=True, shift=True), fkey(5))
        assert sh.line == "", sh.line


@test
def test_ctrl_shift_right_selects_the_word_after_the_cursor():
    with shell() as sh:
        sh.type("echo hello world")
        sh.press(*[LEFT] * 5)
        sh.press(arrow("right", shift=True, ctrl=True))
        assert selected(sh) == ["world"], selected(sh)


@test
def test_shift_up_and_down_select_across_lines():
    """Only meaningful once the buffer has more than one line. The cursor keeps
    its column, so the selection runs from that column on the row above."""
    with shell() as sh:
        sh.type("for i in 1; do")
        sh.press("\r")
        sh.type("echo hi")
        sh.press(arrow("up", shift=True))
        assert selected(sh) == ["n 1; do ", "echo hi"], selected(sh)
        # Back down again, and the selection collapses.
        sh.press(arrow("down", shift=True))
        assert selected(sh) == [], selected(sh)


@test
def test_a_bare_arrow_moves_without_selecting():
    with shell() as sh:
        sh.type("echo hello world")
        sh.press(LEFT, LEFT, LEFT)
        assert selected(sh) == []


@test
def test_a_selected_word_can_be_cut():
    with shell(config=CLIPBOARD) as sh:
        sh.type("echo A hello world")
        sh.press(arrow("left", shift=True, ctrl=True), fkey(5))
        assert sh.run("") == ["A hello"]


@test
def test_a_cut_selection_can_be_pasted_back():
    with shell(config=CLIPBOARD) as sh:
        sh.type("echo A hello world")
        sh.press(arrow("left", shift=True, ctrl=True), fkey(5), fkey(7), fkey(7))
        assert sh.run("") == ["A hello worldworld"]


@test
def test_a_cut_selection_can_be_pasted_somewhere_else():
    """The paste lands at the cursor, not back where the cut was taken."""
    with shell(config=CLIPBOARD) as sh:
        sh.type("echo alpha beta")
        sh.press(arrow("left", shift=True, ctrl=True), fkey(5))
        assert sh.line == "echo alpha", sh.line
        sh.press(home(), fkey(7))
        assert sh.line == "betaecho alpha", sh.line


@test
def test_copy_leaves_the_text_where_it_is():
    with shell(config=CLIPBOARD) as sh:
        sh.type("echo A hello world")
        sh.press(arrow("left", shift=True, ctrl=True), fkey(6), fkey(7))
        assert sh.run("") == ["A hello worldworld"]


@test
def test_select_all_then_cut_empties_the_line():
    with shell(config=CLIPBOARD) as sh:
        sh.type("echo A hello world")
        sh.press(fkey(8), fkey(5))
        assert sh.line == "", sh.line
        assert sh.run("echo after") == ["after"]


@test
def test_ctrl_shift_letters_need_the_kitty_protocol():
    """A plain terminal sends one byte for both Ctrl+X and Ctrl+Shift+X, so
    nushell's `control_shift` letter bindings only exist under kitty."""
    config = CLIPBOARD + (
        '\n[[keybinding]]\nkey = "ctrl-shift-x"\n'
        'edit = [{ CutSelection = { granularity = "CharWise" } }]\n'
    )
    with shell(config=config) as sh:
        sh.type("echo A hello world")
        sh.press(arrow("left", shift=True, ctrl=True), kitty("x", ctrl=True, shift=True))
        assert sh.run("") == ["A hello"]
