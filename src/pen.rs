//! Semantic pen styles and local preset library. No UI, filesystem or PDF types.
use crate::drawing::PressureStyle;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_PRESETS: usize = 32;
pub const DEFAULT_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PenType {
    #[default]
    Monoline,
    Pressure,
    Highlighter,
}
impl PenType {
    pub const ALL: [Self; 3] = [Self::Monoline, Self::Pressure, Self::Highlighter];
    pub fn label(self) -> &'static str {
        match self {
            Self::Monoline => "Monoline",
            Self::Pressure => "Pressure Pen",
            Self::Highlighter => "Highlighter",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PenStyle {
    pub pen_type: PenType,
    pub color: [u8; 3],
    pub width: f64,
    pub opacity: u8,
    pub smoothing_strength: u8,
    pub pressure: PressureStyle,
}
impl Default for PenStyle {
    fn default() -> Self {
        Self {
            pen_type: PenType::Monoline,
            color: [37, 99, 200],
            width: 2.5,
            opacity: 100,
            smoothing_strength: 50,
            pressure: PressureStyle {
                enabled: true,
                ..Default::default()
            },
        }
    }
}
impl PenStyle {
    pub fn validate(&self) -> bool {
        self.width.is_finite()
            && (0.25..=24.0).contains(&self.width)
            && (1..=100).contains(&self.opacity)
            && self.smoothing_strength <= 100
            && self.pressure.strength <= 100
            && (1..=100).contains(&self.pressure.minimum)
    }
    pub fn rgba(&self) -> [u8; 4] {
        [
            self.color[0],
            self.color[1],
            self.color[2],
            (u16::from(self.opacity) * 255 / 100) as u8,
        ]
    }
    pub fn effective_pressure(&self) -> PressureStyle {
        PressureStyle {
            enabled: self.pen_type == PenType::Pressure && self.pressure.enabled,
            ..self.pressure
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PenPreset {
    pub id: Uuid,
    pub name: String,
    pub style: PenStyle,
}
impl PenPreset {
    pub fn label(&self) -> &str {
        if self.name.trim().is_empty() {
            "Untitled pen"
        } else {
            &self.name
        }
    }
    pub fn builtin(&self) -> bool {
        (1..=DEFAULT_COUNT as u128).contains(&self.id.as_u128())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presets {
    pub items: Vec<PenPreset>,
    pub selected: Uuid,
}
impl Default for Presets {
    fn default() -> Self {
        let items = [
            ("Black pen", PenType::Monoline, [32, 39, 49], 2.5, 100),
            ("Blue pen", PenType::Monoline, [37, 99, 200], 2.5, 100),
            ("Red pen", PenType::Monoline, [200, 48, 60], 2.5, 100),
            (
                "Highlighter",
                PenType::Highlighter,
                [255, 206, 38],
                18.0,
                35,
            ),
            ("Pressure pen", PenType::Pressure, [98, 65, 164], 4.0, 100),
        ]
        .into_iter()
        .enumerate()
        .map(
            |(index, (name, pen_type, color, width, opacity))| PenPreset {
                id: Uuid::from_u128(index as u128 + 1),
                name: name.into(),
                style: PenStyle {
                    pen_type,
                    color,
                    width,
                    opacity,
                    ..Default::default()
                },
            },
        )
        .collect();
        Self {
            items,
            selected: Uuid::from_u128(2),
        }
    }
}
impl Presets {
    pub fn active(&self) -> &PenPreset {
        self.items
            .iter()
            .find(|p| p.id == self.selected)
            .expect("validated preset selection")
    }
    pub fn active_mut(&mut self) -> &mut PenPreset {
        self.items
            .iter_mut()
            .find(|p| p.id == self.selected)
            .expect("validated preset selection")
    }
    pub fn validate(&self) -> bool {
        let mut ids = std::collections::HashSet::new();
        !self.items.is_empty()
            && self.items.len() <= MAX_PRESETS
            && self.items.iter().any(|p| p.id == self.selected)
            && self.items.iter().all(|p| {
                !p.id.is_nil()
                    && ids.insert(p.id)
                    && p.name.chars().count() <= 64
                    && !p.name.chars().any(char::is_control)
                    && p.style.validate()
            })
            && (1..=DEFAULT_COUNT as u128).all(|id| ids.contains(&Uuid::from_u128(id)))
    }
    pub fn duplicate(&mut self) -> bool {
        if self.items.len() >= MAX_PRESETS {
            return false;
        }
        let mut preset = self.active().clone();
        preset.id = Uuid::new_v4();
        preset.name = format!("{} copy", preset.label())
            .chars()
            .take(64)
            .collect();
        self.selected = preset.id;
        self.items.push(preset);
        true
    }
    pub fn delete_selected(&mut self) -> bool {
        if self.active().builtin() {
            return false;
        }
        let index = self
            .items
            .iter()
            .position(|p| p.id == self.selected)
            .unwrap();
        self.items.remove(index);
        self.selected = self.items[index.min(self.items.len() - 1)].id;
        true
    }
    pub fn move_selected(&mut self, offset: isize) {
        let index = self
            .items
            .iter()
            .position(|p| p.id == self.selected)
            .unwrap();
        let target = index
            .saturating_add_signed(offset)
            .min(self.items.len() - 1);
        self.items.swap(index, target);
    }
    pub fn restore(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preset_lifecycle_preserves_identity_and_limits() {
        let mut presets = Presets::default();
        assert!(presets.validate());
        assert_eq!(
            presets
                .items
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            [
                "Black pen",
                "Blue pen",
                "Red pen",
                "Highlighter",
                "Pressure pen"
            ]
        );
        assert!(!presets.delete_selected());
        assert!(presets.duplicate());
        let custom = presets.selected;
        presets.active_mut().name = "Fine green".into();
        presets.active_mut().style.color = [0, 140, 80];
        presets.active_mut().style.width = 0.5;
        presets.move_selected(-3);
        assert_eq!(presets.items[2].id, custom);
        let decoded: Presets =
            serde_json::from_str(&serde_json::to_string(&presets).unwrap()).unwrap();
        assert_eq!(decoded, presets);
        assert!(presets.delete_selected());
        assert!(!presets.items.iter().any(|p| p.id == custom));
        while presets.items.len() < MAX_PRESETS {
            assert!(presets.duplicate());
        }
        assert!(!presets.duplicate());
        assert!(presets.validate());
        presets.restore();
        assert_eq!(presets, Presets::default());
    }
    #[test]
    fn registry_rejects_invalid_styles_ids_selection_and_future_types() {
        let base = Presets::default();
        for case in 0..9 {
            let mut invalid = base.clone();
            match case {
                0 => invalid.selected = Uuid::new_v4(),
                1 => invalid.items[0].id = invalid.items[1].id,
                2 => invalid.items[0].style.width = f64::NAN,
                3 => invalid.items[0].style.opacity = 0,
                4 => invalid.items[0].style.smoothing_strength = 101,
                5 => invalid.items[0].style.pressure.minimum = 0,
                6 => invalid.items[0].name = "x".repeat(65),
                7 => invalid.items[0].name = "a\nb".into(),
                _ => {
                    invalid.items.remove(0);
                }
            }
            assert!(!invalid.validate(), "invalid case {case}");
        }
        assert!(serde_json::from_str::<PenType>("\"Watercolor\"").is_err());
        let mut style = PenStyle::default();
        for kind in PenType::ALL {
            style.pen_type = kind;
            assert_eq!(
                style.effective_pressure().enabled,
                kind == PenType::Pressure
            );
            assert_eq!(style.effective_pressure().factor(None), 1.0);
        }
        style.opacity = 35;
        assert_eq!(style.rgba()[3], 89);
    }
}
