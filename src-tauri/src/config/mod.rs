use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PopupPosition {
    #[default]
    NearCursor,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub mouse_enabled: bool,
    pub sensitivity: f64,
    pub cooldown_ms: u64,
    #[serde(default)]
    pub popup_position: PopupPosition,
    #[serde(default)]
    pub model: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mouse_enabled: true,
            sensitivity: 1.0,
            cooldown_ms: 2500,
            popup_position: PopupPosition::NearCursor,
            model: None,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.sensitivity.is_finite() || !(0.5..=2.0).contains(&self.sensitivity) {
            return Err("Sensitivity must be between 0.5 and 2.0".into());
        }
        if !(500..=30000).contains(&self.cooldown_ms) {
            return Err("Cooldown must be between 500 and 30000 ms".into());
        }
        if self.model.as_ref().is_some_and(|model| {
            model.is_empty() || model.len() > 128 || model.chars().any(char::is_control)
        }) {
            return Err("Model selection is invalid".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        match fs::read(path) {
            Ok(bytes) => {
                let settings: Self =
                    serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
                settings.validate()?;
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.to_string()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let parent = path.parent().ok_or("Invalid settings path")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        fs::rename(temporary, path).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_corner_choice_to_disk() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "wiggy-settings-test-{}-{unique}",
            std::process::id()
        ));
        let path = directory.join("settings.json");
        let settings = Settings {
            popup_position: PopupPosition::TopLeft,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        let updated = Settings {
            popup_position: PopupPosition::BottomRight,
            ..settings
        };
        updated.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), updated);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn migrates_old_settings_and_roundtrips_position() {
        let previous: Settings =
            serde_json::from_str(r#"{"mouse_enabled":true,"sensitivity":1.0,"cooldown_ms":2500}"#)
                .unwrap();
        assert_eq!(previous.popup_position, PopupPosition::NearCursor);
        let settings = Settings {
            popup_position: PopupPosition::BottomRight,
            ..previous
        };
        let decoded: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(settings, decoded);
        assert!(serde_json::from_str::<Settings>(r#"{"mouse_enabled":true,"sensitivity":1.0,"cooldown_ms":2500,"popup_position":"invalid"}"#).is_err());
    }

    #[test]
    fn validates_bounds_and_nan() {
        assert!(Settings::default().validate().is_ok());
        assert!(Settings {
            sensitivity: f64::NAN,
            ..Settings::default()
        }
        .validate()
        .is_err());
        assert!(Settings {
            cooldown_ms: 0,
            ..Settings::default()
        }
        .validate()
        .is_err());
    }
}
