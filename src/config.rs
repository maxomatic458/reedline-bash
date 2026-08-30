//! The configuration file for reedline-bash.
//!
//! This is basically a TOML version of the relevant reedline/menu settings from
//! nushells `config.nu`

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use nu_ansi_term::{Color, Style};
use reedline::{KeyCode, KeyModifiers, ReedlineEvent};
use serde::Deserialize;
use strum::{Display, EnumString, VariantNames};

use crate::{keys, style};

/// Path of the config file.
///
/// - `None` being defaults.
pub fn path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("REEDLINE_BASH_CONFIG")
        && !explicit.is_empty()
    {
        return Some(PathBuf::from(explicit));
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Some(Path::new(&xdg).join("reedline-bash/config.toml"));
    }
    let home = std::env::var("HOME").ok()?;
    Some(Path::new(&home).join(".config/reedline-bash/config.toml"))
}

/// a warning about a invalid config setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning(pub String);

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum EditMode {
    #[default]
    Emacs,
    Vi,
    Helix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum MenuStyle {
    #[default]
    Columnar,
    Ide,
    List,
}

/// The direction of the description box
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum DescriptionSide {
    /// Description is always shown on the left
    Left,
    /// Description is always shown on the right
    Right,
    /// Description is shown on the right of the completion if there is enough
    /// space, otherwise it is shown on the left
    #[default]
    PreferRight,
}

/// The traversal direction of the menu
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum Traversal {
    /// Traverse horizontally
    #[default]
    Horizontal,
    /// Traverse vertically
    Vertical,
}

/// Controls where the description is rendered relative to the completion value
/// in a list menu row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum DescriptionPlace {
    /// Description is shown **before** the value, wrapped in parentheses:
    /// `(description) value`
    #[default]
    Before,
    /// Description is shown **after** the value with a leading space:
    /// `value description`
    After,
}

/// Settings for the columnar menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnarConfig {
    /// Number of columns that the menu will have
    pub columns: u16,
    /// Column width. `None` uses the whole screen width to calculate it, which
    /// the config file spells `0`.
    pub col_width: Option<usize>,
    /// Column padding
    pub col_padding: usize,
    /// Traversal direction
    pub traversal: Traversal,
}

impl Default for ColumnarConfig {
    fn default() -> Self {
        ColumnarConfig {
            columns: 4,
            col_width: None,
            col_padding: 2,
            traversal: Traversal::Horizontal,
        }
    }
}

/// Settings for the ide menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeConfig {
    /// Min width of the completion box, including the border
    pub min_width: u16,
    /// Max width of the completion box, including the border
    pub max_width: u16,
    /// Max height of the completion box, including the border
    /// this will be capped by the lines available in the terminal
    ///
    /// `u16::MAX` is reedline's "as many rows as the screen allows", which the
    /// config file spells `0`.
    pub max_height: u16,
    /// Padding to the left and right of the suggestions
    pub padding: u16,
    /// Whether the menu has a border or not
    pub border: bool,
    /// Horizontal offset from the cursor.
    /// 0 means the top left corner of the menu is below the cursor
    pub cursor_offset: i16,
    /// How the description is shown
    pub description_mode: DescriptionSide,
    /// Min width of the description, including the border
    /// this will be applied, when the description is "squished"
    /// by the completion box
    pub min_description_width: u16,
    /// Max width of the description, including the border
    pub max_description_width: u16,
    /// Max height of the description, including the border
    pub max_description_height: u16,
    /// Offset from the suggestion box to the description box
    pub description_offset: u16,
    /// If true, the cursor pos will be corrected, so the suggestions match up
    /// with the typed text
    /// ```text
    /// C:\> str
    ///      str join
    ///      str trim
    ///      str split
    /// ```
    pub correct_cursor_pos: bool,
    /// The characters the border is drawn with, when `border` is on. Reedline
    /// carries these on `border` itself, as `Option<BorderSymbols>`.
    pub border_symbols: BorderSymbols,
}

/// Symbols used for the border of the menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderSymbols {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
}

impl Default for BorderSymbols {
    fn default() -> Self {
        // Reedline's own defaults.
        BorderSymbols {
            top_left: '╭',
            top_right: '╮',
            bottom_left: '╰',
            bottom_right: '╯',
            horizontal: '─',
            vertical: '│',
        }
    }
}

impl Default for IdeConfig {
    fn default() -> Self {
        // Reedline's defaults, restated so `config.example.toml` can show them.
        IdeConfig {
            min_width: 0,
            max_width: 50,
            max_height: u16::MAX,
            padding: 0,
            border: false,
            cursor_offset: 0,
            description_mode: DescriptionSide::PreferRight,
            min_description_width: 15,
            max_description_width: 50,
            max_description_height: 10,
            description_offset: 1,
            correct_cursor_pos: false,
            border_symbols: BorderSymbols::default(),
        }
    }
}

/// Settings that only the list menu has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListConfig {
    /// Number of records pulled until page is full
    pub page_size: usize,
    /// Max number of lines that are shown with large suggestions entries
    pub max_entry_lines: u16,
    /// Where descriptions are rendered relative to the completion value
    pub description_position: DescriptionPlace,
}

impl Default for ListConfig {
    fn default() -> Self {
        ListConfig {
            page_size: 10,
            max_entry_lines: 5,
            description_position: DescriptionPlace::Before,
        }
    }
}

/// Which menu, plus the settings for each kind. Each kind gets its own table,
/// since a flat one invites setting an option the chosen menu ignores.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MenuConfig {
    pub style: MenuStyle,
    /// Keep an open menu alive while the line is edited, instead of dismissing
    /// it on a backspace or an emptied line.
    pub persistent: bool,
    pub marker: String,
    pub input_mode: InputMode,
    pub output_mode: OutputMode,
    pub colors: MenuColors,
    pub columnar: ColumnarConfig,
    pub ide: IdeConfig,
    pub list: ListConfig,
}

/// What the menu sends to the completer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum InputMode {
    /// Only the text typed after the menu opened.
    Diff,
    /// The buffer up to the cursor.
    #[default]
    CursorPrefix,
    /// The whole buffer, including text after the cursor.
    FullBuffer,
}

