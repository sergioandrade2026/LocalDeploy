use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub font_size: u16,
    pub theme: String,
    pub sync_seconds: u64,
    pub run_tests: bool,
    pub backend: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            font_size: 16,
            theme: "Claro".into(),
            sync_seconds: 30,
            run_tests: true,
            backend: "Docker".into(),
        }
    }
}
pub fn load() -> Preferences {
    crate::storage::app_data_dir()
        .ok()
        .and_then(|root| std::fs::read(root.join("preferences.json")).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}
pub fn save(value: &Preferences) -> Result<(), String> {
    if !(14..=22).contains(&value.font_size)
        || !(15..=300).contains(&value.sync_seconds)
        || !["Claro", "Escuro"].contains(&value.theme.as_str())
        || !["Docker", "Nativo"].contains(&value.backend.as_str())
    {
        return Err("Preferências fora dos limites permitidos.".into());
    }
    let root = crate::storage::app_data_dir().map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("preferences.json"),
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
