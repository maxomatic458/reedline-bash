import os

from harness import ESC, TAB, complete, ctrl, scratch, shell, test


@test
def test_a_wordlist_compspec_is_offered_in_full():
    assert complete("widget ", rc="complete -W 'red green blue' widget") == [
        "red",
        "green",
        "blue",
    ]


@test
def test_a_compspec_that_declines_does_not_leak_filenames():
    assert complete("widget zz", rc="complete -W 'red green' widget") == []


@test
def test_a_function_compspec_runs_in_the_live_shell():
    rc = "_w() { COMPREPLY=(from_function); }\ncomplete -F _w widget"
    assert complete("widget x", rc=rc) == ["from_function"]


@test
def test_a_command_compspec_reads_an_external_programs_stdout():
    workdir = scratch()
    gen = os.path.join(workdir, "gen")
    with open(gen, "w") as fh:
        fh.write("#!/bin/bash\nprintf '%s\\n' external one\n")
    os.chmod(gen, 0o755)
    assert complete("widget ", rc=f"complete -C {gen} widget") == ["external", "one"]


@test
def test_the_command_word_is_completed_as_a_command():
    rc = "myfunc() { :; }\nalias myalias=ls"
    assert complete("myfu", rc=rc) == ["myfunc"]
    assert complete("myali", rc=rc) == ["myalias"]


@test
def test_a_command_name_is_offered_once_not_once_per_source():
    """`echo` is a builtin and also on `$PATH`, possibly more than once."""
    assert complete("ech").count("echo") == 1


@test
def test_a_command_name_is_not_mistaken_for_a_directory():
    workdir = scratch()
    os.mkdir(os.path.join(workdir, "echo"))
    assert "echo" in complete("ech", rc=f"cd {workdir}")


@test
def test_filenames_and_directories_complete_with_no_compspec():
    workdir = scratch()
    open(os.path.join(workdir, "alpha.txt"), "w").close()
    os.mkdir(os.path.join(workdir, "subdir"))
    rc = f"cd {workdir}"
    assert complete("cat al", rc=rc) == ["alpha.txt"]
    # The trailing slash is what lets the menu chain into the directory.
    assert complete("cat sub", rc=rc) == ["subdir/"]


@test
def test_a_directory_under_a_tilde_is_marked_like_any_other():
    workdir = scratch()
    os.mkdir(os.path.join(workdir, "zztilde"))
    assert complete("cat ~/zztil", rc=f"HOME={workdir}") == ["~/zztilde/"]


@test
def test_a_quoted_or_escaped_word_finds_the_file_it_names():
    workdir = scratch()
    open(os.path.join(workdir, "my file.txt"), "w").close()
    rc = f"cd {workdir}"
    for typed in ["cat 'my fi", 'cat "my fi', "cat my\\ fi"]:
        assert complete(typed, rc=rc) == ["my file.txt"], typed


@test
def test_a_variable_completes_from_the_live_shell():
    assert "$HOME" in complete("echo $HO")


@test
def test_an_assignment_prefix_does_not_hide_the_command():
    """Readline drops `FOO=bar` before choosing a compspec, so taking the first
    word literally would look one up under `FOO=bar` and find none."""
    rc = "complete -W 'alpha beta' widget"
    for line in ["FOO=bar widget a", "A=1 B=2 widget a", "arr[0]=x widget a"]:
        assert complete(line, rc=rc) == ["alpha"], line
    # And a word that only looks like one is still the command.
    assert complete("widget --opt=al", rc=rc) == ["alpha"]


@test
def test_a_command_completes_at_the_start_of_a_continuation_line():
    assert "echo" in complete("ls; ech")
    assert "echo" in complete("for i in 1; do\nech")


@test
def test_completion_is_scoped_to_the_command_under_the_cursor():
    rc = "complete -W 'red green' first\ncomplete -W 'zulu' second"
    assert complete("first r | second z", rc=rc) == ["zulu"]


