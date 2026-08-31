use reedline::{
    ColumnarMenu, CursorConfig, DefaultHinter, DescriptionMode, DescriptionPosition, EditMode,
    Emacs, Helix, IdeMenu, KeyCode, KeyModifiers, Keybindings, ListMenu, Menu, MenuBuilder,
    MouseClickMode, Osc133ClickEventsMarkers, Osc133Markers, Osc633Markers, Reedline,
    ReedlineEvent, ReedlineMenu, SemanticPromptMarkers, Signal, TraversalDirection, Vi,
    default_emacs_keybindings, default_helix_insert_keybindings, default_helix_normal_keybindings,
    default_helix_select_keybindings, default_vi_insert_keybindings, default_vi_normal_keybindings,
};
use reedline::{InputMode, OutputMode};

use crate::bash::symbols;
use crate::{
    completer::{BashCompleter, BashSource},
    config, highlighter, history,
    prompt::BashPrompt,
    validator::BashValidator,
};
use crossterm::cursor::SetCursorStyle;

pub struct Editor {
    line_editor: Reedline,
    config_stamp: Option<Stamp>,
}

/// Last modified + file size of the config file to detect changes.
type Stamp = (std::time::SystemTime, u64);

impl Editor {
    pub fn new() -> (Self, Vec<config::Warning>) {
        let (settings, warnings) = config::Config::load();
        (
            Editor {
                line_editor: build(&settings),
                config_stamp: config_stamp(),
            },
            warnings,
        )
    }

    /// `None` will exit the shell
    pub fn read_line(&mut self) -> Option<String> {
        self.reload_if_config_changed();
        self.take_terminal();

        loop {
            // Re-expanded each time round: a command may have changed the
            // directory, or anything else the prompt shows.
            let prompt = BashPrompt::new(&expand("PS1"), &expand("PS2"));
            return match self.line_editor.read_line(&prompt) {
                Ok(Signal::Success(line)) => Some(line),
                // reedlines equivalent of bash's `bind -x`:
                Ok(Signal::HostCommand(command)) => {
                    // reedline resumes on the prompt it suspended,
                    // print a newline so the command output doesn't overwrite it.
                    println!();
                    unsafe { symbols::run_host_command(&command) };
                    continue;
                }
                // Abandoned, not submitted: an empty command gets a fresh prompt.
                Ok(Signal::CtrlC) => Some(String::new()),
                Ok(Signal::CtrlD) => None,
                Ok(_) => Some(String::new()),
                Err(_) => None,
            };
        }
    }

    fn reload_if_config_changed(&mut self) {
        let stamp = config_stamp();
        if stamp == self.config_stamp {
            return;
        }
        self.config_stamp = stamp;
        let (settings, warnings) = config::Config::load();
        for warning in &warnings {
            eprintln!("reedline-bash: {warning}");
        }
        self.line_editor = build(&settings);
    }

    /// claim back the terminal from a foreground job that ended.
    fn take_terminal(&self) {
        unsafe {
            if symbols::job_control != 0 {
                symbols::give_terminal_to(symbols::shell_pgrp, 0);
            }
        }
    }
}

/// Expand a prompt variable the way bash does.
fn expand(name: &str) -> String {
    let raw = unsafe { symbols::shell_variable(name) }.unwrap_or_default();
    if raw.is_empty() {
        return raw;
    }
    unsafe { symbols::expand_prompt(&raw) }.unwrap_or(raw)
}

