use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub show_installed_only: bool,
    pub delete_appx_after_download: bool,
    #[serde(rename = "VersionsApi")]
    pub versions_api_uwp: String,
    pub versions_api_gdk: String,
    pub has_previously_used_gdk: bool,
    pub show_legacy_beta_tab: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_installed_only: false,
            delete_appx_after_download: true,
            versions_api_uwp: String::new(),
            versions_api_gdk: String::new(),
            has_previously_used_gdk: false,
            show_legacy_beta_tab: false,
        }
    }
}
