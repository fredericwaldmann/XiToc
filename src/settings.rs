//! Versioned application preferences. Never part of a document or its PDF payload.
use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "XiToc/preferences";
const VERSION: u32 = 7;

#[derive(Clone, Copy, Default, Debug, PartialEq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DrawingPreferences {
    pub snapping: crate::snapping::SnapSettings,
    pub shape_defaults: crate::shape::ShapeDefaults,
    pub presets: crate::pen::Presets,
    pub eraser_radius: f64,
    pub eraser_mode: crate::drawing::EraserMode,
    pub sticky_note_color: [u8; 3],
}
impl Default for DrawingPreferences {
    fn default() -> Self {
        Self {
            snapping: Default::default(),
            shape_defaults: Default::default(),
            presets: Default::default(),
            eraser_radius: 14.0,
            eraser_mode: Default::default(),
            sticky_note_color: [255, 244, 177],
        }
    }
}

/// Decode legacy preference schemas without retaining a second live pen state.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LegacyDrawing {
    color: [u8; 4],
    width: f64,
    smoothing_strength: u8,
    pressure: crate::drawing::PressureStyle,
    eraser_radius: f64,
    eraser_mode: crate::drawing::EraserMode,
}
impl Default for LegacyDrawing {
    fn default() -> Self {
        Self {
            color: [37, 99, 200, 255],
            width: 2.5,
            smoothing_strength: 50,
            pressure: crate::drawing::PressureStyle {
                enabled: true,
                ..Default::default()
            },
            eraser_radius: 14.0,
            eraser_mode: Default::default(),
        }
    }
}