fn config_stamp() -> Option<Stamp> {
    let path = config::path()?;
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

fn build(config: &config::Config) -> Reedline {
    let mut editor = Reedline::create()
        .with_edit_mode(edit_mode(config))
        .with_menu(menu(config))
        .with_highlighter(highlighter::for_config(config.highlight, &config.palette))
        .with_completer(Box::new(BashCompleter::new(BashSource)))
        .with_validator(Box::new(BashValidator))
        .with_history(Box::new(history::BashHistory::new(
            history::BashSource,
            config.history_size,
        )))
        .with_cursor_config(cursor_config(&config.cursor))
        .with_visual_selection_style(config.selection_style)
        .use_bracketed_paste(config.bracketed_paste)
        .use_kitty_keyboard_enhancement(config.kitty_protocol)
        .with_abbreviations(config.abbreviations.clone())
        .with_mouse_click(if config.mouse_click {
            MouseClickMode::Enabled
        } else {
            MouseClickMode::Disabled
        })
        .with_semantic_markers(semantic_markers(config.shell_integration))
        .with_cross_line_cursor(config.cross_line_cursor)
        .with_persistent_menus(config.menu.persistent)
        .with_quick_completions(true)
        .with_partial_completions(config.partial_completions)
        .with_ansi_colors(config.ansi_colors);

    if let Some(style) = config.selection_cursor_style {
        editor = editor.with_visual_selection_cursor_style(style);
    }

    editor = if config.hints {
        editor.with_hinter(Box::new(
            DefaultHinter::default().with_style(config.hint_style),
        ))
    } else {
        editor.disable_hints()
    };

    if !config.buffer_editor.is_empty() {
        let mut parts = config.buffer_editor.split_whitespace();
        if let Some(program) = parts.next() {
            let mut command = std::process::Command::new(program);
            command.args(parts);
            editor = editor.with_buffer_editor(
                command,
                std::env::temp_dir().join("reedline-bash-buffer.sh"),
            );
        }
    }

    editor
}

fn with_completion_keys(mut keybindings: Keybindings) -> Keybindings {
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );
    keybindings
}

fn with_user_keys(mut keybindings: Keybindings, config: &config::Config) -> Keybindings {
    for binding in &config.keybindings {
        keybindings.add_binding(binding.modifiers, binding.key, binding.event.clone());
    }
    keybindings
}

fn edit_mode(config: &config::Config) -> Box<dyn EditMode> {
    let insert = |base| with_user_keys(with_completion_keys(base), config);
    match config.edit_mode {
        config::EditMode::Emacs => Box::new(Emacs::new(insert(default_emacs_keybindings()))),
        config::EditMode::Vi => Box::new(Vi::new(
            insert(default_vi_insert_keybindings()),
            with_user_keys(default_vi_normal_keybindings(), config),
        )),
        config::EditMode::Helix => Box::new(
            Helix::default()
                .with_insert_keybindings(insert(default_helix_insert_keybindings()))
                .with_normal_keybindings(with_user_keys(default_helix_normal_keybindings(), config))
                .with_select_keybindings(with_user_keys(
                    default_helix_select_keybindings(),
                    config,
                )),
        ),
    }
}

fn semantic_markers(kind: config::ShellIntegration) -> Option<Box<dyn SemanticPromptMarkers>> {
    match kind {
        config::ShellIntegration::Off => None,
        config::ShellIntegration::Osc133 => Some(Osc133Markers::boxed()),
        config::ShellIntegration::Osc133ClickEvents => Some(Osc133ClickEventsMarkers::boxed()),
        config::ShellIntegration::Osc633 => Some(Osc633Markers::boxed()),
    }
}

fn cursor_config(shapes: &config::CursorShapes) -> CursorConfig {
    let shape = |s: config::CursorShape| match s {
        config::CursorShape::Inherit => None,
        config::CursorShape::Block => Some(SetCursorStyle::SteadyBlock),
        config::CursorShape::Underscore => Some(SetCursorStyle::SteadyUnderScore),
        config::CursorShape::Line => Some(SetCursorStyle::SteadyBar),
        config::CursorShape::BlinkBlock => Some(SetCursorStyle::BlinkingBlock),
        config::CursorShape::BlinkUnderscore => Some(SetCursorStyle::BlinkingUnderScore),
        config::CursorShape::BlinkLine => Some(SetCursorStyle::BlinkingBar),
    };
    CursorConfig {
        emacs: shape(shapes.emacs),
        vi_insert: shape(shapes.vi_insert),
        vi_normal: shape(shapes.vi_normal),
        hx_insert: shape(shapes.helix_insert),
        hx_normal: shape(shapes.helix_normal),
        hx_select: shape(shapes.helix_select),
    }
}

