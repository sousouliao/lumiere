use lumiere_capture_contract::{CaptureMode, Delivery};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AfterCapture {
    DoNothing,
    ShowInFolder,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shortcuts {
    pub region: Option<String>,
    pub display: Option<String>,
}
impl Shortcuts {
    pub fn get(&self, mode: CaptureMode) -> &Option<String> {
        match mode {
            CaptureMode::Region => &self.region,
            CaptureMode::Display => &self.display,
        }
    }
    pub fn set(&mut self, mode: CaptureMode, value: Option<String>) {
        *match mode {
            CaptureMode::Region => &mut self.region,
            CaptureMode::Display => &mut self.display,
        } = value;
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    version: u8,
    pub output_delivery: Delivery,
    pub save_directory: Option<PathBuf>,
    pub capture_shortcuts: Shortcuts,
    pub after_capture_behavior: AfterCapture,
    pub hdr_status_reminders: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 5,
            output_delivery: Delivery::Both,
            save_directory: None,
            capture_shortcuts: Shortcuts::default(),
            after_capture_behavior: AfterCapture::DoNothing,
            hdr_status_reminders: true,
        }
    }
}
impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .and_then(|value| Self::parse(value).ok())
            .unwrap_or_default()
    }
    fn parse(mut value: Value) -> Result<Self, String> {
        let object = value.as_object_mut().ok_or("Invalid settings")?;
        let version = object
            .get("version")
            .and_then(Value::as_u64)
            .ok_or("Invalid settings version")?;
        let keys: &[&str] = match version {
            1 => &["version", "outputDelivery"],
            2 => &["version", "outputDelivery", "captureShortcuts"],
            3 => &[
                "version",
                "outputDelivery",
                "captureShortcuts",
                "afterCaptureBehavior",
            ],
            4 => &[
                "version",
                "outputDelivery",
                "captureShortcuts",
                "afterCaptureBehavior",
                "hdrStatusReminders",
            ],
            5 => &[
                "version",
                "outputDelivery",
                "captureShortcuts",
                "afterCaptureBehavior",
                "hdrStatusReminders",
                "saveDirectory",
            ],
            _ => return Err("Unsupported settings version".into()),
        };
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err("Invalid settings shape".into());
        }
        object.insert("version".into(), json!(5));
        object
            .entry("captureShortcuts")
            .or_insert(json!({"region":null,"display":null}));
        object
            .entry("afterCaptureBehavior")
            .or_insert(json!("do-nothing"));
        object.entry("hdrStatusReminders").or_insert(json!(true));
        object.entry("saveDirectory").or_insert(Value::Null);
        let mut settings: Self =
            serde_json::from_value(value).map_err(|error| error.to_string())?;
        settings.validate()?;
        for mode in [CaptureMode::Region, CaptureMode::Display] {
            let normalized = settings
                .capture_shortcuts
                .get(mode)
                .as_deref()
                .map(normalize_shortcut)
                .transpose()?;
            settings.capture_shortcuts.set(mode, normalized);
        }
        Ok(settings)
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(path) = &self.save_directory
            && (!path.is_absolute() || path.as_os_str().is_empty())
        {
            return Err("Invalid save directory".into());
        }
        for shortcut in [
            &self.capture_shortcuts.region,
            &self.capture_shortcuts.display,
        ]
        .into_iter()
        .flatten()
        {
            normalize_shortcut(shortcut)?;
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let parent = path.parent().ok_or("Invalid settings location")?;
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let temp = path.with_extension("json.tmp");
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        bytes.push(b'\n');
        std::fs::write(&temp, bytes).map_err(|error| error.to_string())?;
        std::fs::rename(&temp, path).map_err(|error| error.to_string())
    }
}

/// Persist the existing canonical Electron spelling, translate only at registration.
pub fn normalize_shortcut(value: &str) -> Result<String, String> {
    let modifiers = ["Command", "Control", "Alt", "Shift"];
    let mut parts: Vec<_> = value.split('+').collect();
    let key = parts.pop().ok_or("Shortcut is unsupported")?;
    let supported = (key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase())
        || (key.len() == 1 && key.as_bytes()[0].is_ascii_digit())
        || ["Down", "Left", "Right", "Up", "Space"].contains(&key)
        || key
            .strip_prefix('F')
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=24).contains(&number) && key == format!("F{number}"));
    if value.len() > 80
        || !supported
        || parts.is_empty()
        || parts.iter().any(|part| !modifiers.contains(part))
        || !parts
            .iter()
            .any(|part| ["Command", "Control", "Alt"].contains(part))
        || parts
            .iter()
            .enumerate()
            .any(|(index, part)| parts[..index].contains(part))
    {
        return Err("Shortcut accelerator is unsupported.".into());
    }
    let mut canonical: Vec<_> = modifiers
        .into_iter()
        .filter(|part| parts.contains(part))
        .collect();
    canonical.push(key);
    Ok(canonical.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates_old_versions_and_rejects_invalid_shapes() {
        for version in 1..=5 {
            let mut value = json!({"version":version,"outputDelivery":"folder"});
            if version >= 2 {
                value["captureShortcuts"] = json!({"region":"Shift+Control+R","display":null});
            }
            if version >= 3 {
                value["afterCaptureBehavior"] = json!("show-in-folder");
            }
            if version >= 4 {
                value["hdrStatusReminders"] = json!(false);
            }
            if version >= 5 {
                value["saveDirectory"] = json!("C:\\Screenshots");
            }
            let settings = Settings::parse(value.clone()).unwrap();
            assert_eq!(settings.version, 5);
            if version >= 2 {
                assert_eq!(
                    settings.capture_shortcuts.region.as_deref(),
                    Some("Control+Shift+R")
                );
            }
            assert_eq!(settings.hdr_status_reminders, version < 4);
            value["unexpected"] = json!(true);
            assert!(Settings::parse(value).is_err());
        }
        assert_eq!(
            normalize_shortcut("Shift+Control+R").unwrap(),
            "Control+Shift+R"
        );
        for invalid in [
            "R",
            "Shift+R",
            "Control+Control+R",
            "Alt+F01",
            "Control+Delete",
        ] {
            assert!(normalize_shortcut(invalid).is_err());
        }
    }
}