/// What range of the buffer a selected suggestion replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum OutputMode {
    /// The range the completer asked for.
    #[default]
    SuggestedSpan,
    /// The whole buffer.
    FullBuffer,
    /// From the start of the suggested range to the end of the buffer.
    ExtendToEnd,
}

/// Cursor shapes, named the same as in nushell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum CursorShape {
    /// Leave whatever shape the terminal was already using.
    #[default]
    Inherit,
    Block,
    Underscore,
    Line,
    BlinkBlock,
    BlinkUnderscore,
    BlinkLine,
}

/// A shape per edit mode. `None` is "inherit": leave the terminal's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorShapes {
    pub emacs: CursorShape,
    pub vi_insert: CursorShape,
    pub vi_normal: CursorShape,
    pub helix_insert: CursorShape,
    pub helix_normal: CursorShape,
    pub helix_select: CursorShape,
}

/// Menu styling, mirroring reedline's `MenuTextStyle`.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuColors {
    pub text: Style,
    pub selected_text: Style,
    pub description: Style,
    pub matched: Style,
    pub selected_match: Style,
}

impl Default for MenuColors {
    fn default() -> Self {
        // Reedline's defaults
        MenuColors {
            text: Style::new().fg(Color::Green),
            selected_text: Style::new().fg(Color::Green).reverse(),
            description: Style::new().fg(Color::Yellow),
            matched: Style::new().fg(Color::Green).bold(),
            selected_match: Style::new().fg(Color::Green).bold().reverse(),
        }
    }
}

/// A key and what it should do.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub modifiers: KeyModifiers,
    pub key: KeyCode,
    pub event: ReedlineEvent,
}

/// Colours for the syntax highlighter.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub command: Style,
    pub builtin: Style,
    pub string: Style,
    pub variable: Style,
    pub operator: Style,
    pub comment: Style,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            command: Style::new().fg(Color::LightGreen),
            builtin: Style::new().fg(Color::LightGreen).bold(),
            string: Style::new().fg(Color::LightYellow),
            variable: Style::new().fg(Color::LightCyan),
            operator: Style::new().fg(Color::LightPurple),
            comment: Style::new().fg(Color::DarkGray).italic(),
        }
    }
}

/// Resolved settings, every field usable as-is.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    // editor
    pub edit_mode: EditMode,
    /// Whether a block caret crosses line boundaries on `h`/`l`. Vi and helix
    /// normal mode only; a bar caret always moves freely.
    pub cross_line_cursor: bool,
    pub shell_integration: ShellIntegration,
    pub highlight: bool,
    pub hints: bool,
    pub bracketed_paste: bool,
    pub kitty_protocol: bool,
    pub ansi_colors: bool,
    pub mouse_click: bool,
    /// Command that `Ctrl-O` opens the line in. Empty disables it.
    pub buffer_editor: String,

    // cursor
    pub cursor: CursorShapes,

    pub menu: MenuConfig,

    // colours
    pub palette: Palette,
    pub hint_style: Style,
    pub selection_style: Style,
    pub selection_cursor_style: Option<Style>,

    // completion
    pub partial_completions: bool,

    // history
    pub history_size: usize,
    pub history_ignore_prefix: Option<String>,

    pub abbreviations: HashMap<String, String>,
    pub keybindings: Vec<Binding>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            edit_mode: EditMode::Emacs,
            cross_line_cursor: true,
            shell_integration: ShellIntegration::Off,
            highlight: true,
            hints: true,
            bracketed_paste: true,
            kitty_protocol: false,
            ansi_colors: true,
            mouse_click: false,
            buffer_editor: String::new(),
            cursor: CursorShapes::default(),
            menu: MenuConfig::default(),
            palette: Palette::default(),
            hint_style: Style::new().fg(Color::DarkGray),
            selection_style: Style::new().reverse(),
            selection_cursor_style: None,
            partial_completions: true,
            history_size: 5000,
            // Bashs HISTCONTROL=ignorespace.
            history_ignore_prefix: Some(" ".to_string()),
            abbreviations: HashMap::new(),
            keybindings: Vec::new(),
        }
    }
}

impl Config {
    pub fn load() -> (Config, Vec<Warning>) {
        let Some(path) = path() else {
            return (Config::default(), Vec::new());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Config::parse(&text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                (Config::default(), Vec::new())
            }
            Err(err) => (
                Config::default(),
                vec![Warning(format!("cannot read {}: {err}", path.display()))],
            ),
        }
    }

    /// Parse TOML into settings, collecting anything that could not be honoured.
    pub fn parse(text: &str) -> (Config, Vec<Warning>) {
        let raw: RawConfig = match toml::from_str(text) {
            Ok(raw) => raw,
            // Only a key with the wrong type or broken syntax gets this far;
            // an unrecognised key is collected per section instead.
            Err(err) => {
                let message = err.message().to_string();
                return (
                    Config::default(),
                    vec![Warning(format!(
                        "config unreadable, using defaults: {message}"
                    ))],
                );
            }
        };

        let mut notes = Vec::new();
        note_unknown(&raw, &mut notes);
        let mut config = Config::default();

        config.apply_editor(&raw.editor, &mut notes);
        config.cursor = cursor_config(&raw.cursor, &mut notes);
        config.apply_menu(&raw.menu, &mut notes);
        config.apply_colors(&raw.colors, &mut notes);

        if let Some(partial) = raw.completion.partial {
            config.partial_completions = partial;
        }
        if raw.completion.quick.is_some() {
            notes.push("completion.quick is not configurable: Tab stepping depends on it".into());
        }

        assign_positive(
            &mut config.history_size,
            raw.history.size,
            "history.size",
            &mut notes,
        );
        if let Some(prefix) = raw.history.ignore_prefix {
            config.history_ignore_prefix = if prefix.is_empty() {
                None
            } else {
                Some(prefix)
            };
        }

        config.abbreviations = raw.abbreviations;
        config.keybindings = bindings(raw.keybinding, &mut notes);

        (config, notes.into_iter().map(Warning).collect())
    }

