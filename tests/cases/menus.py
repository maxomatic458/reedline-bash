from harness import ESC, TAB, shell, test

WORDS = "complete -W 'alpha beta gamma delta epsilon zeta' widget"


@test(style=["columnar", "ide", "list"])
def test_each_menu_style_draws_the_candidates(style):
    with shell(rc=WORDS, config=f'[menu]\nstyle = "{style}"\n') as sh:
        sh.type("widget ")
        sh.press(TAB)
        assert "alpha" in sh.screen, sh.screen


@test
def test_the_columnar_menu_uses_the_configured_columns():
    config = '[menu]\nstyle = "columnar"\n[menu.columnar]\ncolumns = 2\ncol_width = 20\n'
    with shell(rc=WORDS, config=config) as sh:
        sh.type("widget ")
        sh.press(TAB)
        row = next(r for r in sh.visible_rows() if r.startswith("alpha"))
        assert len(row.split()) == 2, row


@test
def test_columnar_traversal_decides_which_way_the_grid_reads():
    for traversal, beside in (("horizontal", "beta"), ("vertical", "delta")):
        config = (
            '[menu]\nstyle = "columnar"\n'
            f'[menu.columnar]\ncolumns = 2\ncol_width = 20\ntraversal = "{traversal}"\n'
        )
        with shell(rc=WORDS, config=config) as sh:
            sh.type("widget ")
            sh.press(TAB)
            row = next(r for r in sh.visible_rows() if r.startswith("alpha"))
            assert row.split()[1] == beside, f"{traversal}: {row!r}"


@test
def test_the_list_menu_draws_one_entry_per_row():
    config = '[menu]\nstyle = "list"\n[menu.list]\npage_size = 3\n'
    with shell(rc=WORDS, config=config) as sh:
        sh.type("widget ")
        sh.press(TAB)
        rows = [r for r in sh.visible_rows() if "alpha" in r or "beta" in r]
        assert all(len(r.split()) <= 3 for r in rows), rows


@test
def test_the_ide_menu_border_characters_can_be_replaced():
    config = (
        '[menu]\nstyle = "ide"\n[menu.ide]\nborder = true\n'
        '[menu.ide.border_symbols]\ntop_left = "1"\ntop_right = "2"\n'
        'bottom_left = "3"\nbottom_right = "4"\nhorizontal = "-"\nvertical = "|"\n'
    )
    with shell(rc=WORDS, config=config) as sh:
        sh.type("widget ")
        sh.press(TAB)
        for corner in "1234":
            assert corner in sh.screen, f"{corner} missing:\n{sh.screen}"


@test
def test_the_menu_marker_is_drawn_when_set():
    config = '[menu]\nstyle = "columnar"\nmarker = "PICK> "\n'
    with shell(rc=WORDS, config=config) as sh:
        sh.type("widget ")
        sh.press(TAB)
        assert "PICK>" in sh.screen, sh.screen


@test
def test_menu_colours_are_configurable():
    config = '[menu]\nstyle = "columnar"\n[menu.colors]\ntext = "#123456"\n'
    with shell(rc=WORDS, config=config) as sh:
        sh.type("widget ")
        sh.press(TAB)
        assert sh.style_of("beta").fg == "rgb(18,52,86)", sh.style_of("beta")


@test
def test_the_ide_menu_geometry_is_configurable():
    config = (
        '[menu]\nstyle = "ide"\n'
        "[menu.ide]\nmax_width = 12\nmax_height = 2\nborder = true\n"
    )
    with shell(rc=WORDS, config=config) as sh:
        sh.type("widget ")
        sh.press(TAB)
        box = [r for r in sh.visible_rows() if "\u2500" in r or "alpha" in r or "beta" in r]
        assert box, sh.screen
        assert all(len(r) <= 40 for r in box), box
        assert len(box) <= 4, f"max_height ignored, {len(box)} rows:\n{sh.screen}"


@test
def test_a_multiline_prompt_survives_a_screen_filling_menu():
    with shell(ps1="banner\n$ ", marker="$", rows=10) as sh:
        sh.press(TAB)
        sh.press(ESC)
        assert sh.run("echo survived") == ["survived"]


@test
def test_a_screen_filling_menu_does_not_kill_the_editor():
    with shell(rows=10) as sh:
        sh.press(TAB)
        sh.press(ESC)
        assert sh.run("echo survived") == ["survived"]
