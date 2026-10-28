//! Versioned application preferences. Never part of a document or its PDF payload.
use serde::{Deserialize, Serialize};

pub const STORAGE_KEY: &str = "XiToc/preferences";
const VERSION: u32 = 1;

#[derive(Clone, Copy, Default, Debug, PartialEq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DrawingPreferences {
    pub color: [u8; 4],
    pub width: f64,
    pub eraser_radius: f64,
}
impl Default for DrawingPreferences {
    fn default() -> Self {
        Self {
            color: [37, 99, 200, 255],
            width: 2.5,
            eraser_radius: 14.0,
        }
    }
}

// Future smoothing strength, eraser modes and pressure options belong in typed
// groups here, with validation and schema migration when their behavior exists.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    schema_version: u32,
    pub theme: Theme,
    pub explorer_visible: bool,
    pub drawing: DrawingPreferences,
    pub middle_button_pan: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            schema_version: VERSION,
            theme: Theme::Light,
            explorer_visible: true,
            drawing: DrawingPreferences::default(),
            middle_button_pan: true,
        }
    }
}
impl Preferences {
    pub fn decode(json: &str) -> Result<Self, String> {
        if json.len() > 16 * 1024 {
            return Err("Preferences exceed the supported size.".into());
        }
        let prefs: Self =
            serde_json::from_str(json).map_err(|e| format!("Cannot read preferences: {e}"))?;
        if prefs.schema_version != VERSION {
            return Err("Unsupported preferences version.".into());
        }
        if !prefs.drawing.width.is_finite()
            || !(0.25..=12.0).contains(&prefs.drawing.width)
            || !prefs.drawing.eraser_radius.is_finite()
            || !(4.0..=48.0).contains(&prefs.drawing.eraser_radius)
            || prefs.drawing.color[3] != 255
        {
            return Err("Invalid drawing preferences.".into());
        }
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
        canvas.color = self.drawing.color;
        canvas.width = self.drawing.width;
        canvas.eraser_radius = self.drawing.eraser_radius;
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
    fn preferences_round_trip_and_reject_invalid_or_future_data() {
        let mut storage = MemoryStorage::default();
        assert_eq!(
            Preferences::load(Some(&storage)).unwrap(),
            Preferences::default()
        );
        let prefs = Preferences {
            theme: Theme::Dark,
            explorer_visible: false,
            middle_button_pan: false,
            drawing: DrawingPreferences {
                width: 8.0,
                color: [2, 3, 4, 255],
                eraser_radius: 32.0,
            },
            ..Default::default()
        };
        prefs.store(&mut storage);
        assert_eq!(Preferences::load(Some(&storage)).unwrap(), prefs);
        for json in [
            "broken",
            r#"{"schema_version":2}"#,
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
