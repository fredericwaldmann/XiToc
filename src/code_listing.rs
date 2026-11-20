//! Portable listing appearance and immutable source validation.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Styles = BTreeMap<String, Style>;
pub const LIGHT: &str = "Scientific";
pub const DARK: &str = "Scientific Dark";
pub const CLASSIC: &str = "Classic LaTeX";
pub const MINIMAL: &str = "Minimal";
pub const PRESETS: [&str; 4] = [LIGHT, DARK, CLASSIC, MINIMAL];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Frame {
    None,
    #[default]
    Single,
    Lines,
    Left,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Numbers {
    None,
    #[default]
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Typeface {
    #[default]
    DejaVu,
    UiMonospace,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub typeface: Typeface,
    pub size: f32,
    pub spacing: f32,
    pub foreground: [u8; 3],
    pub keyword: [u8; 3],
    pub comment: [u8; 3],
    pub string: [u8; 3],
    pub identifier: [u8; 3],
    pub numeric: [u8; 3],
    pub number_color: [u8; 3],
    pub italic_comments: bool,
    pub background: [u8; 3],
    pub opacity: u8,
    pub frame: Frame,
    pub rule: [u8; 3],
    pub thickness: f32,
    pub numbers: Numbers,
    pub number_size: f32,
    pub number_sep: f32,
    pub first: u32,
    pub step: u32,
    pub wrap: bool,
    pub break_at_space: bool,
    pub tabs: u8,
    pub show_spaces: bool,
    pub show_tabs: bool,
    pub show_string_spaces: bool,
    pub padding: f32,
    pub left: f32,
    pub right: f32,
    pub caption: String,
    pub header: bool,
    pub language: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            typeface: Typeface::DejaVu,
            size: 12.,
            spacing: 1.25,
            foreground: [29, 36, 46],
            keyword: [28, 65, 160],
            comment: [54, 102, 75],
            string: [153, 49, 62],
            identifier: [98, 51, 135],
            numeric: [135, 65, 20],
            number_color: [96, 105, 115],
            italic_comments: false,
            background: [250, 251, 253],
            opacity: 255,
            frame: Frame::Single,
            rule: [185, 194, 206],
            thickness: 0.8,
            numbers: Numbers::Left,
            number_size: 10.,
            number_sep: 9.,
            first: 1,
            step: 1,
            wrap: true,
            break_at_space: false,
            tabs: 4,
            show_spaces: false,
            show_tabs: false,
            show_string_spaces: false,
            padding: 10.,
            left: 0.,
            right: 0.,
            caption: String::new(),
            header: true,
            language: None,
        }
    }
}

