"""The `reedline` command itself."""

import os
import shutil
import subprocess

from harness import Skipped, scratch, shell, test


@test
def test_no_subcommand_reports_the_loaded_version():
    with shell() as sh:
        out = sh.run("reedline")
        assert out and out[0].startswith("reedline-bash ") and "(loaded)" in out[0], out


@test
def test_help_lists_the_subcommands_and_does_not_end_the_shell():
    with shell() as sh:
        out = " ".join(sh.run("reedline --help"))
        for name in ("complete", "clear-cache", "install-man"):
            assert name in out, out
        assert sh.run("echo $?") == ["0"]
        assert sh.run("reedline clear-cache --help; echo $?")[-1] == "0"


@test
def test_a_wrong_subcommand_is_an_error_and_the_shell_goes_on():
    with shell() as sh:
        sh.run('reedline frobnicate 2>"$HOME/err"; echo status=$?')
        assert sh.output()[-1] == "status=2", sh.output()
        assert any("frobnicate" in line for line in sh.run('cat "$HOME/err"')), sh.output()
        assert sh.run("echo alive") == ["alive"]


@test
def test_install_man_writes_a_page_per_command_where_man_looks():
    home = scratch()
    with shell(cwd=home) as sh:
        out = sh.run("reedline install-man")
        # The scratch home has no ~/.local/bin on PATH, so man will not look
        # there on its own; the command says how to make it.
        assert any("MANPATH" in line for line in out), out
        real_man = subprocess.run(["man", "--version"], capture_output=True, text=True)
        if real_man.stdout.startswith("man"):
            assert sh.run('MANPATH="$HOME/.local/share/man:" man -w reedline') == [
                os.path.join(home, ".local", "share", "man", "man1", "reedline.1")
            ]
    man1 = os.path.join(home, ".local", "share", "man", "man1")
    pages = sorted(os.listdir(man1))
    assert pages == [
        "reedline-clear-cache.1",
        "reedline-complete.1",
        "reedline-install-man.1",
        "reedline.1",
    ], pages
    assert any(line.endswith("reedline.1") for line in out), out
    if not shutil.which("man") or not real_man.stdout.startswith("man"):
        # Ubuntu's container images ship a stub that exits 0 saying so.
        raise Skipped("man is not installed, or is a stub")
    text = subprocess.run(
        ["man", "-P", "cat", "-l", os.path.join(man1, "reedline.1")],
        capture_output=True,
        text=True,
        env={"PATH": os.environ.get("PATH", ""), "LC_ALL": "C", "MANWIDTH": "1000"},
    ).stdout
    assert "reedline - Reedline as bash" in text, text
    assert "reedline-clear-cache(1)" in text, text
    assert "$XDG_CACHE_HOME/reedline-bash/man" in text, text


@test
def test_install_man_accepts_a_directory():
    target = os.path.join(scratch(), "pages")
    with shell() as sh:
        sh.run(f"reedline install-man --dir {target}")
    assert "reedline.1" in os.listdir(target)
