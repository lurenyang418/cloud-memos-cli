use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;
use uuid::Uuid;

pub const KEYRING_SERVICE: &str = "cloud-memos-cli";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileMode {
    #[default]
    ReadWrite,
    ReadOnly,
}

impl ProfileMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReadWrite => "读写",
            Self::ReadOnly => "只读",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub expected_mode: ProfileMode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub current_profile: Option<Uuid>,
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("无法确定用户配置目录")]
    ConfigDirectoryUnavailable,
    #[error("无法读取配置：{0}")]
    Read(#[source] std::io::Error),
    #[error("配置格式无效：{0}")]
    Parse(#[source] toml::de::Error),
    #[error("无法序列化配置：{0}")]
    Serialize(#[source] toml::ser::Error),
    #[error("无法安全保存配置：{0}")]
    Save(#[source] std::io::Error),
    #[error("profile 名称不能为空、不能超过 64 个字符，也不能包含控制字符")]
    InvalidProfileName,
    #[error("profile “{0}” 已存在")]
    DuplicateProfile(String),
    #[error("找不到 profile “{0}”")]
    ProfileNotFound(String),
    #[error("系统凭据存储不可用：{0}")]
    SecretStore(String),
}

impl Config {
    pub fn path() -> Result<PathBuf, ConfigError> {
        if let Some(path) = env::var_os("CLOUD_MEMOS_CONFIG") {
            return Ok(PathBuf::from(path));
        }
        dirs::config_dir()
            .map(|directory| directory.join("cloud-memos-cli").join("config.toml"))
            .ok_or(ConfigError::ConfigDirectoryUnavailable)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).map_err(ConfigError::Parse),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(ConfigError::Read(error)),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let parent = path.parent().ok_or_else(|| {
            ConfigError::Save(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "配置路径没有父目录",
            ))
        })?;
        fs::create_dir_all(parent).map_err(ConfigError::Save)?;
        let encoded = toml::to_string_pretty(self).map_err(ConfigError::Serialize)?;
        let mut temporary = NamedTempFile::new_in(parent).map_err(ConfigError::Save)?;
        temporary
            .write_all(encoded.as_bytes())
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(ConfigError::Save)?;
        temporary
            .persist(path)
            .map_err(|error| ConfigError::Save(error.error))?;
        set_private_permissions(path)?;
        Ok(())
    }

    pub fn profile(&self, selector: Option<&str>) -> Result<&Profile, ConfigError> {
        let profile = match selector {
            Some(value) => self.profiles.iter().find(|profile| {
                profile.name == value || profile.id.to_string().eq_ignore_ascii_case(value)
            }),
            None => self
                .current_profile
                .and_then(|id| self.profiles.iter().find(|profile| profile.id == id)),
        };
        profile.ok_or_else(|| {
            ConfigError::ProfileNotFound(selector.unwrap_or("当前 profile").to_owned())
        })
    }

    pub fn add(&mut self, profile: Profile) -> Result<(), ConfigError> {
        validate_profile_name(&profile.name)?;
        if self.profiles.iter().any(|item| item.name == profile.name) {
            return Err(ConfigError::DuplicateProfile(profile.name));
        }
        if self.current_profile.is_none() {
            self.current_profile = Some(profile.id);
        }
        self.profiles.push(profile);
        Ok(())
    }

    pub fn use_profile(&mut self, selector: &str) -> Result<(), ConfigError> {
        let id = self.profile(Some(selector))?.id;
        self.current_profile = Some(id);
        Ok(())
    }

    pub fn remove(&mut self, selector: &str) -> Result<Profile, ConfigError> {
        let id = self.profile(Some(selector))?.id;
        let index = self
            .profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| ConfigError::ProfileNotFound(selector.to_owned()))?;
        let removed = self.profiles.remove(index);
        if self.current_profile == Some(id) {
            self.current_profile = self.profiles.first().map(|profile| profile.id);
        }
        Ok(removed)
    }
}

pub fn validate_profile_name(name: &str) -> Result<(), ConfigError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 64 || trimmed.chars().any(char::is_control) {
        return Err(ConfigError::InvalidProfileName);
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), ConfigError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(ConfigError::Save)
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), ConfigError> {
    Ok(())
}

pub trait SecretStore {
    fn set(&self, profile_id: Uuid, token: &str) -> Result<(), ConfigError>;
    fn get(&self, profile_id: Uuid) -> Result<String, ConfigError>;
    fn delete(&self, profile_id: Uuid) -> Result<(), ConfigError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KeyringSecretStore;

impl KeyringSecretStore {
    fn entry(profile_id: Uuid) -> Result<keyring::Entry, ConfigError> {
        keyring::Entry::new(KEYRING_SERVICE, &format!("profile:{profile_id}"))
            .map_err(|error| ConfigError::SecretStore(error.to_string()))
    }
}

impl SecretStore for KeyringSecretStore {
    fn set(&self, profile_id: Uuid, token: &str) -> Result<(), ConfigError> {
        Self::entry(profile_id)?
            .set_password(token)
            .map_err(|error| ConfigError::SecretStore(error.to_string()))
    }

    fn get(&self, profile_id: Uuid) -> Result<String, ConfigError> {
        Self::entry(profile_id)?
            .get_password()
            .map_err(|error| ConfigError::SecretStore(error.to_string()))
    }

    fn delete(&self, profile_id: Uuid) -> Result<(), ConfigError> {
        match Self::entry(profile_id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(ConfigError::SecretStore(error.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn profile(name: &str) -> Profile {
        Profile {
            id: Uuid::new_v4(),
            name: name.to_owned(),
            url: "https://memos.example.com/".to_owned(),
            expected_mode: ProfileMode::ReadWrite,
        }
    }

    #[test]
    fn config_round_trip_contains_no_token() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("nested/config.toml");
        let mut config = Config::default();
        config.add(profile("工作")).expect("add profile");
        config.save(&path).expect("save config");

        let contents = fs::read_to_string(&path).expect("read config");
        assert!(!contents.to_ascii_lowercase().contains("token"));
        assert_eq!(Config::load(&path).expect("load config"), config);

        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn add_use_and_remove_profiles() {
        let mut config = Config::default();
        let first = profile("first");
        let second = profile("second");
        config.add(first.clone()).expect("add first");
        config.add(second.clone()).expect("add second");
        assert_eq!(config.current_profile, Some(first.id));
        assert!(matches!(
            config.add(profile("first")),
            Err(ConfigError::DuplicateProfile(_))
        ));
        config.use_profile("second").expect("switch profile");
        assert_eq!(config.current_profile, Some(second.id));
        config.remove("second").expect("remove profile");
        assert_eq!(config.current_profile, Some(first.id));
    }

    #[test]
    fn rejects_unknown_config_fields() {
        let result = toml::from_str::<Config>("unexpected = true");
        assert!(result.is_err());
    }
}
