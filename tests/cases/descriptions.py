"""Descriptions next to completions, from the manual.

The manual is faked: a directory first in PATH holds a `whatis` and a `man`
that answer from fixtures, so the test is about the plumbing, not about what
this machine has installed. The parsers are unit-tested against real layouts.
"""

import gzip
import os
import stat

from harness import TAB, ctrl, scratch, shell, test

LIST_MENU = '[menu]\nstyle = "list"\n\n[menu.list]\ndescription_position = "after"\n'

PAGE = """\
WIDGET(1)

NAME
       widget - frob the widgets

OPTIONS
       --frob
           Frob harder than usual.

       --nofrob
           Do not frob at all.

SUBCOMMANDS
       widget-frob(1)
              Frob one widget, at length

       widget-frobnicate(1)
              Frobnicate many widgets
"""

SUBCOMMAND_PAGE = """\
WIDGET-FROB(1)

NAME
       widget-frob - frob one widget

OPTIONS
       --force
           Frob it even if it resists.
"""


def fake_manual():
    """A directory with `widget`, `widgeon`, and a `whatis` and `man` that
    know them. Returns it."""
    tools = scratch()
    page = os.path.join(tools, "widget.1")
    subpage = os.path.join(tools, "widget-frob.1")
    with open(page, "w") as fh:
        fh.write(PAGE)
    with open(subpage, "w") as fh:
        fh.write(SUBCOMMAND_PAGE)
    scripts = {
        "widget": "#!/bin/sh\necho widget\n",
        "widgeon": "#!/bin/sh\necho widgeon\n",
        "whatis": "#!/bin/sh\n"
        "echo 'widget (1)           - Frob the widgets'\n"
        "echo 'widgeon (1)          - Look like a widget'\n"
        "echo 'widget-frob (1)      - Frob one widget'\n",
        # `man -w -- NAME` names the page; `man ... -P cat PATH` formats it.
        "man": "#!/bin/sh\n"
        "case \"$1\" in\n"
        f"  -w) case \"$3\" in widget) echo {page};; widget-frob) echo {subpage};; *) exit 1;; esac;;\n"
        "  *) cat \"$5\";;\n"
        "esac\n",
    }
    for name, body in scripts.items():
        path = os.path.join(tools, name)
        with open(path, "w") as fh:
            fh.write(body)
        os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
    return tools


def with_manual(config=LIST_MENU, rc=""):
    tools = fake_manual()
    cache = scratch()
    env = {"PATH": tools + ":" + os.environ.get("PATH", "/usr/bin:/bin"), "XDG_CACHE_HOME": cache}
    return shell(config=config, rc=rc, env=env), cache


def menu_rows(sh):
    return [row.strip() for row in sh.visible_rows()]


@test
def test_a_command_name_shows_what_the_command_does():
    sh, _ = with_manual()
    with sh:
        sh.type("widge")
        sh.press(TAB)
        rows = menu_rows(sh)
        assert any("widget" in r and "Frob the widgets" in r for r in rows), sh.screen
        assert any("widgeon" in r and "Look like a widget" in r for r in rows), sh.screen


@test
def test_a_builtin_shows_bashs_own_summary():
    """`help -d`, so no manual page is needed for `cd` and friends."""
    sh, _ = with_manual()
    with sh:
        sh.type("call")
        sh.press(TAB)
        # `caller` is a builtin; the fake `whatis` has never heard of it.
        rows = menu_rows(sh)
        assert any("caller" in r and "context of the current subroutine" in r for r in rows), sh.screen


@test
def test_an_option_shows_what_it_does_and_is_cached_on_disk():
    sh, cache = with_manual(rc="complete -W '--frob --nofrob' widget")
    with sh:
        sh.type("widget --")
        sh.press(TAB)
        rows = menu_rows(sh)
        assert any("--frob" in r and "Frob harder than usual." in r for r in rows), sh.screen
        assert any("--nofrob" in r and "Do not frob at all." in r for r in rows), sh.screen
        sh.press(ctrl("c"))
        sh.wait_prompt()

    cached = os.path.join(cache, "reedline-bash", "man", "widget.gz")
    with gzip.open(cached, "rt") as fh:
        text = fh.read()
    assert "--frob\tFrob harder than usual." in text, text


@test
def test_descriptions_can_be_turned_off():
    config = LIST_MENU + "\n[completion]\ndescriptions = false\n"
    sh, _ = with_manual(config=config)
    with sh:
        sh.type("widge")
        sh.press(TAB)
        rows = menu_rows(sh)
        assert any("widget" in r for r in rows), sh.screen
        assert not any("Frob" in r for r in rows), sh.screen


@test
def test_a_subcommand_is_described_by_its_own_page_or_its_parents_listing():
    """`widget frob` has a page of its own, widget-frob(1), as `git add`
    does; `widget frobnicate` is only listed in widget(1), as clap, cargo and
    docker document theirs."""
    sh, _ = with_manual(rc="complete -W 'frob frobnicate' widget")
    with sh:
        sh.type("widget f")
        sh.press(TAB)
        rows = menu_rows(sh)
        assert any("frob " in r and "Frob one widget" in r for r in rows), sh.screen
        assert any("frobnicate" in r and "Frobnicate many widgets" in r for r in rows), sh.screen


@test
def test_an_option_after_a_subcommand_is_read_from_the_subcommands_page():
    sh, _ = with_manual(rc="complete -W '--force --frob' widget")
    with sh:
        sh.type("widget frob --")
        sh.press(TAB)
        rows = menu_rows(sh)
        assert any("--force" in r and "Frob it even if it resists." in r for r in rows), sh.screen
        # Not in widget-frob(1), and widget(1) is not consulted for it.
        assert any("--frob" in r and "harder" not in r for r in rows), sh.screen


@test
def test_the_disk_cache_can_be_turned_off():
    config = LIST_MENU + "\n[completion]\ndescription_cache = false\n"
    sh, cache = with_manual(config=config, rc="complete -W '--frob --nofrob' widget")
    with sh:
        sh.type("widget --")
        sh.press(TAB)
        rows = menu_rows(sh)
        assert any("--frob" in r and "Frob harder than usual." in r for r in rows), sh.screen
    assert not os.path.exists(os.path.join(cache, "reedline-bash")), os.listdir(cache)


@test
def test_clearing_the_cache_removes_the_files():
    sh, cache = with_manual(rc="complete -W '--frob --nofrob' widget")
    with sh:
        sh.type("widget --")
        sh.press(TAB)
        sh.press(ctrl("c"))
        sh.wait_prompt()
        cached = os.path.join(cache, "reedline-bash", "man", "widget.gz")
        assert os.path.exists(cached)

        out = sh.run("reedline clear-cache")
        assert out and "cleared 1 cached page" in out[0], out
        assert not os.path.exists(cached)

        # This shell keeps what it parsed; the next one starts afresh.
        sh.type("widget --")
        sh.press(TAB)
        rows = menu_rows(sh)
        assert any("--frob" in r and "Frob harder than usual." in r for r in rows), sh.screen
        assert not os.path.exists(cached)
