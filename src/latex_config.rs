//! Portable math-only configuration. No filesystem, packages, or TeX execution.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LatexConfig {
    pub preamble: String,
    pub postamble: String,
}
impl LatexConfig {
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LatexPreferences {
    /// Unknown saved preset names resolve to the bundled Latin Modern Math preset.
    pub preset: String,
    pub size: f64,
    pub color: [u8; 3],
    pub high_quality: bool,
    pub config: LatexConfig,
}
impl Default for LatexPreferences {
    fn default() -> Self {
        Self {
            preset: "latin-modern-math".into(),
            size: 24.,
            color: [32, 39, 49],
            high_quality: true,
            config: LatexConfig::default(),
        }
    }
}
impl LatexPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if !self.size.is_finite() || !(6.0..=144.0).contains(&self.size) {
            return Err("Default formula size must be 6–144 pt.".into());
        }
        if self.preset.len() > 128 {
            return Err("Invalid rendering preset.".into());
        }
        validate(&self.config, &LatexConfig::default())
    }
}
#[derive(Clone)]
struct Macro {
    args: usize,
    body: String,
}
type Macros = BTreeMap<String, Macro>;
fn group(input: &str, at: &mut usize) -> Result<String, String> {
    while input
        .as_bytes()
        .get(*at)
        .is_some_and(u8::is_ascii_whitespace)
    {
        *at += 1;
    }
    if input.as_bytes().get(*at) != Some(&b'{') {
        return Err("Expected a braced argument {...}.".into());
    }
    *at += 1;
    let start = *at;
    let mut depth = 1;
    while *at < input.len() {
        let b = input.as_bytes()[*at];
        if b == b'\\' {
            *at += 1;
            if *at < input.len() {
                *at += input[*at..].chars().next().unwrap().len_utf8();
            }
            continue;
        }
        if b == b'{' {
            depth += 1;
        }
        if b == b'}' {
            depth -= 1;
            if depth == 0 {
                let body = input[start..*at].to_owned();
                *at += 1;
                return Ok(body);
            }
        }
        *at += 1;
    }
    Err("Unclosed braced argument in LaTeX configuration.".into())
}
fn command(input: &str, at: &mut usize) -> String {
    let start = *at;
    *at += 1;
    while input
        .as_bytes()
        .get(*at)
        .is_some_and(u8::is_ascii_alphabetic)
    {
        *at += 1;
    }
    if *at == start + 1 && *at < input.len() {
        *at += input[*at..].chars().next().unwrap().len_utf8();
    }
    input[start..*at].into()
}
fn definitions(input: &str, macros: &mut Macros) -> Result<String, String> {
    if input.len() > 4096 || input.contains('\0') {
        return Err("Each preamble/postamble is limited to 4096 bytes without NUL.".into());
    }
    let mut out = String::new();
    let mut at = 0;
    while at < input.len() {
        if input.as_bytes()[at] == b'%' {
            while at < input.len() && input.as_bytes()[at] != b'\n' {
                at += 1;
            }
        } else if input.as_bytes()[at] == b'\\' {
            let name = command(input, &mut at);
            if name == r"\newcommand" || name == r"\renewcommand" {
                let key = group(input, &mut at)?;
                if !key.starts_with('\\')
                    || key.len() < 2
                    || !key[1..].bytes().all(|b| b.is_ascii_alphabetic())
                {
                    return Err("Macro names must be braced control words, e.g. {\\vect}.".into());
                }
                if macros.contains_key(&key) != (name == r"\renewcommand") {
                    return Err(format!(
                        "{name}: {key} is {}defined.",
                        if macros.contains_key(&key) {
                            "already "
                        } else {
                            "not "
                        }
                    ));
                }
                // Reject attempts to shadow engine primitives; only custom names are supported.
                if crate::formula_render::render(&format!("{key}{{x}}{{y}}"), [0, 0, 0, 255])
                    .is_ok()
                {
                    return Err(format!("Cannot redefine built-in command {key}."));
                }
                while input
                    .as_bytes()
                    .get(at)
                    .is_some_and(u8::is_ascii_whitespace)
                {
                    at += 1;
                }
                let args = if input.as_bytes().get(at) == Some(&b'[') {
                    at += 1;
                    let n = input
                        .as_bytes()
                        .get(at)
                        .copied()
                        .ok_or("Missing macro argument count.")?;
                    at += 1;
                    if !n.is_ascii_digit() || input.as_bytes().get(at) != Some(&b']') {
                        return Err("Macro argument count must be [0] through [9]; optional arguments are unsupported.".into());
                    }
                    at += 1;
                    (n - b'0') as usize
                } else {
                    0
                };
                let body = group(input, &mut at)?;
                for (i, c) in body.char_indices() {
                    if c == '#'
                        && !body
                            .as_bytes()
                            .get(i + 1)
                            .is_some_and(|b| (b'1'..=b'0' + args as u8).contains(b))
                    {
                        return Err("Macro body contains an invalid argument placeholder.".into());
                    }
                }
                macros.insert(key, Macro { args, body });
                if macros.len() > 64 {
                    return Err("At most 64 custom commands are supported.".into());
                }
            } else {
                out.push_str(&name);
            }
        } else {
            let c = input[at..].chars().next().unwrap();
            out.push(c);
            at += c.len_utf8();
        }
    }
    Ok(out)
}
fn expand(input: &str, macros: &Macros, depth: usize) -> Result<String, String> {
    if depth > 16 {
        return Err("Recursive macro or expansion nesting exceeds 16.".into());
    }
    let mut out = String::new();
    let mut at = 0;
    while at < input.len() {
        if input.as_bytes()[at] == b'\\' {
            let name = command(input, &mut at);
            if let Some(m) = macros.get(&name) {
                let mut args = Vec::new();
                for _ in 0..m.args {
                    args.push(group(input, &mut at)?);
                }
                let mut body = String::new();
                let mut chars = m.body.chars();
                while let Some(c) = chars.next() {
                    if c == '#' {
                        let index = chars
                            .next()
                            .ok_or("Invalid macro placeholder.")?
                            .to_digit(10)
                            .ok_or("Invalid macro placeholder.")?
                            as usize;
                        body.push_str(
                            args.get(index.wrapping_sub(1))
                                .ok_or("Invalid macro argument.")?,
                        );
                    } else {
                        body.push(c);
                    }
                }
                out.push_str(&expand(&body, macros, depth + 1)?);
            } else {
                out.push_str(&name);
            }
        } else {
            let c = input[at..].chars().next().unwrap();
            out.push(c);
            at += c.len_utf8();
        }
        if out.len() > crate::formula::MAX_FORMULA_BYTES {
            return Err("Expanded formula exceeds 16 KiB.".into());
        }
    }
    Ok(out)
}
/// Definitions are resolved in configuration order; remaining fragments are math wrappers.
/// Newlines terminate comments/control words without changing stored user input.
pub fn assemble(global: &LatexConfig, doc: &LatexConfig, source: &str) -> Result<String, String> {
    if source.len() > crate::formula::MAX_FORMULA_BYTES {
        return Err("Formula exceeds 16 KiB.".into());
    }
    let mut macros = Macros::new();
    let gp = definitions(&global.preamble, &mut macros)?;
    let dp = definitions(&doc.preamble, &mut macros)?;
    let ds = definitions(&doc.postamble, &mut macros)?;
    let gs = definitions(&global.postamble, &mut macros)?;
    // Validate unused definitions too; unsupported commands must never disappear silently.
    for m in macros.values() {
        let mut body = m.body.clone();
        for n in 1..=m.args {
            body = body.replace(&format!("#{n}"), "x");
        }
        crate::formula_render::render(&expand(&body, &macros, 0)?, [0, 0, 0, 255])
            .map_err(|e| format!("Invalid macro definition: {e}"))?;
    }
    expand(&format!("{gp}\n{dp}\n{source}\n{ds}\n{gs}"), &macros, 0)
}
pub fn validate(global: &LatexConfig, doc: &LatexConfig) -> Result<(), String> {
    crate::formula_render::render(&assemble(global, doc, "x")?, [0, 0, 0, 255]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inheritance_macros_wrappers_and_exact_input() {
        let global = LatexConfig {
            preamble: "\\newcommand{\\vect}[1]{\\mathbf{#1}}\n\\left( % comment\n".into(),
            postamble: r"\right)".into(),
        };
        let doc = LatexConfig {
            preamble: r"\renewcommand{\vect}[1]{\mathit{#1}}\left[".into(),
            postamble: r"\right]".into(),
        };
        let original = serde_json::to_string(&doc).unwrap();
        let assembled = assemble(&global, &doc, r"\vect{x}+\alpha").unwrap();
        assert!(assembled.contains(r"\mathit{x}+\alpha"));
        assert!(assembled.find(r"\left(").unwrap() < assembled.find(r"\left[").unwrap());
        validate(&global, &doc).unwrap();
        assert_eq!(serde_json::to_string(&doc).unwrap(), original);
        assert_eq!(serde_json::from_str::<LatexConfig>(&original).unwrap(), doc);
    }
    #[test]
    fn unsupported_unused_definitions_recursion_and_limits_are_errors() {
        for preamble in [
            r"\usepackage{amsmath}",
            r"\input{file}",
            r"\newcommand{\bad}{\unsupported}",
            r"\newcommand{\loop}{\loop}",
            r"\newcommand{\foo}[1]{#2}",
            r"\newcommand{\foo}[x]{x}",
            r"\newcommand{\foo}{x",
            r"\renewcommand{\missing}{x}",
            r"\newcommand{\sin}{x}",
        ] {
            let config = LatexConfig {
                preamble: preamble.into(),
                ..Default::default()
            };
            assert!(
                validate(&config, &LatexConfig::default()).is_err(),
                "{preamble}"
            );
        }
        assert!(
            validate(
                &LatexConfig {
                    preamble: "x".repeat(4097),
                    ..Default::default()
                },
                &LatexConfig::default()
            )
            .is_err()
        );
        let config = LatexConfig {
            preamble: r"\newcommand{\pair}[1]{#1#1#1#1#1#1#1#1#1}".into(),
            ..Default::default()
        };
        assert!(
            assemble(
                &config,
                &LatexConfig::default(),
                &format!(r"\pair{{{}}}", "x".repeat(3000))
            )
            .is_err()
        );
    }
    #[test]
    fn latin_modern_math_is_default_and_previous_presets_migrate() {
        for preset in ["stix-two-math", "computer-modern-math"] {
            let mut previous = crate::settings::Preferences::default();
            previous.latex.preset = preset.into();
            previous.latex.size = 32.;
            previous.latex.color = [20, 40, 60];
            previous.latex.config.preamble = r"\newcommand{\R}{\mathbb{R}}".into();
            let restored =
                crate::settings::Preferences::decode(&serde_json::to_string(&previous).unwrap())
                    .unwrap();
            previous.latex.preset = "latin-modern-math".into();
            assert_eq!(restored, previous);
        }
        assert_eq!(LatexPreferences::default().preset, "latin-modern-math");
    }

    #[test]
    fn preferences_persist_and_unknown_preset_falls_back_without_losing_text() {
        let mut prefs = crate::settings::Preferences::default();
        prefs.latex.preset = "removed-preset".into();
        prefs.latex.size = 36.;
        prefs.latex.config.preamble = "\\newcommand{\\R}{\\mathbb{R}}\n".into();
        let mut storage = crate::settings::tests::MemoryStorage::default();
        assert!(prefs.store(&mut storage));
        assert_eq!(
            crate::settings::Preferences::load(Some(&storage)).unwrap(),
            prefs
        );
        assert!(
            crate::settings::Preferences::decode("{}")
                .unwrap()
                .latex
                .high_quality
        );
    }
    #[test]
    fn document_configuration_is_atomic_isolated_undoable_and_not_pdf_persistence() {
        use crate::{
            document::{DocumentData, DocumentKind},
            drawing::{Drawing, Surface},
            formula::FormulaObject,
        };
        for kind in [DocumentKind::Page, DocumentKind::Whiteboard] {
            let mut data = DocumentData::blank(kind);
            let surface = if kind == DocumentKind::Page {
                Surface::Page(0)
            } else {
                Surface::Canvas
            };
            let mut a = Drawing::new(&data);
            let b = Drawing::new(&data);
            let mut f =
                FormulaObject::new(data.id(), surface, [30., 40.], "x".into(), [0, 0, 0, 255]);
            let r = crate::formula_render::render("x", f.color).unwrap();
            f.layout = crate::formula::FormulaLayout {
                width: r.width as f64,
                height: r.height as f64,
                baseline: r.baseline,
            };
            assert!(a.add_formula(f));
            let original = a.items().to_vec();
            let config = LatexConfig {
                preamble: r"\left(".into(),
                postamble: r"\right)".into(),
            };
            a.set_latex(config.clone(), &LatexConfig::default())
                .unwrap();
            assert_eq!(b.latex, LatexConfig::default());
            assert_eq!(a.latex, config);
            assert_ne!(a.items(), original);
            let changed = a.items().to_vec();
            a.undo();
            assert_eq!(a.items(), original);
            assert!(a.latex.is_default());
            a.redo();
            assert_eq!(a.items(), changed);
            assert_eq!(a.latex, config);
            assert!(
                a.set_latex(
                    LatexConfig {
                        preamble: r"\unsupported".into(),
                        ..Default::default()
                    },
                    &LatexConfig::default()
                )
                .is_err()
            );
            assert_eq!(a.items(), changed);
            assert_eq!(a.latex, config);
            data.latex = config;
            assert!(crate::pdf::encode(&data).is_err());
            assert_eq!(
                serde_json::from_str::<DocumentData>(&serde_json::to_string(&data).unwrap())
                    .unwrap(),
                data
            );
        }
    }
}
