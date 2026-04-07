use crate::error::{Error, Result};
use crate::types::{PackageType, Version, VersionType};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_VERSIONS_API_UWP: &str = "https://mrarm.io/r/w10-vdb";
pub const DEFAULT_VERSIONS_API_GDK: &str = "https://raw.githubusercontent.com/MinecraftBedrockArchiver/GdkLinks/refs/heads/master/urls.min.json";

const GDK_CONFIG_FILENAME: &str = "MicrosoftGame.Config";

pub struct VersionList {
    versions_api_uwp: String,
    versions_api_gdk: String,
    cache_file_uwp: PathBuf,
    cache_file_gdk: PathBuf,
    imported_directory: PathBuf,
    client: reqwest::Client,
    versions: Vec<Version>,
    db_versions: HashSet<String>,
}

impl VersionList {
    pub fn new(
        cache_file_uwp: impl Into<PathBuf>,
        cache_file_gdk: impl Into<PathBuf>,
        imported_directory: impl Into<PathBuf>,
        versions_api_uwp: impl Into<String>,
        versions_api_gdk: impl Into<String>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent("mclauncher/0.1")
            .build()?;
        Ok(Self {
            versions_api_uwp: versions_api_uwp.into(),
            versions_api_gdk: versions_api_gdk.into(),
            cache_file_uwp: cache_file_uwp.into(),
            cache_file_gdk: cache_file_gdk.into(),
            imported_directory: imported_directory.into(),
            client,
            versions: Vec::new(),
            db_versions: HashSet::new(),
        })
    }

    pub fn versions(&self) -> &[Version] {
        &self.versions
    }

    pub fn prepare_for_reload(&mut self) {
        self.versions.retain(|v| v.version_type == VersionType::Imported);
    }

    pub async fn load_from_cache_uwp(&mut self) -> Result<()> {
        let data = match fs::read_to_string(&self.cache_file_uwp) {
            Ok(data) => data,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        let value: serde_json::Value = serde_json::from_str(&data)?;
        self.parse_list_uwp(&value, true)?;
        Ok(())
    }

    pub async fn load_from_cache_gdk(&mut self) -> Result<()> {
        let data = match fs::read_to_string(&self.cache_file_gdk) {
            Ok(data) => data,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err.into()),
        };
        let value: serde_json::Value = serde_json::from_str(&data)?;
        self.parse_data_gdk(&value, true)?;
        Ok(())
    }

    pub async fn download_versions_uwp(&mut self) -> Result<()> {
        let resp = self.client.get(&self.versions_api_uwp).send().await?;
        let resp = resp.error_for_status()?;
        let data = resp.text().await?;
        fs::write(&self.cache_file_uwp, &data)?;
        let value: serde_json::Value = serde_json::from_str(&data)?;
        self.parse_list_uwp(&value, false)?;
        Ok(())
    }

    pub async fn download_versions_gdk(&mut self) -> Result<()> {
        let resp = self.client.get(&self.versions_api_gdk).send().await?;
        let resp = resp.error_for_status()?;
        let data = resp.text().await?;
        fs::write(&self.cache_file_gdk, &data)?;
        let value: serde_json::Value = serde_json::from_str(&data)?;
        self.parse_data_gdk(&value, false)?;
        Ok(())
    }

    pub fn load_imported(&mut self) -> Result<()> {
        if !self.imported_directory.exists() {
            return Ok(());
        }
        let entries = fs::read_dir(&self.imported_directory)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue,
            };
            if self
                .db_versions
                .insert(format!("IMPORTED_{}", name))
            {
                let package_type = if path.join(GDK_CONFIG_FILENAME).is_file() {
                    PackageType::Gdk
                } else {
                    PackageType::Uwp
                };
                self.versions
                    .push(Version::new_imported(name, path, package_type));
            }
        }
        Ok(())
    }

    fn parse_list_uwp(&mut self, data: &serde_json::Value, is_cache: bool) -> Result<()> {
        let array = data
            .as_array()
            .ok_or_else(|| Error::InvalidResponse("UWP list is not an array".into()))?;
        for item in array.iter().rev() {
            let entry = item
                .as_array()
                .ok_or_else(|| Error::InvalidResponse("UWP entry is not an array".into()))?;
            if entry.len() < 3 {
                return Err(Error::InvalidResponse(
                    "UWP entry does not have 3 fields".into(),
                ));
            }
            let name = entry[0]
                .as_str()
                .ok_or_else(|| Error::InvalidResponse("UWP name is not a string".into()))?
                .to_string();
            let uuid = entry[1]
                .as_str()
                .ok_or_else(|| Error::InvalidResponse("UWP uuid is not a string".into()))?
                .to_string();
            let version_type_raw = entry[2]
                .as_i64()
                .ok_or_else(|| Error::InvalidResponse("UWP version type is not an int".into()))?;
            let version_type = match VersionType::from_i64(version_type_raw) {
                Some(v) => v,
                None => continue,
            };
            if version_type == VersionType::Imported {
                continue;
            }
            let exists = !self.db_versions.insert(name.clone());
            let is_new = !exists && !is_cache;
            self.versions.push(Version::new_from_db(
                uuid,
                name,
                version_type,
                is_new,
                PackageType::Uwp,
                Vec::new(),
            ));
        }
        Ok(())
    }

    fn parse_data_gdk(&mut self, data: &serde_json::Value, is_cache: bool) -> Result<()> {
        let release = data
            .get("release")
            .ok_or_else(|| Error::InvalidResponse("GDK data missing release".into()))?;
        let preview = data
            .get("preview")
            .ok_or_else(|| Error::InvalidResponse("GDK data missing preview".into()))?;
        self.parse_list_gdk(release, is_cache, VersionType::Release)?;
        self.parse_list_gdk(preview, is_cache, VersionType::Preview)?;
        Ok(())
    }

    fn parse_list_gdk(
        &mut self,
        data: &serde_json::Value,
        is_cache: bool,
        version_type: VersionType,
    ) -> Result<()> {
        let obj = data
            .as_object()
            .ok_or_else(|| Error::InvalidResponse("GDK list is not an object".into()))?;
        for (version_name, urls_value) in obj.iter() {
            let urls_array = urls_value.as_array().ok_or_else(|| {
                Error::InvalidResponse("GDK urls entry is not an array".into())
            })?;
            let mut download_urls = Vec::new();
            for url in urls_array {
                if let Some(url) = url.as_str() {
                    download_urls.push(url.to_string());
                }
            }
            if download_urls.is_empty() {
                continue;
            }
            let exists = !self.db_versions.insert(version_name.clone());
            let is_new = !exists && !is_cache;
            self.versions.push(Version::new_from_db(
                crate::types::UNKNOWN_UUID.to_string(),
                version_name.clone(),
                version_type,
                is_new,
                PackageType::Gdk,
                download_urls,
            ));
        }
        Ok(())
    }
}
