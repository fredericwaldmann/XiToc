//! Portable listing appearance and immutable source validation.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Styles = BTreeMap<String, Style>;
pub const LIGHT: &str = "Scientific Light";
pub const DARK: &str = "Scientific Dark";
pub const CLASSIC: &str = "Classic LaTeX Listings";
pub const PRESETS: [&str; 3] = [LIGHT, DARK, CLASSIC];

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
        LIGHT => (),
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
        CLASSIC => {
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
#[serde(deny_unknown_fields)]
pub struct Listing {
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

impl Listing {
    pub fn new(dark: bool) -> Self {
        let name = if dark { DARK } else { LIGHT };
        Self {
            language: None,
            style_name: name.into(),
            fallback: preset(name).unwrap(),
            overrides: None,
        }
    }
    pub fn style(&self, document: &Styles, global: &Styles) -> Style {
        self.overrides
            .clone()
            .or_else(|| effective(&self.style_name, document, global))
            .unwrap_or_else(|| self.fallback.clone())
    }
    pub fn valid(&self) -> bool {
        valid_name(&self.style_name)
            && self.language.as_ref().is_none_or(|s| language_known(s))
            && self.fallback.valid()
            && self.overrides.as_ref().is_none_or(Style::valid)
    }
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
            "language" if language_known(value) => base.language = Some(value.into()),
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
            "firstnumber" => base.first = value.parse().map_err(|_| invalid())?,
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
        let mut model = Listing::new(false);
        assert_eq!(model.style(&Styles::new(), &Styles::new()), preset);
        assert_eq!(model.style(&Styles::new(), &global), outer);
        assert_eq!(model.style(&document, &global), inner);
        model.overrides = Some(preset.clone());
        assert_eq!(model.style(&document, &global), preset);
        assert_ne!(super::preset(DARK), super::preset(CLASSIC));
    }

    #[test]
    fn validates_listing_directives_and_named_style_overlays() {
        let input = r"\lstdefinestyle{Scientific}{
            basicstyle=\ttfamily\small, keywordstyle=\color{blue},
            commentstyle=\color{gray}\itshape, stringstyle=\color{red},
            numbers=left, numberstyle=\tiny, frame=single, breaklines=true,
            tabsize=4, title={Algorithm, step 1}, xleftmargin=3pt,
            backgroundcolor=\color{#FAFBFC}, language=Python
        }";
        let (name, style) = parse(input, Style::default(), preset).unwrap();
        assert_eq!(name.as_deref(), Some("Scientific"));
        assert_eq!(style.size, 11.);
        assert_eq!(style.number_size, 8.);
        assert_eq!(style.background, [250, 251, 252]);
        assert_eq!(style.caption, "Algorithm, step 1");
        assert_eq!(style.language.as_deref(), Some("Python"));
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