    fn apply_editor(&mut self, raw: &RawEditor, notes: &mut Vec<String>) {
        assign_enum(&mut self.edit_mode, raw.mode.as_ref(), "editor.mode", notes);
        assign_enum(
            &mut self.shell_integration,
            raw.shell_integration.as_ref(),
            "editor.shell_integration",
            notes,
        );
        assign(&mut self.cross_line_cursor, raw.cross_line_cursor);
        assign(&mut self.highlight, raw.highlight);
        assign(&mut self.hints, raw.hints);
        assign(&mut self.bracketed_paste, raw.bracketed_paste);
        assign(&mut self.kitty_protocol, raw.kitty_protocol);
        assign(&mut self.ansi_colors, raw.ansi_colors);
        assign(&mut self.mouse_click, raw.mouse_click);
        if let Some(editor) = &raw.buffer_editor {
            self.buffer_editor = editor.clone();
        }
    }

    fn apply_menu(&mut self, raw: &RawMenu, notes: &mut Vec<String>) {
        assign_enum(
            &mut self.menu.style,
            raw.style.as_ref(),
            "menu.style",
            notes,
        );
        assign(&mut self.menu.persistent, raw.persistent);
        if let Some(marker) = &raw.marker {
            self.menu.marker = marker.clone();
        }
        assign_enum(
            &mut self.menu.input_mode,
            raw.input_mode.as_ref(),
            "menu.input_mode",
            notes,
        );
        assign_enum(
            &mut self.menu.output_mode,
            raw.output_mode.as_ref(),
            "menu.output_mode",
            notes,
        );

        let columnar = &raw.columnar;
        // A zero-column grid reaches a division inside the layout.
        assign_positive(
            &mut self.menu.columnar.columns,
            columnar.columns,
            "menu.columnar.columns",
            notes,
        );
        if let Some(width) = columnar.col_width {
            // 0 is how a config file spells "work it out yourself".
            self.menu.columnar.col_width = (width > 0).then_some(width);
        }
        assign(&mut self.menu.columnar.col_padding, columnar.col_padding);
        assign_enum(
            &mut self.menu.columnar.traversal,
            columnar.traversal.as_ref(),
            "menu.columnar.traversal",
            notes,
        );

        let ide = &raw.ide;
        assign(&mut self.menu.ide.min_width, ide.min_width);
        assign_positive(
            &mut self.menu.ide.max_width,
            ide.max_width,
            "menu.ide.max_width",
            notes,
        );
        if let Some(height) = ide.max_height {
            // 0 spells "as many rows as fit", which reedline reads as u16::MAX.
            self.menu.ide.max_height = if height == 0 { u16::MAX } else { height };
        }
        assign(&mut self.menu.ide.padding, ide.padding);
        assign(&mut self.menu.ide.border, ide.border);
        assign(&mut self.menu.ide.cursor_offset, ide.cursor_offset);
        assign_enum(
            &mut self.menu.ide.description_mode,
            ide.description_mode.as_ref(),
            "menu.ide.description_mode",
            notes,
        );
        assign(
            &mut self.menu.ide.min_description_width,
            ide.min_description_width,
        );
        assign(
            &mut self.menu.ide.max_description_width,
            ide.max_description_width,
        );
        assign(
            &mut self.menu.ide.max_description_height,
            ide.max_description_height,
        );
        assign(
            &mut self.menu.ide.description_offset,
            ide.description_offset,
        );
        assign(
            &mut self.menu.ide.correct_cursor_pos,
            ide.correct_cursor_pos,
        );
        if let Some(border) = &ide.border_symbols {
            let symbols = &mut self.menu.ide.border_symbols;
            assign(&mut symbols.top_left, border.top_left);
            assign(&mut symbols.top_right, border.top_right);
            assign(&mut symbols.bottom_left, border.bottom_left);
            assign(&mut symbols.bottom_right, border.bottom_right);
            assign(&mut symbols.horizontal, border.horizontal);
            assign(&mut symbols.vertical, border.vertical);
        }

        let list = &raw.list;
        assign_positive(
            &mut self.menu.list.page_size,
            list.page_size,
            "menu.list.page_size",
            notes,
        );
        assign(&mut self.menu.list.max_entry_lines, list.max_entry_lines);
        assign_enum(
            &mut self.menu.list.description_position,
            list.description_position.as_ref(),
            "menu.list.description_position",
            notes,
        );

        let colors = &raw.colors;
        let base = MenuColors::default();
        self.menu.colors = MenuColors {
            text: style::parse_or(colors.text.as_ref(), base.text, notes),
            selected_text: style::parse_or(
                colors.selected_text.as_ref(),
                base.selected_text,
                notes,
            ),
            description: style::parse_or(colors.description.as_ref(), base.description, notes),
            matched: style::parse_or(colors.r#match.as_ref(), base.matched, notes),
            selected_match: style::parse_or(
                colors.selected_match.as_ref(),
                base.selected_match,
                notes,
            ),
        };
    }

    fn apply_colors(&mut self, raw: &RawColors, notes: &mut Vec<String>) {
        let base = Palette::default();
        self.palette = Palette {
            command: style::parse_or(raw.command.as_ref(), base.command, notes),
            builtin: style::parse_or(raw.builtin.as_ref(), base.builtin, notes),
            string: style::parse_or(raw.string.as_ref(), base.string, notes),
            variable: style::parse_or(raw.variable.as_ref(), base.variable, notes),
            operator: style::parse_or(raw.operator.as_ref(), base.operator, notes),
            comment: style::parse_or(raw.comment.as_ref(), base.comment, notes),
        };
        self.hint_style =
            style::parse_or(raw.hint.as_ref(), Style::new().fg(Color::DarkGray), notes);
        self.selection_style =
            style::parse_or(raw.selection.as_ref(), Style::new().reverse(), notes);
        // Absent means "same as the selection", empty means the empty style.
        self.selection_cursor_style = match &raw.selection_cursor {
            None => None,
            Some(spec) if spec.is_empty() => None,
            Some(spec) => Some(style::parse_or(Some(spec), self.selection_style, notes)),
        };
    }
}

/// Report every key the file used that no setting matches.
///
/// A typo is worth saying out loud, but not worth throwing the rest of the file
/// away over -- so each section keeps what it did recognise.
fn note_unknown(raw: &RawConfig, notes: &mut Vec<String>) {
    let mut sections: Vec<(&str, &HashMap<String, toml::Value>)> = vec![
        ("", &raw.unknown),
        ("editor.", &raw.editor.unknown),
        ("cursor.", &raw.cursor.unknown),
        ("menu.", &raw.menu.unknown),
        ("menu.columnar.", &raw.menu.columnar.unknown),
        ("menu.ide.", &raw.menu.ide.unknown),
        ("menu.list.", &raw.menu.list.unknown),
        ("menu.colors.", &raw.menu.colors.unknown),
        ("colors.", &raw.colors.unknown),
        ("completion.", &raw.completion.unknown),
        ("history.", &raw.history.unknown),
    ];
    if let Some(border) = &raw.menu.ide.border_symbols {
        sections.push(("menu.ide.border_symbols.", &border.unknown));
    }
    for binding in &raw.keybinding {
        sections.push(("keybinding.", &binding.unknown));
    }

    for (section, unknown) in sections {
        let mut keys: Vec<&str> = unknown.keys().map(String::as_str).collect();
        keys.sort_unstable();
        for key in keys {
            notes.push(format!("{section}{key} is not a setting; ignoring it"));
        }
    }
}

fn assign<T: Copy>(field: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *field = value;
    }
}

