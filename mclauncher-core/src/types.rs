use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const UNKNOWN_UUID: &str = "UNKNOWN";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum VersionType {
    Release = 0,
    Beta = 1,
    Preview = 2,
    Imported = 100,
}

impl VersionType {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(VersionType::Release),
            1 => Some(VersionType::Beta),
            2 => Some(VersionType::Preview),
            100 => Some(VersionType::Imported),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageType {
    Uwp,
    Gdk,
}

pub struct PackageFamilies;

impl PackageFamilies {
    pub const MINECRAFT: &'static str = "Microsoft.MinecraftUWP_8wekyb3d8bbwe";
    pub const MINECRAFT_PREVIEW: &'static str = "Microsoft.MinecraftWindowsBeta_8wekyb3d8bbwe";
}

#[derive(Debug, Clone)]
pub struct Version {
    pub uuid: String,
    pub name: String,
    pub version_type: VersionType,
    pub is_new: bool,
    pub package_type: PackageType,
    pub download_urls: Vec<String>,
    pub game_directory: PathBuf,
}

impl Version {
    pub fn new_from_db(
        uuid: String,
        name: String,
        version_type: VersionType,
        is_new: bool,
        package_type: PackageType,
        download_urls: Vec<String>,
    ) -> Self {
        let game_directory = resolve_game_directory(&name, version_type, package_type);
        Self {
            uuid,
            name,
            version_type,
            is_new,
            package_type,
            download_urls,
            game_directory,
        }
    }

    pub fn new_imported(name: String, directory: PathBuf, package_type: PackageType) -> Self {
        Self {
            uuid: UNKNOWN_UUID.to_string(),
            name,
            version_type: VersionType::Imported,
            is_new: false,
            package_type,
            download_urls: Vec::new(),
            game_directory: directory,
        }
    }

    pub fn is_imported(&self) -> bool {
        self.version_type == VersionType::Imported
    }

    pub fn is_installed(&self) -> bool {
        self.game_directory.exists()
    }

    pub fn display_name(&self) -> String {
        let type_tag = match self.version_type {
            VersionType::Beta => "(beta)",
            VersionType::Preview => "(preview)",
            _ => "",
        };
        let package_type_tag = match self.package_type {
            PackageType::Gdk => "GDK",
            PackageType::Uwp => "UWP",
        };
        let mut name = format!("{} - {}", self.name, package_type_tag);
        if !type_tag.is_empty() {
            name.push(' ');
            name.push_str(type_tag);
        }
        if self.is_new {
            name.push_str(" (NEW!)");
        }
        name
    }

    pub fn game_package_family(&self) -> &'static str {
        match self.version_type {
            VersionType::Preview => PackageFamilies::MINECRAFT_PREVIEW,
            _ => PackageFamilies::MINECRAFT,
        }
    }
}

fn get_directory_prefix(version_type: VersionType) -> &'static str {
    match version_type {
        VersionType::Preview => "Minecraft-Preview-",
        _ => "Minecraft-",
    }
}

fn build_game_directory(name: &str, version_type: VersionType, package_type: PackageType) -> String {
    let prefix = get_directory_prefix(version_type);
    let type_tag = match package_type {
        PackageType::Gdk => "GDK-",
        PackageType::Uwp => "UWP-",
    };
    format!("{}{}{}", prefix, type_tag, name)
}

fn build_legacy_game_directory(name: &str, version_type: VersionType) -> String {
    format!("{}{}", get_directory_prefix(version_type), name)
}

fn is_gdk_directory(path: &Path) -> bool {
    path.join("MicrosoftGame.Config").is_file()
}

fn resolve_game_directory(name: &str, version_type: VersionType, package_type: PackageType) -> PathBuf {
    let new_dir = PathBuf::from(build_game_directory(name, version_type, package_type));
    if new_dir.exists() {
        return new_dir;
    }

    let legacy_dir = PathBuf::from(build_legacy_game_directory(name, version_type));
    if legacy_dir.exists() {
        let legacy_is_gdk = is_gdk_directory(&legacy_dir);
        if legacy_is_gdk == matches!(package_type, PackageType::Gdk) {
            return legacy_dir;
        }
    }

    new_dir
}

pub fn build_download_filename(name: &str, version_type: VersionType, package_type: PackageType) -> String {
    let prefix = get_directory_prefix(version_type);
    let ext = match package_type {
        PackageType::Uwp => "appx",
        PackageType::Gdk => "msixvc",
    };
    format!("{}{}.{}", prefix, name, ext)
}