// Application preferences stay separate from captured document ink.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    schema_version: u32,
    pub theme: Theme,
    pub explorer_visible: bool,
    pub drawing: DrawingPreferences,
    pub middle_button_pan: bool,
    pub text_resize: crate::text::ResizeBehavior,
    pub latex: crate::latex_config::LatexPreferences,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema_version: VERSION,
            theme: Theme::Light,
            explorer_visible: true,
            drawing: DrawingPreferences::default(),
            middle_button_pan: true,
            text_resize: Default::default(),
            latex: Default::default(),
        }
    }
}
impl Preferences {
    pub fn decode(json: &str) -> Result<Self, String> {
        if json.len() > 64 * 1024 {
            return Err("Preferences exceed the supported size.".into());
        }
        let mut value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("Cannot read preferences: {e}"))?;
        let version = value
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(VERSION as u64);
        if (1..=4).contains(&version)
            || (value.get("schema_version").is_none()
                && value
                    .get("drawing")
                    .is_some_and(|d| d.get("presets").is_none()))
        {
            let old: LegacyDrawing = serde_json::from_value(
                value
                    .get("drawing")
                    .cloned()
                    .unwrap_or(serde_json::json!({})),
            )
            .map_err(|e| format!("Cannot migrate drawing preferences: {e}"))?;
            if !old.width.is_finite() || !(0.25..=12.0).contains(&old.width) || old.color[3] != 255
            {
                return Err("Invalid legacy ink preferences.".into());
            }
            let mut drawing = DrawingPreferences {
                eraser_radius: old.eraser_radius,
                eraser_mode: old.eraser_mode,
                ..Default::default()
            };
            // Preserve customized legacy ink as its own preset, with sensible defaults alongside.
            if version <= 4
                || old.color != [37, 99, 200, 255]
                || old.width != 2.5
                || old.smoothing_strength != 50
                || old.pressure != LegacyDrawing::default().pressure
            {
                drawing.presets.duplicate();
                let migrated = drawing.presets.active_mut();
                migrated.id = uuid::Uuid::from_u128(100);
                migrated.name = "Migrated pen".into();
                migrated.style = crate::pen::PenStyle {
                    pen_type: if old.pressure.enabled {
                        crate::pen::PenType::Pressure
                    } else {
                        crate::pen::PenType::Monoline
                    },
                    color: [old.color[0], old.color[1], old.color[2]],
                    width: old.width,
                    opacity: 100,
                    smoothing_strength: old.smoothing_strength,
                    pressure: old.pressure,
                };
            }
            if drawing
                .presets
                .items
                .last()
                .is_some_and(|p| p.id == uuid::Uuid::from_u128(100))
            {
                drawing.presets.selected = uuid::Uuid::from_u128(100);
            }
            let object = value
                .as_object_mut()
                .ok_or("Preferences must be an object.")?;
            object.insert(
                "drawing".into(),
                serde_json::to_value(drawing).expect("serializable drawing defaults"),
            );
            object.insert("schema_version".into(), VERSION.into());
        } else if version == 5 || version == 6 {
            value
                .as_object_mut()
                .ok_or("Preferences must be an object.")?
                .insert("schema_version".into(), VERSION.into());
        }
        let mut prefs: Self =
            serde_json::from_value(value).map_err(|e| format!("Cannot read preferences: {e}"))?;
        if prefs.schema_version != VERSION {
            return Err("Unsupported preferences version.".into());
        }
        if !prefs.drawing.snapping.valid()
            || !prefs.drawing.shape_defaults.valid()
            || !prefs.drawing.presets.validate()
            || !prefs.drawing.eraser_radius.is_finite()
            || !(4.0..=48.0).contains(&prefs.drawing.eraser_radius)
        {
            return Err("Invalid drawing preferences.".into());
        }
        if matches!(
            prefs.latex.preset.as_str(),
            "stix-two-math" | "computer-modern-math"
        ) {
            prefs.latex.preset = "latin-modern-math".into();
        }
        prefs.latex.validate()?;
        Ok(prefs)
    }
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Result<Self, String> {
        match storage.and_then(|s| s.get_string(STORAGE_KEY)) {
            Some(json) => Self::decode(&json),
            None => Ok(Self::default()),
        }
    }
    pub fn store(&self, storage: &mut dyn eframe::Storage) -> bool {
        let json = serde_json::to_string(self).expect("validated preferences");
        storage.set_string(STORAGE_KEY, json.clone());
        storage.get_string(STORAGE_KEY).as_deref() == Some(json.as_str())
    }
    pub fn apply_drawing(&self, canvas: &mut crate::canvas::Canvas) {
        canvas.latex = self.latex.clone();
        canvas.drawing.latex_global = self.latex.config.clone();
        canvas.snapping = self.drawing.snapping;
        canvas.text_resize_default = self.text_resize;
        canvas.shape_defaults = self.drawing.shape_defaults.clone();
        canvas.apply_pen(&self.drawing.presets.active().style);
        canvas.eraser_radius = self.drawing.eraser_radius;
        canvas.eraser_mode = self.drawing.eraser_mode;
        canvas.sticky_note_color = self.drawing.sticky_note_color;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    #[derive(Default)]
    pub struct MemoryStorage(pub std::collections::HashMap<String, String>);
    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }
    #[test]
    fn legacy_migration_is_stable_and_preserves_all_ink_fields() {
        for version in 1..=4 {
            let json = format!(
                r#"{{"schema_version":{version},"drawing":{{"color":[3,44,100,255],"width":7.5,"smoothing_strength":25,"pressure":{{"enabled":true,"strength":60,"minimum":40}},"eraser_radius":20.0,"eraser_mode":"Partial"}}}}"#
            );
            let prefs = Preferences::decode(&json).unwrap();
            assert_eq!(prefs, Preferences::decode(&json).unwrap());
            let active = prefs.drawing.presets.active();
            assert_eq!(active.name, "Migrated pen");
            assert_eq!(active.style.width, 7.5);
            assert_eq!(active.style.color, [3, 44, 100]);
            assert_eq!(active.style.opacity, 100);
            assert_eq!(active.style.pen_type, crate::pen::PenType::Pressure);
            assert_eq!(active.style.smoothing_strength, 25);
            assert_eq!(active.style.pressure.strength, 60);
            assert_eq!(active.style.pressure.minimum, 40);
            assert_eq!(
                prefs.drawing.eraser_mode,
                crate::drawing::EraserMode::Partial
            );
            assert_eq!(prefs.drawing.eraser_radius, 20.0);
            assert_eq!(
                Preferences::decode(&serde_json::to_string(&prefs).unwrap()).unwrap(),
                prefs
            );
        }
        let mut prefs = Preferences::default();
        prefs.drawing.presets.selected = uuid::Uuid::new_v4();
        assert!(Preferences::decode(&serde_json::to_string(&prefs).unwrap()).is_err());
    }

    #[test]
    fn pressure_preferences_migrate_validate_persist_and_apply() {
        for version in 1..=3 {
            let prefs = Preferences::decode(&format!(r#"{{"schema_version":{version}}}"#)).unwrap();
            assert_eq!(prefs.schema_version, VERSION);
            assert!(prefs.drawing.presets.active().style.pressure.enabled);
        }
        for strength in [0, 50, 100] {
            for minimum in [1, 20, 100] {
                let mut prefs = Preferences::default();
                prefs.drawing.presets.active_mut().style.pen_type = crate::pen::PenType::Pressure;
                prefs.drawing.presets.active_mut().style.pressure = crate::drawing::PressureStyle {
                    enabled: strength != 0,
                    strength,
                    minimum,
                };
                let mut storage = MemoryStorage::default();
                assert!(prefs.store(&mut storage));
                assert_eq!(Preferences::load(Some(&storage)).unwrap(), prefs);
                let data =
                    crate::document::DocumentData::blank(crate::document::DocumentKind::Whiteboard);
                let mut canvas = crate::canvas::Canvas::new(&data);
                prefs.apply_drawing(&mut canvas);
                assert_eq!(
                    canvas.pressure_style,
                    prefs.drawing.presets.active().style.effective_pressure()
                );
            }
        }
        for json in [
            r#"{"drawing":{"pressure":{"strength":101}}}"#,
            r#"{"drawing":{"pressure":{"minimum":0}}}"#,
        ] {
            assert!(Preferences::decode(json).is_err());
        }
    }

    #[test]
    fn eraser_modes_migrate_and_persist_independently_of_smoothing() {
        use crate::drawing::EraserMode;
        for json in [
            r#"{"schema_version":1}"#,
            r#"{"schema_version":2,"drawing":{"smoothing_strength":75}}"#,
        ] {
            let migrated = Preferences::decode(json).unwrap();
            assert_eq!(migrated.schema_version, VERSION);
            assert_eq!(migrated.drawing.eraser_mode, EraserMode::Stroke);
        }
        for mode in [EraserMode::Stroke, EraserMode::Partial] {
            let mut prefs = Preferences::default();
            prefs.drawing.eraser_mode = mode;
            let mut storage = MemoryStorage::default();
            prefs.store(&mut storage);
            let restored = Preferences::load(Some(&storage)).unwrap();
            assert_eq!(restored.drawing.eraser_mode, mode);
            assert_eq!(
                restored.drawing.presets.active().style.smoothing_strength,
                50
            );
        }
        assert!(Preferences::decode(r#"{"drawing":{"eraser_mode":"Unknown"}}"#).is_err());
    }

    #[test]
    fn sticky_note_default_color_migrates_persists_and_applies_only_to_future_notes() {
        let migrated = Preferences::decode(r#"{"schema_version":5,"drawing":{}}"#).unwrap();
        assert_eq!(migrated.drawing.sticky_note_color, [255, 244, 177]);
        let mut prefs = Preferences::default();
        prefs.drawing.sticky_note_color = [205, 231, 255];
        let mut storage = MemoryStorage::default();
        assert!(prefs.store(&mut storage));
        let loaded = Preferences::load(Some(&storage)).unwrap();
        assert_eq!(loaded.drawing.sticky_note_color, [205, 231, 255]);
        let data = crate::document::DocumentData::blank(crate::document::DocumentKind::Whiteboard);
        let mut canvas = crate::canvas::Canvas::new(&data);
        loaded.apply_drawing(&mut canvas);
        assert_eq!(canvas.sticky_note_color, [205, 231, 255]);
        let note = crate::shape::Shape::create_sticky_note(
            data.id(),
            crate::drawing::Surface::Canvas,
            [40., 50.],
            [180., 140.],
            canvas.sticky_note_color,
        )
        .unwrap();
        assert_eq!(note.style.fill, Some([205, 231, 255]));
        let mut changed_default = loaded;
        changed_default.drawing.sticky_note_color = [255, 218, 226];
        assert_eq!(changed_default.drawing.sticky_note_color, [255, 218, 226]);
        assert_eq!(note.style.fill, Some([205, 231, 255]));
    }

    #[test]
    fn smoothing_preferences_migrate_and_persist_every_strength() {
        let old = Preferences::decode(r#"{"schema_version":1,"drawing":{"width":6.0}}"#).unwrap();
        assert_eq!(old.schema_version, VERSION);
        assert_eq!(old.drawing.presets.active().style.smoothing_strength, 50);
        assert_eq!(old.drawing.presets.active().style.width, 6.0);
        for strength in [0, 25, 50, 75, 100] {
            let mut prefs = old.clone();
            prefs.drawing.presets.active_mut().style.smoothing_strength = strength;
            let mut storage = MemoryStorage::default();
            prefs.store(&mut storage);
            assert_eq!(Preferences::load(Some(&storage)).unwrap(), prefs);
        }
        assert!(Preferences::decode(r#"{"drawing":{"smoothing_strength":101}}"#).is_err());
    }

    #[test]
    fn preferences_round_trip_and_reject_invalid_or_future_data() {
        let mut storage = MemoryStorage::default();
        assert_eq!(
            Preferences::load(Some(&storage)).unwrap(),
            Preferences::default()
        );
        let mut prefs = Preferences {
            theme: Theme::Dark,
            explorer_visible: false,
            middle_button_pan: false,
            ..Default::default()
        };
        prefs.drawing.eraser_radius = 32.0;
        prefs.drawing.eraser_mode = crate::drawing::EraserMode::Partial;
        let pen = &mut prefs.drawing.presets.active_mut().style;
        pen.width = 8.0;
        pen.color = [2, 3, 4];
        pen.smoothing_strength = 75;
        pen.pressure = crate::drawing::PressureStyle {
            enabled: true,
            strength: 75,
            minimum: 30,
        };
        prefs.store(&mut storage);
        assert_eq!(Preferences::load(Some(&storage)).unwrap(), prefs);
        for json in [
            "broken",
            r#"{"schema_version":8}"#,
            r#"{"drawing":{"width":0.0}}"#,
            r#"{"drawing":{"eraser_radius":49.0}}"#,
            r#"{"drawing":{"color":[0,0,0,0]}}"#,
            r#"{"unknown":true}"#,
        ] {
            assert!(Preferences::decode(json).is_err(), "{json}");
        }
        assert_eq!(Preferences::decode("{}").unwrap(), Preferences::default());
    }
}

#[cfg(test)]
mod snapping_preferences_tests {
    use super::*;
    #[test]
    fn snapping_defaults_migrate_validate_and_remain_separate_from_active_editors() {
        for version in [5, 6, 7] {
            let json = format!(
                r#"{{"schema_version":{version},"drawing":{{"presets":{}}}}}"#,
                serde_json::to_string(&crate::pen::Presets::default()).unwrap()
            );
            let preferences = Preferences::decode(&json).unwrap();
            assert_eq!(preferences.drawing.snapping, Default::default());
        }
        let mut p = Preferences::default();
        p.drawing.snapping.grid = true;
        p.drawing.snapping.show_grid = false;
        p.drawing.snapping.spacing = 24.;
        p.drawing.snapping.sensitivity = crate::snapping::Sensitivity::High;
        let mut storage = tests::MemoryStorage::default();
        assert!(p.store(&mut storage));
        assert_eq!(Preferences::load(Some(&storage)).unwrap(), p);
        for bad in [0., 5000.] {
            let mut value = serde_json::to_value(&p).unwrap();
            value["drawing"]["snapping"]["spacing"] = bad.into();
            assert!(Preferences::decode(&value.to_string()).is_err());
        }
        let doc = crate::document::DocumentData::blank(crate::document::DocumentKind::Page);
        let mut c = crate::canvas::Canvas::new(&doc);
        p.apply_drawing(&mut c);
        assert_eq!(c.snapping, p.drawing.snapping);
        c.snapping.objects = false;
        assert!(p.drawing.snapping.objects);
        assert!(!c.dirty());
    }
}