/// Apply a value that must not be zero
fn assign_positive<T>(field: &mut T, value: Option<T>, path: &str, notes: &mut Vec<String>)
where
    T: Copy + Default + PartialEq + fmt::Display,
{
    let Some(value) = value else { return };
    if value == T::default() {
        notes.push(format!("{path} must be at least 1; using {field}"));
    } else {
        *field = value;
    }
}

/// Apply one of a fixed set of names, case- and dash-insensitively.
fn assign_enum<T>(field: &mut T, value: Option<&String>, path: &str, notes: &mut Vec<String>)
where
    T: FromStr + VariantNames + fmt::Display,
{
    let Some(value) = value else { return };
    // `-` and `_` are interchangeable
    match T::from_str(&value.replace('-', "_")) {
        Ok(chosen) => *field = chosen,
        Err(_) => notes.push(format!(
            "{path} {value:?} is not one of {}; using {field}",
            T::VARIANTS.join(", "),
        )),
    }
}

/// Escape sequences marking where the prompt and command begin and end, so a
/// terminal can offer things like jump-to-previous-prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, EnumString, VariantNames, Display)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
pub enum ShellIntegration {
    #[default]
    Off,
    /// Supported by Ghostty, WezTerm and Kitty.
    #[strum(serialize = "osc133")]
    Osc133,
    /// OSC 133, plus asking the terminal to report mouse clicks so it can
    /// offer click-to-cursor.
    #[strum(serialize = "osc133_click_events")]
    Osc133ClickEvents,
    /// VS Code's extension of OSC 133, which also reports the command line
    /// and the working directory.
    #[strum(serialize = "osc633")]
    Osc633,
}

fn cursor_config(raw: &RawCursor, notes: &mut Vec<String>) -> CursorShapes {
    let mut config = CursorShapes::default();
    for (spec, slot, name) in [
        (&raw.emacs, &mut config.emacs, "emacs"),
        (&raw.vi_insert, &mut config.vi_insert, "vi_insert"),
        (&raw.vi_normal, &mut config.vi_normal, "vi_normal"),
        (&raw.helix_insert, &mut config.helix_insert, "helix_insert"),
        (&raw.helix_normal, &mut config.helix_normal, "helix_normal"),
        (&raw.helix_select, &mut config.helix_select, "helix_select"),
    ] {
        assign_enum(slot, spec.as_ref(), &format!("cursor.{name}"), notes);
    }
    config
}

fn bindings(raw: Vec<RawBinding>, notes: &mut Vec<String>) -> Vec<Binding> {
    let mut out = Vec::new();
    for entry in raw {
        let (modifiers, key) = match keys::parse(&entry.key) {
            Ok(parsed) => parsed,
            Err(message) => {
                notes.push(format!("keybinding: {message}"));
                continue;
            }
        };

        // `event` and `edit` are the same.
        let value = match (entry.event, entry.edit) {
            (Some(_), Some(_)) => {
                notes.push(format!(
                    "keybinding {:?}: set either event or edit, not both",
                    entry.key
                ));
                continue;
            }
            (Some(event), None) => event.try_into::<ReedlineEvent>().map_err(|e| e.to_string()),
            (None, Some(edit)) => toml::Value::Array(edit)
                .try_into::<Vec<reedline::EditCommand>>()
                .map(ReedlineEvent::Edit)
                .map_err(|e| e.to_string()),
            (None, None) => {
                notes.push(format!(
                    "keybinding {:?}: no event or edit given",
                    entry.key
                ));
                continue;
            }
        };

        match value {
            Ok(event) => out.push(Binding {
                modifiers,
                key,
                event,
            }),
            Err(message) => notes.push(format!("keybinding {:?}: {message}", entry.key)),
        }
    }
    out
}