@test
def test_a_completion_function_sees_the_words_readline_would_give_it():
    rc = (
        "_w() { COMPREPLY=(\"cword=${COMP_CWORD}\" \"cur=${COMP_WORDS[COMP_CWORD]}\"); }\n"
        "complete -F _w widget"
    )
    assert complete("widget --opt=va", rc=rc) == ["cword=3", "cur=va"]


@test
def test_the_real_bash_completion_package_works():
    """The synthetic compspecs above prove the mechanism; this proves it against
    the package people have installed. `dpkg` covers a loaded function, `chmod`
    the `--help`-parsing fallback."""
    assert "--install" in complete("dpkg --insta", bash_completion=True)
    assert "--reference" in complete("chmod --ref", bash_completion=True)
    assert "-HUP" in complete("kill -", bash_completion=True)


@test
def test_a_lazily_loaded_completion_is_loaded_and_then_used():
    """bash-completion registers a `complete -D` loader that returns 124 to ask
    for a retry. The rc asserts the premise: `dpkg` has no compspec until then."""
    rc = "complete -p dpkg >/dev/null 2>&1 && { echo 'preloaded' >&2; exit 1; }"
    assert complete("dpkg --insta", rc=rc, bash_completion=True) == ["--install"]


@test
def test_a_finished_word_does_not_arrive_with_an_escaped_space():
    got = complete("dpkg --insta", bash_completion=True)
    assert got, "no candidates to check"
    assert all(not c.endswith(" ") for c in got), got


@test
def test_tab_inserts_the_only_candidate():
    with shell(rc="complete -W 'solitary' widget") as sh:
        sh.type("widget s")
        sh.press(TAB)
        assert sh.line == "widget solitary"


@test
def test_tab_walks_through_every_candidate():
    rc = "mycmd() { echo PICKED=$1; }\ncomplete -W 'aaa bbb ccc' mycmd"
    for presses, expected in [(1, "aaa"), (2, "bbb"), (3, "ccc")]:
        with shell(rc=rc) as sh:
            sh.type("mycmd ")
            sh.press(*[TAB] * presses)
            sh.press("\r")
            assert sh.run("") == [f"PICKED={expected}"], presses


@test
def test_a_candidate_with_spaces_is_inserted_quoted():
    workdir = scratch()
    with open(os.path.join(workdir, "two words.txt"), "w") as fh:
        fh.write("found\n")
    with shell(cwd=workdir) as sh:
        sh.type("cat two")
        sh.press(TAB)
        assert "\\ " in sh.line or '"' in sh.line or "'" in sh.line, sh.line
        # The quoting is right only if the shell can open the file.
        assert sh.run("") == ["found"]


@test
def test_a_directory_completion_chains_into_it():
    workdir = scratch()
    os.makedirs(os.path.join(workdir, "zzdir", "nested"))
    with shell(cwd=workdir) as sh:
        sh.type("ls zzdi")
        sh.press(TAB)
        assert sh.line == "ls zzdir/", sh.line
        sh.press(TAB)
        assert "nested" in sh.line or "nested" in sh.screen


@test
def test_only_the_word_after_a_break_character_is_replaced():
    """`$COMP_WORDBREAKS` decides where a word starts, which is why
    `--opt=val` completes `val` and not the whole thing. Verified against a
    real readline bash."""
    rc = "complete -W 'auto never always' widget"
    assert complete("widget --color=au", rc=rc) == ["auto"]
    rc2 = "_w() { COMPREPLY=(\"cur=${COMP_WORDS[COMP_CWORD]}\"); }\ncomplete -F _w widget"
    assert complete("widget host:/pa", rc=rc2) == ["cur=/pa"]
    assert complete("widget plain", rc=rc2) == ["cur=plain"]


@test
def test_completion_does_not_disturb_the_shell_it_asks():
    """It runs inside the live shell, so it must leave nothing behind."""
    rc = (
        "MARKER=untouched\n"
        "_w() { COMPREPLY=(x); }\n"
        "complete -F _w widget\n"
        "false\n"
    )
    assert complete("echo $MARKER; echo done", rc=rc) == []
    with shell(rc=rc) as sh:
        sh.type("widget ")
        sh.press(TAB)
        sh.press(ctrl("c"))
        sh.wait_prompt()
        assert sh.run("echo $MARKER") == ["untouched"]