fn marker_for(configured: &str) -> &str {
    if configured.is_empty() {
        // Not `""`: reedline subtracts 1 so an empty one underflows and panics.
        //
        // this is a bug in reedline i think
        "\u{1b}[0m"
    } else {
        configured
    }
}

/// Settings every menu type shares.
fn shared<M: MenuBuilder>(menu: M, settings: &config::MenuConfig) -> M {
    let colors = &settings.colors;
    menu.with_input_mode(match settings.input_mode {
        config::InputMode::Diff => InputMode::Diff,
        config::InputMode::CursorPrefix => InputMode::CursorPrefix,
        config::InputMode::FullBuffer => InputMode::FullBuffer,
    })
    .with_output_mode(match settings.output_mode {
        config::OutputMode::SuggestedSpan => OutputMode::SuggestedSpan,
        config::OutputMode::FullBuffer => OutputMode::FullBuffer,
        config::OutputMode::ExtendToEnd => OutputMode::ExtendToEnd,
    })
    .with_text_style(colors.text)
    .with_selected_text_style(colors.selected_text)
    .with_description_text_style(colors.description)
    .with_match_text_style(colors.matched)
    .with_selected_match_text_style(colors.selected_match)
}

fn menu(config: &config::Config) -> ReedlineMenu {
    let name = "completion_menu";
    let menu = &config.menu;
    let marker = marker_for(&menu.marker);
    let inner: Box<dyn Menu> = match menu.style {
        config::MenuStyle::Columnar => {
            let it = &menu.columnar;
            Box::new(
                shared(ColumnarMenu::default(), menu)
                    .with_name(name)
                    .with_marker(marker)
                    .with_columns(it.columns)
                    .with_column_width(it.col_width)
                    .with_column_padding(it.col_padding)
                    .with_traversal_direction(match it.traversal {
                        config::Traversal::Horizontal => TraversalDirection::Horizontal,
                        config::Traversal::Vertical => TraversalDirection::Vertical,
                    }),
            )
        }
        config::MenuStyle::Ide => {
            let it = &menu.ide;
            let built = shared(IdeMenu::default(), menu)
                .with_name(name)
                .with_marker(marker)
                .with_min_completion_width(it.min_width)
                .with_max_completion_width(it.max_width)
                .with_max_completion_height(it.max_height)
                .with_padding(it.padding)
                .with_cursor_offset(it.cursor_offset)
                .with_description_mode(match it.description_mode {
                    config::DescriptionSide::Left => DescriptionMode::Left,
                    config::DescriptionSide::Right => DescriptionMode::Right,
                    config::DescriptionSide::PreferRight => DescriptionMode::PreferRight,
                })
                .with_min_description_width(it.min_description_width)
                .with_max_description_width(it.max_description_width)
                .with_max_description_height(it.max_description_height)
                .with_description_offset(it.description_offset)
                .with_correct_cursor_pos(it.correct_cursor_pos);
            Box::new(if it.border {
                let b = &it.border_symbols;
                built.with_border(
                    b.top_right,
                    b.top_left,
                    b.bottom_right,
                    b.bottom_left,
                    b.horizontal,
                    b.vertical,
                )
            } else {
                built
            })
        }
        config::MenuStyle::List => {
            let it = &menu.list;
            Box::new(
                shared(ListMenu::default(), menu)
                    .with_name(name)
                    .with_marker(marker)
                    .with_page_size(it.page_size)
                    .with_max_entry_lines(it.max_entry_lines)
                    .with_description_position(match it.description_position {
                        config::DescriptionPlace::Before => DescriptionPosition::Before,
                        config::DescriptionPlace::After => DescriptionPosition::After,
                    }),
            )
        }
    };
    ReedlineMenu::EngineCompleter(inner)
}