#[derive(Debug, Default, Deserialize)]
struct RawConfig {
    #[serde(default)]
    editor: RawEditor,
    #[serde(default)]
    cursor: RawCursor,
    #[serde(default)]
    menu: RawMenu,
    #[serde(default)]
    colors: RawColors,
    #[serde(default)]
    completion: RawCompletion,
    #[serde(default)]
    history: RawHistory,
    #[serde(default)]
    abbreviations: HashMap<String, String>,
    #[serde(default)]
    keybinding: Vec<RawBinding>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawEditor {
    mode: Option<String>,
    cross_line_cursor: Option<bool>,
    shell_integration: Option<String>,
    highlight: Option<bool>,
    hints: Option<bool>,
    bracketed_paste: Option<bool>,
    kitty_protocol: Option<bool>,
    ansi_colors: Option<bool>,
    mouse_click: Option<bool>,
    buffer_editor: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCursor {
    emacs: Option<String>,
    vi_insert: Option<String>,
    vi_normal: Option<String>,
    helix_insert: Option<String>,
    helix_normal: Option<String>,
    helix_select: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMenu {
    style: Option<String>,
    persistent: Option<bool>,
    marker: Option<String>,
    input_mode: Option<String>,
    output_mode: Option<String>,
    #[serde(default)]
    colors: RawMenuColors,
    #[serde(default)]
    columnar: RawColumnar,
    #[serde(default)]
    ide: RawIde,
    #[serde(default)]
    list: RawList,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawColumnar {
    columns: Option<u16>,
    col_width: Option<usize>,
    col_padding: Option<usize>,
    traversal: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawIde {
    border_symbols: Option<RawBorderSymbols>,
    min_width: Option<u16>,
    max_width: Option<u16>,
    max_height: Option<u16>,
    padding: Option<u16>,
    border: Option<bool>,
    cursor_offset: Option<i16>,
    description_mode: Option<String>,
    min_description_width: Option<u16>,
    max_description_width: Option<u16>,
    max_description_height: Option<u16>,
    description_offset: Option<u16>,
    correct_cursor_pos: Option<bool>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawBorderSymbols {
    top_left: Option<char>,
    top_right: Option<char>,
    bottom_left: Option<char>,
    bottom_right: Option<char>,
    horizontal: Option<char>,
    vertical: Option<char>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawList {
    page_size: Option<usize>,
    max_entry_lines: Option<u16>,
    description_position: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMenuColors {
    text: Option<String>,
    selected_text: Option<String>,
    description: Option<String>,
    r#match: Option<String>,
    selected_match: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawColors {
    command: Option<String>,
    builtin: Option<String>,
    string: Option<String>,
    variable: Option<String>,
    operator: Option<String>,
    comment: Option<String>,
    hint: Option<String>,
    selection: Option<String>,
    selection_cursor: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCompletion {
    partial: Option<bool>,
    quick: Option<bool>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawHistory {
    size: Option<usize>,
    ignore_prefix: Option<String>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct RawBinding {
    key: String,
    event: Option<toml::Value>,
    edit: Option<Vec<toml::Value>>,
    #[serde(flatten)]
    unknown: HashMap<String, toml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> (Config, Vec<String>) {
        let (config, warnings) = Config::parse(text);
        (config, warnings.into_iter().map(|w| w.0).collect())
    }

    #[test]
    fn an_empty_config_is_all_defaults_and_says_nothing() {
        let (config, warnings) = parse("");
        assert_eq!(config, Config::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn settings_are_read_from_their_sections() {
        let (config, warnings) = parse(
            r#"
            [editor]
            mode = "vi"
            hints = false
            bracketed_paste = false
            buffer_editor = "vim"

            [menu]
            style = "ide"

            [menu.columnar]
            columns = 2

            [menu.ide]
            border = true

            [history]
            size = 100
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.edit_mode, EditMode::Vi);
        assert!(!config.hints);
        assert!(!config.bracketed_paste);
        assert_eq!(config.buffer_editor, "vim");
        assert_eq!(config.menu.style, MenuStyle::Ide);
        assert_eq!(config.menu.columnar.columns, 2);
        assert!(config.menu.ide.border);
        assert_eq!(config.history_size, 100);
        // Untouched fields keep their defaults rather than being reset.
        assert!(config.highlight);
        assert_eq!(config.menu.columnar.col_padding, 2);
        assert!(config.ansi_colors);
    }

    #[test]
    fn an_unknown_enum_value_falls_back_and_keeps_the_rest_of_the_file() {
        let (config, warnings) = parse(
            r#"
            [editor]
            mode = "vim"
            highlight = false
            "#,
        );
        assert_eq!(config.edit_mode, EditMode::Emacs, "fell back");
        assert!(
            !config.highlight,
            "the valid setting beside it still applied"
        );
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("vim"), "{warnings:?}");
    }

    #[test]
    fn values_that_would_divide_by_zero_are_refused() {
        for (text, field) in [
            ("[menu.columnar]\ncolumns = 0\n", "columns"),
            ("[menu.ide]\nmax_width = 0\n", "max_width"),
            ("[menu.list]\npage_size = 0\n", "page_size"),
            ("[history]\nsize = 0\n", "size"),
        ] {
            let (config, warnings) = parse(text);
            assert_eq!(warnings.len(), 1, "{field}: {warnings:?}");
            assert!(warnings[0].contains(field), "{warnings:?}");
            assert_eq!(config, Config::default(), "{field} should not have applied");
        }
    }

    #[test]
    fn a_zero_column_width_means_let_the_menu_decide() {
        let (config, warnings) = parse("[menu.columnar]\ncol_width = 0\n");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.menu.columnar.col_width, None);
        let (config, _) = parse("[menu.columnar]\ncol_width = 20\n");
        assert_eq!(config.menu.columnar.col_width, Some(20));
    }

    #[test]
    fn broken_toml_gives_defaults_and_one_complaint() {
        let (config, warnings) = parse("[editor\nmode = ");
        assert_eq!(config, Config::default());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("unreadable"), "{warnings:?}");
    }

    #[test]
    fn a_misspelt_key_is_reported_and_the_rest_of_the_file_still_applies() {
        let (config, warnings) =
            parse("[editor]\nmode = \"vi\"\nhighlite = true\n\n[colors]\ncommand = \"red\"\n");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("editor.highlite"), "{warnings:?}");
        assert_eq!(config.edit_mode, EditMode::Vi);
        assert_eq!(config.palette.command, Style::new().fg(Color::Red));
    }

    #[test]
    fn an_unknown_key_is_named_with_the_section_it_was_written_in() {
        for (text, expected) in [
            ("[menu.ide]\nbrdr = 1\n", "menu.ide.brdr"),
            ("[menu.columnar]\ncols = 1\n", "menu.columnar.cols"),
            ("[history]\nmax = 1\n", "history.max"),
            ("[nonsense]\nx = 1\n", "nonsense"),
        ] {
            let (_, warnings) = parse(text);
            assert_eq!(warnings.len(), 1, "{text:?}");
            assert!(warnings[0].contains(expected), "{text:?}: {warnings:?}");
        }
    }

    #[test]
    fn a_key_of_the_wrong_type_falls_back_to_the_defaults_for_the_file() {
        let (config, warnings) = parse("[editor]\nmode = \"vi\"\nhighlight = \"yes\"\n");
        assert_eq!(config, Config::default(), "the whole file is dropped");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("boolean"), "{warnings:?}");
    }

    #[test]
    fn the_flag_tab_depends_on_is_refused_with_a_reason() {
        let (config, warnings) = parse("[completion]\nquick = false\n");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Tab"), "{warnings:?}");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn an_empty_ignore_prefix_means_no_exclusion_rather_than_excluding_everything() {
        let (config, _) = parse("[history]\nignore_prefix = \"\"\n");
        assert_eq!(config.history_ignore_prefix, None);
    }

    /// Each menu keeps its own table, so a setting cannot be aimed at the wrong
    /// menu -- and setting one leaves the other two alone.
    #[test]
    fn each_menu_kind_reads_only_its_own_table() {
        let (config, warnings) = parse(
            r#"
            [menu.columnar]
            columns = 7

            [menu.ide]
            border = true
            max_description_width = 30

            [menu.list]
            page_size = 3
            description_position = "after"
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.menu.columnar.columns, 7);
        assert!(config.menu.ide.border);
        assert_eq!(config.menu.ide.max_description_width, 30);
        assert_eq!(config.menu.list.page_size, 3);
        assert_eq!(
            config.menu.list.description_position,
            DescriptionPlace::After
        );
        // Untouched neighbours keep reedline's defaults.
        assert_eq!(config.menu.columnar.col_padding, 2);
        assert_eq!(config.menu.ide.max_width, 50);
        assert_eq!(config.menu.list.max_entry_lines, 5);
    }

    /// A columnar option in the ide table is a mistake worth naming, not
    /// something to quietly ignore -- which is the whole reason the tables are
    /// split.
    #[test]
    fn an_option_in_the_wrong_menu_table_is_rejected() {
        let (_, warnings) = parse("[menu.ide]\ncolumns = 4\n");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("columns"), "{warnings:?}");

        let (_, warnings) = parse("[menu.columnar]\nborder = true\n");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("border"), "{warnings:?}");
    }

    #[test]
    fn a_zero_ide_height_means_as_many_rows_as_fit() {
        let (config, warnings) = parse("[menu.ide]\nmax_height = 0\n");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.menu.ide.max_height, u16::MAX);
        let (config, _) = parse("[menu.ide]\nmax_height = 6\n");
        assert_eq!(config.menu.ide.max_height, 6);
    }

    #[test]
    fn colours_reach_the_highlighter_palette() {
        let (config, warnings) = parse(
            r##"
            [colors]
            command = "blue_bold"
            comment = "#333333"
            hint = "light_gray"
            "##,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.palette.command, Style::new().fg(Color::Blue).bold());
        assert_eq!(
            config.palette.comment,
            Style::new().fg(Color::Rgb(0x33, 0x33, 0x33))
        );
        assert_eq!(config.hint_style, Style::new().fg(Color::LightGray));
        // Unset colours keep their defaults.
        assert_eq!(config.palette.string, Palette::default().string);
    }

    #[test]
    fn a_bad_colour_falls_back_and_names_itself() {
        let (config, warnings) = parse("[colors]\ncommand = \"chartreuse\"\n");
        assert_eq!(config.palette.command, Palette::default().command);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("chartreuse"), "{warnings:?}");
    }

    #[test]
    fn menu_colours_are_read_from_their_own_table() {
        let (config, warnings) = parse(
            r#"
            [menu.colors]
            text = "green"
            selected_text = "green_reverse"
            description = "yellow"
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.menu.colors.text, Style::new().fg(Color::Green));
        assert_eq!(
            config.menu.colors.selected_text,
            Style::new().fg(Color::Green).reverse()
        );
    }

    #[test]
    fn cursor_shapes_are_read_per_mode() {
        let (config, warnings) = parse(
            r#"
            [cursor]
            vi_insert = "line"
            vi_normal = "block"
            emacs = "inherit"
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.cursor.vi_insert, CursorShape::Line);
        assert_eq!(config.cursor.vi_normal, CursorShape::Block);
        assert_eq!(
            config.cursor.emacs,
            CursorShape::Inherit,
            "inherit means leave it alone"
        );
    }

    #[test]
    fn a_bad_cursor_shape_names_the_setting_it_came_from() {
        let (_, warnings) = parse("[cursor]\nvi_normal = \"rhombus\"\n");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("vi_normal"), "{warnings:?}");
        assert!(warnings[0].contains("rhombus"), "{warnings:?}");
    }

    #[test]
    fn a_unit_event_binds_from_its_name_alone() {
        let (config, warnings) = parse(
            r#"
            [[keybinding]]
            key = "ctrl-r"
            event = "SearchHistory"
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.keybindings.len(), 1);
        let binding = &config.keybindings[0];
        assert_eq!(binding.modifiers, KeyModifiers::CONTROL);
        assert_eq!(binding.key, KeyCode::Char('r'));
        assert_eq!(binding.event, ReedlineEvent::SearchHistory);
    }

    #[test]
    fn edit_commands_bind_through_the_shorthand() {
        let (config, warnings) = parse(
            r#"
            [[keybinding]]
            key = "alt-b"
            edit = [{ MoveWordLeft = { select = false } }]
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            config.keybindings[0].event,
            ReedlineEvent::Edit(vec![reedline::EditCommand::MoveWordLeft { select: false }])
        );
    }

    #[test]
    fn a_structured_event_binds_from_a_table() {
        let (config, warnings) = parse(
            r#"
            [[keybinding]]
            key = "f2"
            event = { Menu = "completion_menu" }
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            config.keybindings[0].event,
            ReedlineEvent::Menu("completion_menu".to_string())
        );
    }

    #[test]
    fn a_bad_binding_is_dropped_and_the_others_survive() {
        let (config, warnings) = parse(
            r#"
            [[keybinding]]
            key = "ctrl-r"
            event = "SearchHistory"

            [[keybinding]]
            key = "hyper-q"
            event = "SearchHistory"

            [[keybinding]]
            key = "ctrl-t"
            event = "NoSuchEventAtAll"

            [[keybinding]]
            key = "ctrl-y"
            "#,
        );
        assert_eq!(config.keybindings.len(), 1, "only the good one survives");
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert!(warnings.iter().any(|w| w.contains("hyper")), "{warnings:?}");
        assert!(
            warnings.iter().any(|w| w.contains("ctrl-t")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("ctrl-y")),
            "{warnings:?}"
        );
    }

    #[test]
    fn setting_both_event_and_edit_is_refused_rather_than_guessed_at() {
        let (config, warnings) = parse(
            r#"
            [[keybinding]]
            key = "ctrl-r"
            event = "SearchHistory"
            edit = [{ Undo = {} }]
            "#,
        );
        assert!(config.keybindings.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("not both"), "{warnings:?}");
    }

    #[test]
    fn abbreviations_are_read_as_a_plain_table() {
        let (config, warnings) = parse("[abbreviations]\ngs = \"git status\"\n");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(
            config.abbreviations.get("gs").map(String::as_str),
            Some("git status")
        );
    }

    #[test]
    fn every_setting_reaches_the_field_it_names() {
        let (config, warnings) = parse(
            r##"
            [editor]
            mode = "helix"
            highlight = false
            hints = false
            bracketed_paste = false
            kitty_protocol = true
            ansi_colors = false
            mouse_click = true
            buffer_editor = "nvim -R"

            [cursor]
            emacs = "block"
            vi_insert = "line"
            vi_normal = "underscore"
            helix_insert = "blink_line"
            helix_normal = "blink_block"
            helix_select = "blink_underscore"

            [menu]
            style = "list"
            marker = ">> "

            [menu.columnar]
            columns = 7
            col_width = 21
            col_padding = 3
            traversal = "vertical"

            [menu.ide]
            min_width = 11
            max_width = 61
            max_height = 9
            padding = 4
            border = true
            cursor_offset = -2
            description_mode = "left"
            min_description_width = 12
            max_description_width = 44
            max_description_height = 6
            description_offset = 5
            correct_cursor_pos = true

            [menu.list]
            page_size = 13
            max_entry_lines = 8
            description_position = "after"

            [menu.colors]
            text = "blue"
            selected_text = "red_reverse"
            description = "cyan"
            match = "magenta_bold"
            selected_match = "white_underline"

            [colors]
            command = "blue"
            builtin = "red"
            string = "cyan"
            variable = "magenta"
            operator = "white"
            comment = "black"
            hint = "yellow"
            selection = "bold"
            selection_cursor = "italic"

            [completion]
            partial = false

            [history]
            size = 42
            ignore_prefix = "#"

            [abbreviations]
            gs = "git status"

            [[keybinding]]
            key = "ctrl-t"
            event = "ClearScreen"
            "##,
        );
        assert!(warnings.is_empty(), "{warnings:?}");

        assert_eq!(config.edit_mode, EditMode::Helix);
        assert!(!config.highlight);
        assert!(!config.hints);
        assert!(!config.bracketed_paste);
        assert!(config.kitty_protocol);
        assert!(!config.ansi_colors);
        assert!(config.mouse_click);
        assert_eq!(config.buffer_editor, "nvim -R");

        assert_eq!(config.cursor.emacs, CursorShape::Block);
        assert_eq!(config.cursor.vi_insert, CursorShape::Line);
        assert_eq!(config.cursor.vi_normal, CursorShape::Underscore);
        assert_eq!(config.cursor.helix_insert, CursorShape::BlinkLine);
        assert_eq!(config.cursor.helix_normal, CursorShape::BlinkBlock);
        assert_eq!(config.cursor.helix_select, CursorShape::BlinkUnderscore);

        assert_eq!(config.menu.style, MenuStyle::List);
        assert_eq!(config.menu.marker, ">> ");

        assert_eq!(config.menu.columnar.columns, 7);
        assert_eq!(config.menu.columnar.col_width, Some(21));
        assert_eq!(config.menu.columnar.col_padding, 3);
        assert_eq!(config.menu.columnar.traversal, Traversal::Vertical);

        let ide = &config.menu.ide;
        assert_eq!(ide.min_width, 11);
        assert_eq!(ide.max_width, 61);
        assert_eq!(ide.max_height, 9);
        assert_eq!(ide.padding, 4);
        assert!(ide.border);
        assert_eq!(ide.cursor_offset, -2);
        assert_eq!(ide.description_mode, DescriptionSide::Left);
        assert_eq!(ide.min_description_width, 12);
        assert_eq!(ide.max_description_width, 44);
        assert_eq!(ide.max_description_height, 6);
        assert_eq!(ide.description_offset, 5);
        assert!(ide.correct_cursor_pos);

        assert_eq!(config.menu.list.page_size, 13);
        assert_eq!(config.menu.list.max_entry_lines, 8);
        assert_eq!(
            config.menu.list.description_position,
            DescriptionPlace::After
        );

        let menu_colors = &config.menu.colors;
        assert_eq!(menu_colors.text, Style::new().fg(Color::Blue));
        assert_eq!(
            menu_colors.selected_text,
            Style::new().fg(Color::Red).reverse()
        );
        assert_eq!(menu_colors.description, Style::new().fg(Color::Cyan));
        assert_eq!(menu_colors.matched, Style::new().fg(Color::Magenta).bold());
        assert_eq!(
            menu_colors.selected_match,
            Style::new().fg(Color::White).underline()
        );

        let palette = &config.palette;
        assert_eq!(palette.command, Style::new().fg(Color::Blue));
        assert_eq!(palette.builtin, Style::new().fg(Color::Red));
        assert_eq!(palette.string, Style::new().fg(Color::Cyan));
        assert_eq!(palette.variable, Style::new().fg(Color::Magenta));
        assert_eq!(palette.operator, Style::new().fg(Color::White));
        assert_eq!(palette.comment, Style::new().fg(Color::Black));
        assert_eq!(config.hint_style, Style::new().fg(Color::Yellow));
        assert_eq!(config.selection_style, Style::new().bold());
        assert_eq!(config.selection_cursor_style, Some(Style::new().italic()));

        assert!(!config.partial_completions);
        assert_eq!(config.history_size, 42);
        assert_eq!(config.history_ignore_prefix.as_deref(), Some("#"));

        assert_eq!(
            config.abbreviations.get("gs").map(String::as_str),
            Some("git status")
        );
        assert_eq!(config.keybindings.len(), 1);
        assert_eq!(config.keybindings[0].event, ReedlineEvent::ClearScreen);

        assert_ne!(config, Config::default());
    }

    /// Every name a `Raw*` struct accepts, as `(name, is_a_table)`.
    ///
    /// These are exactly the keys a config file may use, so they are the list
    /// `config.example.toml` has to document.
    fn accepted_keys() -> Vec<(String, bool)> {
        let source = include_str!("config.rs");
        let mut keys = Vec::new();
        let mut rest = source;
        while let Some(at) = rest.find("\nstruct Raw") {
            rest = &rest[at + 1..];
            let Some(open) = rest.find('{') else { break };
            let Some(close) = rest.find("\n}") else { break };
            for line in rest[open..close].lines() {
                let Some((name, kind)) = line.trim().split_once(':') else {
                    continue;
                };
                if name.is_empty()
                    || !name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                    // The catch-all for unrecognised keys, not a setting.
                    || name == "unknown"
                {
                    continue;
                }
                // A table is written as a header; `edit = [..]` is an inline
                // array, so a bare `Vec` is still a value.
                let table = kind.contains("Raw") || kind.contains("HashMap");
                keys.push((name.to_string(), table));
            }
            rest = &rest[close..];
        }
        keys
    }

    /// Settings the parser knows by name only so it can refuse them, and which
    /// the example explains rather than shows.
    const REFUSED: &[&str] = &["quick"];

    #[test]
    fn every_setting_is_documented_in_the_example_config() {
        let example = include_str!("../config.example.toml");
        let keys = accepted_keys();
        assert!(
            keys.len() > 50,
            "found only {} settings to check",
            keys.len()
        );

        let uncommented = |line: &str| line.trim().trim_start_matches(['#', ' ']).to_string();
        let missing: Vec<&str> = keys
            .iter()
            .filter(|(key, _)| !REFUSED.contains(&key.as_str()))
            .filter(|(key, table)| {
                !example.lines().map(uncommented).any(|line| {
                    if *table {
                        // A table is written as a header, possibly nested.
                        line.starts_with(&format!("[{key}]"))
                            || line.starts_with(&format!("[[{key}]]"))
                            || line.contains(&format!(".{key}]"))
                    } else {
                        line.strip_prefix(key.as_str())
                            .is_some_and(|rest| rest.trim_start().starts_with('='))
                    }
                })
            })
            .map(|(key, _)| key.as_str())
            .collect();
        assert!(
            missing.is_empty(),
            "config.example.toml does not document {missing:?}"
        );
    }

    #[test]
    fn the_example_config_parses_clean_and_is_exactly_the_defaults() {
        let example = include_str!("../config.example.toml");
        let (config, warnings) = Config::parse(example);
        assert!(warnings.is_empty(), "example config warns: {warnings:?}");
        assert_eq!(
            config,
            Config::default(),
            "config.example.toml no longer shows the defaults it claims to"
        );
    }

    #[test]
    fn the_reedline_only_toggles_are_read() {
        let (config, warnings) =
            parse("[editor]\ncross_line_cursor = false\n\n[menu]\npersistent = true\n");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(!config.cross_line_cursor);
        assert!(config.menu.persistent);
    }

    #[test]
    fn the_menu_modes_are_read_from_their_names() {
        let (config, warnings) = parse(
            r#"
            [menu]
            input_mode = "full_buffer"
            output_mode = "extend_to_end"
            "#,
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(config.menu.input_mode, InputMode::FullBuffer);
        assert_eq!(config.menu.output_mode, OutputMode::ExtendToEnd);
    }

    #[test]
    fn shell_integration_reads_every_marker_protocol() {
        for (spelling, expected) in [
            ("off", ShellIntegration::Off),
            ("osc133", ShellIntegration::Osc133),
            ("osc133_click_events", ShellIntegration::Osc133ClickEvents),
            ("osc633", ShellIntegration::Osc633),
        ] {
            let (config, warnings) =
                parse(&format!("[editor]\nshell_integration = \"{spelling}\"\n"));
            assert!(warnings.is_empty(), "{spelling}: {warnings:?}");
            assert_eq!(config.shell_integration, expected, "{spelling}");
        }
    }

    /// Names are matched case-insensitively, and `-` reads the same as `_`.
    #[test]
    fn an_enum_name_may_be_spelled_with_dashes_or_capitals() {
        for spelling in [
            "prefer_right",
            "prefer-right",
            "Prefer-Right",
            "PREFER_RIGHT",
        ] {
            let (config, warnings) =
                parse(&format!("[menu.ide]\ndescription_mode = \"{spelling}\"\n"));
            assert!(warnings.is_empty(), "{spelling}: {warnings:?}");
            assert_eq!(
                config.menu.ide.description_mode,
                DescriptionSide::PreferRight,
                "{spelling}"
            );
        }
    }

    /// The message has to name the value actually in use, not a positional
    /// guess at which variant the default is.
    #[test]
    fn a_rejected_enum_value_names_the_fallback_actually_used() {
        let (config, warnings) = parse("[editor]\nmode = \"nonsense\"\n");
        assert_eq!(config.edit_mode, EditMode::Emacs);
        assert!(warnings[0].contains("emacs"), "{warnings:?}");
        assert!(warnings[0].contains("vi"), "should list the alternatives");
    }

    #[test]
    fn ide_border_symbols_can_be_overridden() {
        // All six, so a corner wired to the wrong field is visible.
        let (config, warnings) = Config::parse(
            "[menu.ide.border_symbols]\ntop_left = \"1\"\ntop_right = \"2\"\n\
             bottom_left = \"3\"\nbottom_right = \"4\"\nhorizontal = \"-\"\nvertical = \"|\"\n",
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        let symbols = &config.menu.ide.border_symbols;
        assert_eq!(
            (
                symbols.top_left,
                symbols.top_right,
                symbols.bottom_left,
                symbols.bottom_right,
                symbols.horizontal,
                symbols.vertical
            ),
            ('1', '2', '3', '4', '-', '|')
        );

        // Untouched symbols keep reedline's defaults.
        let (config, _) = Config::parse("[menu.ide.border_symbols]\ntop_left = \"+\"\n");
        assert_eq!(config.menu.ide.border_symbols.vertical, '\u{2502}');
    }

    #[test]
    fn the_path_honours_the_explicit_override_first() {
        // SAFETY: `set_var` races with any other thread reading the
        // environment, and cargo runs tests in parallel. Nothing else in this
        // suite touches `REEDLINE_BASH_CONFIG`, so this is the only writer.
        unsafe { std::env::set_var("REEDLINE_BASH_CONFIG", "/tmp/explicit.toml") };
        assert_eq!(path(), Some(PathBuf::from("/tmp/explicit.toml")));
        unsafe { std::env::remove_var("REEDLINE_BASH_CONFIG") };
    }
}