pub fn preset(name: &str) -> Option<Style> {
    let mut style = Style::default();
    match name {
        LIGHT | "Scientific Light" => (),
        MINIMAL => {
            style.frame = Frame::None;
            style.numbers = Numbers::None;
            style.keyword = style.foreground;
            style.comment = style.foreground;
            style.string = style.foreground;
            style.identifier = style.foreground;
            style.numeric = style.foreground;
        }
        DARK => {
            style.background = [30, 35, 45];
            style.foreground = [229, 234, 241];
            style.keyword = [149, 186, 255];
            style.comment = [145, 193, 159];
            style.string = [255, 165, 150];
            style.identifier = [222, 176, 252];
            style.numeric = [244, 197, 137];
            style.rule = [78, 87, 105];
            style.number_color = [165, 175, 191];
        }
        CLASSIC | "Classic LaTeX Listings" => {
            style.background = [255; 3];
            style.keyword = [38, 47, 85];
            style.comment = [84; 3];
            style.string = [102, 50, 48];
            style.frame = Frame::Lines;
            style.italic_comments = true;
            style.numbers = Numbers::None;
            style.size = 11.;
        }
        _ => return None,
    }
    Some(style)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Listing {
    pub source_path: Option<String>,
    pub configuration: String,
    pub line_ranges: String,
    pub show_title: bool,
    pub custom_title: String,
    pub preferred_width: Option<f64>,
    pub language: Option<String>,
    pub style_name: String,
    pub fallback: Style,
    pub overrides: Option<Style>,
}

pub fn effective(name: &str, document: &Styles, global: &Styles) -> Option<Style> {
    document
        .get(name)
        .or_else(|| global.get(name))
        .cloned()
        .or_else(|| preset(name))
}

impl Default for Listing {
    fn default() -> Self {
        Self::new(false)
    }
}

impl Listing {
    pub fn new(dark: bool) -> Self {
        let name = if dark { DARK } else { LIGHT };
        Self {
            source_path: None,
            configuration: String::new(),
            line_ranges: String::new(),
            show_title: true,
            custom_title: String::new(),
            preferred_width: None,
            language: None,
            style_name: name.into(),
            fallback: preset(name).unwrap(),
            overrides: None,
        }
    }
    pub fn valid(&self) -> bool {
        valid_name(&self.style_name)
            && self.language.as_ref().is_none_or(|s| language_known(s))
            && self.configuration.len() <= 8192
            && self
                .source_path
                .as_ref()
                .is_none_or(|p| p.len() <= 16384 && !p.contains('\0'))
            && self.custom_title.len() <= 512
            && !self.custom_title.chars().any(char::is_control)
            && parse_ranges(&self.line_ranges, None).is_ok()
            && self
                .preferred_width
                .is_none_or(|w| (100. ..=1e6).contains(&w))
            && self.fallback.valid()
            && self.overrides.as_ref().is_none_or(Style::valid)
    }
}

pub fn line_count(source: &str) -> usize {
    source.split_terminator('\n').count()
}

pub fn parse_ranges(input: &str, count: Option<usize>) -> Result<Vec<(usize, usize)>, String> {
    if input.len() > 8192 {
        return Err("Line selection exceeds 8 KiB.".into());
    }
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut ranges = Vec::new();
    let number = |value: &str| {
        let value = value.trim();
        value
            .bytes()
            .all(|c| c.is_ascii_digit())
            .then(|| value.parse::<usize>().ok())
            .flatten()
            .filter(|n| *n != 0)
            .ok_or_else(|| format!("Expected a positive source line number: {value}."))
    };
    for part in input.split(',') {
        let (first, last) = match part.split_once('-') {
            Some((a, b)) => (number(a)?, number(b)?),
            None => {
                let n = number(part)?;
                (n, n)
            }
        };
        if first > last {
            return Err(format!("Reversed line range: {first}-{last}."));
        }
        if count.is_some_and(|n| last > n) {
            return Err(format!(
                "Line {last} exceeds the available {} source lines.",
                count.unwrap()
            ));
        }
        ranges.push((first, last));
    }
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (first, last) in ranges {
        if let Some(previous) = merged.last_mut().filter(|p| first <= p.1.saturating_add(1)) {
            previous.1 = previous.1.max(last);
        } else {
            merged.push((first, last));
        }
    }
    Ok(merged)
}

pub fn format_ranges(ranges: &[(usize, usize)]) -> String {
    ranges
        .iter()
        .map(|&(first, last)| {
            if first == last {
                first.to_string()
            } else {
                format!("{first}-{last}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn preset_configuration(name: &str) -> String {
    let Some(style) = preset(name) else {
        return format!(r"\lstset{{style={name}}}");
    };
    let size = |points: f32| match points as u32 {
        8 => r"\tiny",
        9 => r"\scriptsize",
        10 => r"\footnotesize",
        11 => r"\small",
        14 => r"\large",
        _ => r"\normalsize",
    };
    let color = |[r, g, b]: [u8; 3]| format!(r"\color{{#{r:02X}{g:02X}{b:02X}}}");
    let numbers = match style.numbers {
        Numbers::None => "none",
        Numbers::Left => "left",
        Numbers::Right => "right",
    };
    let frame = match style.frame {
        Frame::None => "none",
        Frame::Single => "single",
        Frame::Lines => "lines",
        Frame::Left => "leftline",
    };
    let italic = if style.italic_comments {
        r"\itshape"
    } else {
        ""
    };
    let options = [
        format!(r"basicstyle=\ttfamily{}", size(style.size)),
        format!("keywordstyle={}", color(style.keyword)),
        format!("commentstyle={}{}", color(style.comment), italic),
        format!("stringstyle={}", color(style.string)),
        format!("identifierstyle={}", color(style.identifier)),
        format!("numbers={numbers}"),
        format!("numberstyle={}", size(style.number_size)),
        format!("frame={frame}"),
        format!("rulecolor={}", color(style.rule)),
        format!("backgroundcolor={}", color(style.background)),
        format!("breaklines={}", style.wrap),
        format!("tabsize={}", style.tabs),
    ]
    .join(",\n    ");
    format!("\\lstset{{\n    {options}\n}}")
}

pub fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 80
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
}

pub fn validate_styles(styles: &Styles) -> Result<(), String> {
    if styles.len() > 64
        || styles
            .iter()
            .any(|(name, style)| !valid_name(name) || !style.valid())
    {
        return Err("Invalid listing style name or values, or more than 64 styles.".into());
    }
    Ok(())
}

impl Style {
    pub fn valid(&self) -> bool {
        [
            (self.size, 6., 72.),
            (self.number_size, 6., 72.),
            (self.spacing, 1., 3.),
            (self.padding, 0., 96.),
            (self.left, 0., 96.),
            (self.right, 0., 96.),
            (self.number_sep, 0., 96.),
            (self.thickness, 0., 12.),
        ]
        .into_iter()
        .all(|(value, min, max)| (min..=max).contains(&value))
            && (1..=16).contains(&self.tabs)
            && self.step > 0
            && self.caption.len() <= 512
            && !self.caption.chars().any(char::is_control)
            && self.language.as_ref().is_none_or(|s| language_known(s))
    }
}

pub const LANGUAGES: &[(&str, &[&str])] = &[
    ("Plain Text", &["txt"]),
    ("Python", &["py", "pyw"]),
    ("Rust", &["rs"]),
    ("C", &["c", "h"]),
    ("C++", &["cpp", "hpp", "cc", "cxx", "hh"]),
    ("JavaScript", &["js", "jsx", "mjs"]),
    ("TypeScript", &["ts", "tsx"]),
    ("JSON", &["json"]),
    ("HTML", &["html", "htm"]),
    ("CSS", &["css"]),
    ("Bash", &["sh", "bash"]),
    ("SQL", &["sql"]),
    ("LaTeX", &["tex"]),
    ("Markdown", &["md", "markdown"]),
    ("Java", &["java"]),
    ("YAML", &["yaml", "yml"]),
    ("XML", &["xml"]),
    ("TOML", &["toml"]),
];
pub fn language_known(name: &str) -> bool {
    LANGUAGES.iter().any(|(language, _)| *language == name)
}
pub fn detect(filename: &str) -> &'static str {
    let ext = filename.rsplit_once('.').map_or("", |(_, ext)| ext);
    LANGUAGES
        .iter()
        .find(|(_, extensions)| extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
        .map_or("Plain Text", |(name, _)| *name)
}
pub fn source(bytes: &[u8]) -> Result<&str, String> {
    let invalid = || {
        "Code listings require UTF-8 text. Use File Attachment for binary files or other encodings."
            .to_owned()
    };
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    if text
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t' | '\u{c}'))
    {
        return Err(invalid());
    }
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

// A configuration is one directive. Nested braces are understood for literal
// arguments and colors. Nothing is passed to the mathematical renderer.
pub fn parse(
    input: &str,
    mut base: Style,
    lookup: impl Fn(&str) -> Option<Style>,
) -> Result<(Option<String>, Style), String> {
    fn group(input: &str) -> Result<(&str, &str), String> {
        let input = input.trim_start();
        if !input.starts_with('{') {
            return Err("Expected a braced argument.".into());
        }
        let mut depth = 1;
        for (i, ch) in input.char_indices().skip(1) {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => (),
            }
            if depth == 0 {
                return Ok((&input[1..i], &input[i + 1..]));
            }
        }
        Err("Unclosed braced argument.".into())
    }
    fn color(value: &str) -> Result<([u8; 3], &str), String> {
        let (name, rest) = group(value.strip_prefix(r"\color").unwrap_or(value))?;
        let rgb = match name {
            "black" => [0, 0, 0],
            "white" => [255; 3],
            "blue" => [25, 58, 165],
            "red" => [165, 32, 39],
            "gray" => [95; 3],
            "green" => [20, 112, 58],
            hex if hex.starts_with('#')
                && hex.len() == 7
                && hex[1..].bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                std::array::from_fn(|i| u8::from_str_radix(&hex[1 + 2 * i..3 + 2 * i], 16).unwrap())
            }
            _ => return Err(format!("Unsupported color: {name}.")),
        };
        Ok((rgb, rest.trim()))
    }
    let input = input.trim();
    if input.len() > 8192 {
        return Err("Listing configuration exceeds 8 KiB.".into());
    }
    let (name, rest) = if let Some(rest) = input.strip_prefix(r"\lstdefinestyle") {
        let (name, rest) = group(rest)?;
        if !valid_name(name) {
            return Err("Invalid style name.".into());
        }
        (Some(name.to_owned()), rest)
    } else if let Some(rest) = input.strip_prefix(r"\lstset") {
        (None, rest)
    } else {
        return Err("Use \\lstset{...} or \\lstdefinestyle{Name}{...}.".into());
    };
    let (body, tail) = group(rest)?;
    if !tail.trim().is_empty() {
        return Err("Enter one listing directive at a time.".into());
    }
    let mut depth = 0_i32;
    let mut start = 0;
    let mut options = Vec::new();
    for (i, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                options.push(&body[start..i]);
                start = i + 1;
            }
            _ => (),
        }
    }
    options.push(&body[start..]);
    for option in options.into_iter().map(str::trim).filter(|s| !s.is_empty()) {
        let (key, value) = option
            .split_once('=')
            .ok_or_else(|| format!("Expected name=value: {option}"))?;
        let (key, value) = (key.trim(), value.trim());
        let invalid = || format!("Invalid value for {key}: {value}");
        let float = || {
            value
                .strip_suffix("pt")
                .unwrap_or(value)
                .parse::<f32>()
                .map_err(|_| invalid())
        };
        let boolean = || value.parse::<bool>().map_err(|_| invalid());
        match key {
            "style" => {
                base = lookup(value).ok_or_else(|| format!("Unknown listing style: {value}"))?
            }

            "basicstyle" | "numberstyle" => {
                let value = value.strip_prefix(r"\ttfamily").unwrap_or(value);
                let size = match value {
                    "" => continue,
                    r"\tiny" => 8.,
                    r"\scriptsize" => 9.,
                    r"\footnotesize" => 10.,
                    r"\small" => 11.,
                    r"\normalsize" => 12.,
                    r"\large" => 14.,
                    r"\Large" => 17.,
                    r"\LARGE" => 20.,
                    r"\huge" => 25.,
                    r"\Huge" => 30.,
                    _ => return Err(invalid()),
                };
                if key == "basicstyle" {
                    base.size = size;
                } else {
                    base.number_size = size;
                }
            }
            "keywordstyle" | "commentstyle" | "stringstyle" | "identifierstyle"
            | "backgroundcolor" | "rulecolor" => {
                let (rgb, rest) = color(value)?;
                match key {
                    "commentstyle" if matches!(rest, "" | r"\itshape") => {
                        base.comment = rgb;
                        base.italic_comments = !rest.is_empty();
                    }
                    _ if !rest.is_empty() => return Err(invalid()),
                    "keywordstyle" => base.keyword = rgb,
                    "stringstyle" => base.string = rgb,
                    "identifierstyle" => base.identifier = rgb,
                    "backgroundcolor" => base.background = rgb,
                    "rulecolor" => base.rule = rgb,
                    _ => return Err(invalid()),
                }
            }
            "numbers" => {
                base.numbers = match value {
                    "none" => Numbers::None,
                    "left" => Numbers::Left,
                    "right" => Numbers::Right,
                    _ => return Err(invalid()),
                }
            }
            "frame" => {
                base.frame = match value {
                    "none" => Frame::None,
                    "single" => Frame::Single,
                    "lines" | "tb" => Frame::Lines,
                    "leftline" | "l" => Frame::Left,
                    _ => return Err(invalid()),
                }
            }
            "numbersep" => base.number_sep = float()?,
            "framerule" => base.thickness = float()?,
            "xleftmargin" => base.left = float()?,
            "xrightmargin" => base.right = float()?,
            "framesep" => base.padding = float()?,

            "stepnumber" => base.step = value.parse().map_err(|_| invalid())?,
            "tabsize" => base.tabs = value.parse().map_err(|_| invalid())?,
            "breaklines" => base.wrap = boolean()?,
            "breakatwhitespace" => base.break_at_space = boolean()?,
            "showspaces" => base.show_spaces = boolean()?,
            "showstringspaces" => base.show_string_spaces = boolean()?,
            "showtabs" => base.show_tabs = boolean()?,
            "caption" | "title" => {
                let (text, rest) = group(value)?;
                if !rest.trim().is_empty() || text.contains(['\\', '{', '}']) {
                    return Err(invalid());
                }
                base.caption = text.into();
            }
            _ => return Err(format!("Unsupported listing option: {key}")),
        }
    }
    if !base.valid() {
        return Err("Listing values are outside supported ranges.".into());
    }
    Ok((name, base))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_detect_language_and_preserve_original_byte_sequences() {
        for (file, expected) in [
            ("simulation.py", "Python"),
            ("main.rs", "Rust"),
            ("main.CPP", "C++"),
            ("math.tex", "LaTeX"),
            ("readme.txt", "Plain Text"),
            ("demo.unknown", "Plain Text"),
        ] {
            assert_eq!(detect(file), expected);
        }
        for original in [&b""[..], b"a\r\n\tb\rc\n", "\u{feff}γειά\r\n".as_bytes()] {
            let asset = crate::asset::BinaryAsset::new(original.into()).unwrap();
            assert!(asset.listing_text());
            let encoded = serde_json::to_string(&asset).unwrap();
            let reopened: crate::asset::BinaryAsset = serde_json::from_str(&encoded).unwrap();
            assert_eq!(reopened.bytes(), original);
            assert_eq!(asset.id(), reopened.id());
            assert!(!asset.listing_source().unwrap().contains('\r'));
        }
        for bytes in [&b"\xff\xfe"[..], b"binary\0bytes", b"\x1b[31m"] {
            assert!(source(bytes).is_err());
        }
    }

    #[test]
    fn named_styles_shadow_in_order_and_instance_overrides_are_isolated() {
        let mut global = Styles::new();
        let mut document = Styles::new();
        let preset = preset(LIGHT).unwrap();
        let outer = Style {
            size: 10.,
            ..preset.clone()
        };
        let inner = Style {
            size: 17.,
            ..preset.clone()
        };
        global.insert(LIGHT.into(), outer.clone());
        document.insert(LIGHT.into(), inner.clone());
        assert_eq!(
            effective(LIGHT, &Styles::new(), &Styles::new()),
            Some(preset)
        );
        assert_eq!(effective(LIGHT, &Styles::new(), &global), Some(outer));
        assert_eq!(effective(LIGHT, &document, &global), Some(inner));
        assert_ne!(super::preset(DARK), super::preset(CLASSIC));
    }

    #[test]
    fn normalizes_and_validates_line_ranges() {
        assert_eq!(
            parse_ranges("5, 1-3, 2-4, 5", Some(5)).unwrap(),
            vec![(1, 5)]
        );
        assert_eq!(
            parse_ranges("1-20, 25-32, 35, 37", Some(50)).unwrap(),
            vec![(1, 20), (25, 32), (35, 35), (37, 37)]
        );
        assert!(parse_ranges("", Some(0)).unwrap().is_empty());
        for invalid in ["0", "-2", "4-1", "a", "1,", "1--3", "6"] {
            assert!(parse_ranges(invalid, Some(5)).is_err(), "{invalid}");
        }
        assert_eq!(line_count(""), 0);
        assert_eq!(line_count("\n"), 1);
        assert_eq!(line_count("a\nb\n"), 2);
        assert_eq!(line_count("a\nb"), 2);
    }

    #[test]
    fn builtin_preset_text_recreates_each_appearance() {
        for name in PRESETS {
            let style = preset(name).unwrap();
            assert_eq!(
                parse(&preset_configuration(name), style.clone(), preset)
                    .unwrap()
                    .1,
                style
            );
        }
    }

    #[test]
    fn validates_listing_directives_and_named_style_overlays() {
        let input = r"\lstdefinestyle{Scientific}{
            basicstyle=\ttfamily\small, keywordstyle=\color{blue},
            commentstyle=\color{gray}\itshape, stringstyle=\color{red},
            numbers=left, numberstyle=\tiny, frame=single, breaklines=true,
            tabsize=4, title={Algorithm, step 1}, xleftmargin=3pt,
            backgroundcolor=\color{#FAFBFC}
        }";
        let (name, style) = parse(input, Style::default(), preset).unwrap();
        assert_eq!(name.as_deref(), Some("Scientific"));
        assert_eq!(style.size, 11.);
        assert_eq!(style.number_size, 8.);
        assert_eq!(style.background, [250, 251, 252]);
        assert_eq!(style.caption, "Algorithm, step 1");
        assert!(style.italic_comments && style.wrap);
        let (_, changed) = parse(
            r"\lstset{style=Scientific,numbers=right,tabsize=8,frame=leftline}",
            Style::default(),
            |name| (name == "Scientific").then(|| style.clone()),
        )
        .unwrap();
        assert_eq!(
            (changed.numbers, changed.tabs, changed.frame),
            (Numbers::Right, 8, Frame::Left)
        );
        assert_eq!(changed.keyword, style.keyword);
        for invalid in [
            r"\input{a}",
            r"\lstset{tabsize=0}",
            r"\lstset{caption={test}",
            r"\lstset{escapeinside={(*@}{@*)}}",
            r"\lstset{style=Missing}",
            r"\lstset{rulecolor=\color{#12界}}",
            r"\lstset{caption={\input{a}}}",
        ] {
            assert!(
                parse(invalid, Style::default(), preset).is_err(),
                "{invalid}"
            );
        }
    }
}
