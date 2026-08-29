from harness import CONTINUATION, shell, test


def continued(sh):
    return any(r.startswith(CONTINUATION.rstrip()) for r in sh.visible_rows())


@test
def test_an_unfinished_loop_opens_a_continuation():
    with shell() as sh:
        sh.type("for i in 1 2 3; do")
        sh.press("\r")
        assert continued(sh), sh.screen
        sh.type("echo loop_$i")
        sh.press("\r")
        sh.type("done")
        assert sh.run("") == ["loop_1", "loop_2", "loop_3"]


@test
def test_an_unterminated_quote_continues():
    with shell() as sh:
        sh.type("echo 'unfinished")
        sh.press("\r")
        assert continued(sh), sh.screen
        sh.type("still here'")
        assert sh.run("") == ["unfinished", "still here"]


@test
def test_a_heredoc_continues_until_its_delimiter():
    with shell() as sh:
        sh.type("cat <<EOF")
        sh.press("\r")
        assert continued(sh)
        sh.type("inside")
        sh.press("\r")
        assert continued(sh), "the heredoc ended early"
        sh.type("EOF")
        assert sh.run("") == ["inside"]


@test
def test_a_trailing_backslash_continues():
    with shell() as sh:
        sh.type("echo one \\")
        sh.press("\r")
        assert continued(sh), sh.screen
        sh.type("two")
        assert sh.run("") == ["one two"]


@test
def test_an_escaped_backslash_submits():
    with shell() as sh:
        assert sh.run("echo done\\\\") == ["done\\"]


@test
def test_a_syntax_error_is_submitted_for_bash_to_report():
    """Only an unfinished command continues; a wrong one is bash's to complain
    about, and swallowing it would leave the line stuck."""
    with shell() as sh:
        sh.type("echo )")
        sh.press("\r")
        sh.wait_prompt()
        assert not continued(sh), sh.screen
        assert "syntax error" in sh.text


@test
def test_an_if_block_across_several_lines():
    with shell() as sh:
        for part in ["if true; then", "echo yes", "else", "echo no"]:
            sh.type(part)
            sh.press("\r")
        sh.type("fi")
        assert sh.run("") == ["yes"]


@test
def test_a_custom_continuation_prompt_is_used():
    with shell(ps2="MORE> ") as sh:
        sh.type("for i in 1; do")
        sh.press("\r")
        assert any(r.startswith("MORE>") for r in sh.visible_rows()), sh.screen
