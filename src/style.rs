//! Colours and attributes as written in the config.
//!
//! One string per style: a colour name, then any attributes separated by `_`.
//! `"light_green"`, `"yellow_bold"`, `"reverse"`, `"#ff8800_underline"`,
//! `"default"`.

use nu_ansi_term::{Color, Style};

/// Try to parse a style
pub fn try_parse(spec: &str) -> Result<Style, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("default") {
        return Ok(Style::new());
    }

    let lowered = spec.to_ascii_lowercase();
    let (color, rest) = split_color(&lowered)?;

    let mut style = match color {
        Some(color) => Style::new().fg(color),
        None => Style::new(),
    };

    for attribute in rest.split('_').filter(|part| !part.is_empty()) {
        style = match attribute {
            "bold" | "b" => style.bold(),
            "dimmed" | "dim" => style.dimmed(),
            "italic" | "i" => style.italic(),
            "underline" | "u" => style.underline(),
            "blink" => style.blink(),
            "reverse" | "r" => style.reverse(),
            "hidden" => style.hidden(),
            "strikethrough" | "s" => style.strikethrough(),
            other => {
                return Err(format!(
                    "{other:?} is not a colour or attribute (in {spec:?})"
                ));
            }
        };
    }

    Ok(style)
}

/// Longest colour name that `lowered` starts with, and whatever follows it.
fn split_color(lowered: &str) -> Result<(Option<Color>, &str), String> {
    if let Some(rest) = lowered.strip_prefix('#') {
        let (hex, rest) = match rest.find('_') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        return Ok((Some(parse_hex(hex)?), rest));
    }

    const NAMES: &[(&str, Color)] = &[
        ("light_magenta", Color::LightMagenta),
        ("light_yellow", Color::LightYellow),
        ("light_purple", Color::LightPurple),
        ("light_green", Color::LightGreen),
        ("light_white", Color::LightGray),
        ("light_blue", Color::LightBlue),
        ("light_cyan", Color::LightCyan),
        ("light_gray", Color::LightGray),
        ("light_red", Color::LightRed),
        ("dark_gray", Color::DarkGray),
        ("magenta", Color::Magenta),
        ("default", Color::Default),
        ("purple", Color::Purple),
        ("yellow", Color::Yellow),
        ("black", Color::Black),
        ("green", Color::Green),
        ("white", Color::White),
        ("blue", Color::Blue),
        ("cyan", Color::Cyan),
        ("gray", Color::DarkGray),
        ("red", Color::Red),
    ];

    for (name, color) in NAMES {
        if let Some(rest) = lowered.strip_prefix(name)
            && (rest.is_empty() || rest.starts_with('_'))
        {
            return Ok((Some(*color), rest));
        }
    }

    Ok((None, lowered))
}

fn parse_hex(hex: &str) -> Result<Color, String> {
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("#{hex} is not a six-digit hex colour"));
    }
    let component = |from: usize| u8::from_str_radix(&hex[from..from + 2], 16).unwrap_or(0);
    Ok(Color::Rgb(component(0), component(2), component(4)))
}

/// Parse, falling back to `fallback` and reporting anything wrong.
pub fn parse_or(spec: Option<&String>, fallback: Style, warnings: &mut Vec<String>) -> Style {
    match spec {
        None => fallback,
        Some(spec) => match try_parse(spec) {
            Ok(style) => style,
            Err(message) => {
                warnings.push(message);
                fallback
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_colour_name_is_a_foreground() {
        assert_eq!(try_parse("green").unwrap(), Style::new().fg(Color::Green));
        assert_eq!(
            try_parse("light_cyan").unwrap(),
            Style::new().fg(Color::LightCyan)
        );
        assert_eq!(
            try_parse("dark_gray").unwrap(),
            Style::new().fg(Color::DarkGray)
        );
    }

    #[test]
    fn attributes_stack_onto_the_colour() {
        assert_eq!(
            try_parse("light_green_bold").unwrap(),
            Style::new().fg(Color::LightGreen).bold()
        );
        assert_eq!(
            try_parse("yellow_bold_italic").unwrap(),
            Style::new().fg(Color::Yellow).bold().italic()
        );
    }

    #[test]
    fn attributes_stand_alone_without_a_colour() {
        assert_eq!(try_parse("reverse").unwrap(), Style::new().reverse());
        assert_eq!(
            try_parse("bold_underline").unwrap(),
            Style::new().bold().underline()
        );
    }

    #[test]
    fn default_and_empty_mean_leave_it_alone() {
        assert_eq!(try_parse("").unwrap(), Style::new());
        assert_eq!(try_parse("   ").unwrap(), Style::new());
        // Also nu-ansi-term's own no-op colour, so either reading is a no-op.
        assert!(try_parse("default").is_ok());
    }

    #[test]
    fn hex_colours_are_accepted() {
        assert_eq!(
            try_parse("#ff8800").unwrap(),
            Style::new().fg(Color::Rgb(255, 136, 0))
        );
        assert_eq!(
            try_parse("#00FF00_bold").unwrap(),
            Style::new().fg(Color::Rgb(0, 255, 0)).bold()
        );
    }

    #[test]
    fn case_does_not_matter() {
        assert_eq!(
            try_parse("LIGHT_GREEN_BOLD").unwrap(),
            try_parse("light_green_bold").unwrap()
        );
    }

    #[test]
    fn short_attribute_names_match_nushells() {
        assert_eq!(
            try_parse("green_b").unwrap(),
            try_parse("green_bold").unwrap()
        );
        assert_eq!(try_parse("r").unwrap(), try_parse("reverse").unwrap());
        assert_eq!(
            try_parse("green_u").unwrap(),
            try_parse("green_underline").unwrap()
        );
    }

    #[test]
    fn a_colour_name_only_matches_a_whole_segment() {
        let err = try_parse("redirect").unwrap_err();
        assert!(err.contains("redirect"), "{err}");
    }

    #[test]
    fn nonsense_is_reported_with_the_offending_word() {
        let err = try_parse("green_sparkly").unwrap_err();
        assert!(err.contains("sparkly"), "{err}");
        let err = try_parse("#12345").unwrap_err();
        assert!(err.contains("hex"), "{err}");
        let err = try_parse("#gggggg").unwrap_err();
        assert!(err.contains("hex"), "{err}");
    }

    #[test]
    fn parse_or_falls_back_and_records_the_reason() {
        let mut warnings = Vec::new();
        let fallback = Style::new().fg(Color::Red);

        assert_eq!(parse_or(None, fallback, &mut warnings), fallback);
        assert!(warnings.is_empty(), "absent is not a complaint");

        let good = "blue".to_string();
        assert_eq!(
            parse_or(Some(&good), fallback, &mut warnings),
            Style::new().fg(Color::Blue)
        );
        assert!(warnings.is_empty());

        let bad = "chartreuse".to_string();
        assert_eq!(parse_or(Some(&bad), fallback, &mut warnings), fallback);
        assert_eq!(warnings.len(), 1);
    }
}