@test
def test_a_repeated_question_is_answered_from_the_memo():
    """Reedline re-asks when a menu reopens, and a compspec for something like
    `git` is not cheap to run again."""
    workdir = scratch()
    counter = os.path.join(workdir, "asks")
    rc = f"_w() {{ echo x >> {counter}; COMPREPLY=(alpha beta); }}\ncomplete -F _w widget"
    with shell(rc=rc) as sh:
        sh.type("widget ")
        sh.press(TAB, ESC, TAB, TAB)
        asked = len(open(counter).read().split())
        assert asked == 1, f"the shell was asked {asked} times"


@test
def test_a_burst_of_input_while_the_menu_is_open_still_submits():
    """Keys arriving faster than the menu redraws must not be dropped."""
    with shell(rc="complete -W 'solitary' widget") as sh:
        sh.send("widget s\t\r")
        sh.wait_prompt()
        assert sh.output() == [], sh.output()
        assert "widget solitary" in sh.text, sh.text


@test
def test_completing_an_empty_line_is_not_slow():
    """Tab on an empty line offers every command on PATH. Timed without a
    terminal, so a loaded machine slows the measurement, not the thing."""
    import time

    started = time.time()
    got = complete("")
    assert len(got) > 100, f"expected every command on PATH, got {len(got)}"
    assert time.time() - started < 5.0


@test
def test_partial_completion_can_be_turned_off():
    rc = "complete -W 'prefix_one prefix_two' widget"
    with shell(rc=rc, config="[completion]\npartial = false\n") as sh:
        sh.type("widget pre")
        sh.press(TAB)
        # Without partial insertion the menu opens instead of extending the word.
        assert "prefix_one" in sh.screen


@test
def test_a_new_prompt_forgets_what_the_last_one_completed():
    """The memo answers a repeated question within one prompt"""
    workdir = scratch()
    for name in ("one/alpha.txt", "two/apple.txt"):
        os.makedirs(os.path.dirname(os.path.join(workdir, name)), exist_ok=True)
        open(os.path.join(workdir, name), "w").close()
    with shell(cwd=workdir) as sh:
        sh.run("cd one")
        sh.type("cat a")
        sh.press(TAB)
        assert sh.line == "cat alpha.txt", sh.line
        sh.press(ctrl("c"))
        sh.wait_prompt()
        sh.run("cd ../two")
        sh.type("cat a")
        sh.press(TAB)
        assert sh.line == "cat apple.txt", sh.line


@test
def test_fignore_hides_the_suffixes_it_names():
    rc = 'cd "$HOME"; touch a.o a.c; FIGNORE=.o'
    assert complete("cat a", rc=rc) == ["a.c"]


@test
def test_a_variable_in_the_directory_part_is_expanded_to_look_inside():
    """Readline asks bash to expand `$D` before reading the directory; the
    match keeps the variable as typed."""
    rc = 'mkdir -p "$HOME/sub"; touch "$HOME/sub/file.txt"; export D="$HOME/sub"'
    assert complete("cat $D/f", rc=rc) == ["$D/file.txt"]


@test
def test_turning_progcomp_off_turns_compspecs_off():
    rc = 'cd "$HOME"; complete -W alpha widget; shopt -u progcomp'
    assert complete("widget a", rc=rc) == []


@test
def test_a_word_a_compspec_finished_with_a_space_ends_the_word():
    """bash-completion's git answers `add ` under `-o nospace`: the space is
    in the word, and readline inserts it. Picking it from the menu must leave
    the cursor past a space too, not glued to the next thing typed."""
    rc = '_w() { COMPREPLY=("add " "am "); compopt -o nospace; }; complete -F _w widget'
    with shell(rc=rc) as sh:
        sh.type("widget a")
        sh.press(TAB)
        sh.press("\r")
        sh.type("--x")
        assert sh.line == "widget add --x", sh.line
