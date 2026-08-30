from harness import shell, test

COMMAND = "bright_green"
STRING = "bright_yellow"
VARIABLE = "bright_cyan"
OPERATOR = "bright_magenta"
COMMENT = "bright_black"


@test
def test_a_command_and_its_arguments_are_told_apart():
    with shell() as sh:
        sh.type("grep pattern file")
        assert sh.style_of("grep").fg == COMMAND
        assert sh.style_of("pattern").fg == ""


@test
def test_a_builtin_is_bold_where_a_command_is_not():
    with shell() as sh:
        sh.type("echo hi; grep x f")
        assert sh.style_of("echo").bold
        assert not sh.style_of("grep").bold


@test
def test_every_palette_entry_reaches_the_line():
    with shell() as sh:
        sh.type('echo "text" $HOME | grep x # trailing')
        assert sh.style_of('"text"').fg == STRING
        assert sh.style_of("$HOME").fg == VARIABLE
        assert sh.style_of("|").fg == OPERATOR
        assert sh.style_of("# trailing").fg == COMMENT
        assert sh.style_of("# trailing").italic


@test
def test_the_colours_are_configurable():
    config = (
        "[colors]\n"
        'command = "#010203"\nstring = "#040506"\nvariable = "#070809"\n'
    )
    with shell(config=config) as sh:
        sh.type('grep "s" $HOME')
        assert sh.style_of("grep").fg == "rgb(1,2,3)"
        assert sh.style_of('"s"').fg == "rgb(4,5,6)"
        assert sh.style_of("$HOME").fg == "rgb(7,8,9)"


@test
def test_highlighting_can_be_turned_off():
    with shell(config="[editor]\nhighlight = false\n") as sh:
        sh.type("echo hi")
        assert sh.style_of("echo").fg == "", sh.style_of("echo")


@test
def test_ansi_colours_can_be_turned_off_entirely():
    with shell(config="[editor]\nansi_colors = false\n") as sh:
        sh.type("echo hi")
        assert not sh.style_of("echo")


@test
def test_a_submitted_line_keeps_its_colours_in_the_scrollback():
    """Nothing repaints the line after it is accepted."""
    with shell() as sh:
        sh.run("echo kept")
        assert sh.style_of("echo").fg == COMMAND, "colour lost on submit"


@test
def test_a_function_body_typed_over_several_lines_is_highlighted():
    body = [
        "hello() {",
        '    if [ -z "$1" ]; then',
        '        echo "Hello, stranger!"',
        "    else",
        '        echo "Hello, $1!"',
        "    fi",
    ]
    with shell() as sh:
        for part in body:
            sh.type(part)
            sh.press("\r")
        sh.type("}")
        for word in ("if", "then", "else", "fi"):
            assert sh.style_of(word).fg == COMMAND, f"{word}: {sh.style_of(word)}"
        assert sh.style_of("echo").bold, "echo is a builtin"
        assert sh.style_of('"Hello, stranger!"').fg == STRING

        assert sh.run("") == []
        assert sh.run("hello world") == ["Hello, world!"]
        assert sh.run("hello") == ["Hello, stranger!"]


@test
def test_a_multiline_command_is_highlighted_throughout():
    with shell() as sh:
        sh.type("for i in 1 2; do")
        sh.press("\r")
        sh.type("echo $i")
        assert sh.style_of("$i").fg == VARIABLE
        assert sh.style_of("for").fg == COMMAND
