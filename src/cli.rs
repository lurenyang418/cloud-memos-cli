use std::{
    env,
    io::{self, Read},
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use crate::{
    api::ApiClient,
    config::{Config, ConfigError, Profile, ProfileMode, SecretStore, validate_profile_name},
    security::{sanitize_terminal, validate_instance_url},
};

#[derive(Debug, Parser)]
#[command(
    name = "cloud-memos",
    version,
    about = "Cloud Memos 的安全跨平台终端客户端",
    long_about = None
)]
pub struct Cli {
    /// 启动指定 profile；环境变量凭据存在时由环境变量覆盖
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 管理实例 profile 和系统凭据
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// 添加并验证一个 profile
    Add {
        /// 本地 profile 名称
        name: String,
        /// Cloud Memos 实例根 URL
        url: String,
        /// 预期权限模式（服务器拒绝写入时仍会自动降级）
        #[arg(long, value_enum, default_value_t = ModeArg::ReadWrite)]
        mode: ModeArg,
        /// 从标准输入读取 PAT；否则使用隐藏输入
        #[arg(long)]
        token_stdin: bool,
    },
    /// 列出 profile（不会读取或显示 PAT）
    List,
    /// 选择默认 profile
    Use { name: String },
    /// 删除 profile 及对应系统凭据
    Remove { name: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ModeArg {
    ReadWrite,
    ReadOnly,
}

impl From<ModeArg> for ProfileMode {
    fn from(value: ModeArg) -> Self {
        match value {
            ModeArg::ReadWrite => Self::ReadWrite,
            ModeArg::ReadOnly => Self::ReadOnly,
        }
    }
}

pub struct RuntimeCredentials {
    pub url: url::Url,
    pub token: String,
    pub mode: ProfileMode,
    pub label: String,
}

impl std::fmt::Debug for RuntimeCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeCredentials")
            .field("url", &self.url)
            .field("token", &"[REDACTED]")
            .field("mode", &self.mode)
            .field("label", &self.label)
            .finish()
    }
}

pub async fn handle_profile_command(
    command: ProfileCommand,
    config_path: &Path,
    store: &impl SecretStore,
) -> Result<()> {
    let mut config = Config::load(config_path)?;
    match command {
        ProfileCommand::Add {
            name,
            url,
            mode,
            token_stdin,
        } => {
            validate_profile_name(&name)?;
            if config.profiles.iter().any(|profile| profile.name == name) {
                return Err(ConfigError::DuplicateProfile(name).into());
            }
            let url = validate_instance_url(&url)?;
            let token = read_token(token_stdin)?;
            let client = ApiClient::new(url.clone(), token.clone())?;
            let session = client
                .session()
                .await
                .context("无法使用该 PAT 验证 /api/v1/session")?;
            let viewer = session
                .viewer
                .ok_or_else(|| anyhow!("令牌有效，但 /api/v1/session 没有返回已登录用户"))?;
            if viewer.status != "ACTIVE" {
                bail!("用户状态不是 ACTIVE，不能添加该 profile");
            }

            let profile = Profile {
                id: Uuid::new_v4(),
                name: name.trim().to_owned(),
                url: url.to_string(),
                expected_mode: mode.into(),
            };
            store.set(profile.id, &token)?;
            if let Err(error) = config
                .add(profile.clone())
                .and_then(|()| config.save(config_path))
            {
                let _ = store.delete(profile.id);
                return Err(error.into());
            }
            println!(
                "已添加 profile “{}”（{}，{}，用户 {}）",
                profile.name,
                profile.url,
                profile.expected_mode.label(),
                sanitize_terminal(&viewer.name)
            );
        }
        ProfileCommand::List => {
            if config.profiles.is_empty() {
                println!("尚未配置 profile。请运行 cloud-memos profile add <name> <url>。");
            } else {
                for profile in &config.profiles {
                    let marker = if config.current_profile == Some(profile.id) {
                        "*"
                    } else {
                        " "
                    };
                    println!(
                        "{marker} {}  {}  {}  {}",
                        sanitize_terminal(&profile.name),
                        sanitize_terminal(&profile.url),
                        profile.expected_mode.label(),
                        profile.id
                    );
                }
            }
        }
        ProfileCommand::Use { name } => {
            config.use_profile(&name)?;
            config.save(config_path)?;
            println!("当前 profile 已切换为“{}”。", sanitize_terminal(&name));
        }
        ProfileCommand::Remove { name } => {
            let profile = config.profile(Some(&name))?.clone();
            store.delete(profile.id)?;
            config.remove(&name)?;
            config.save(config_path)?;
            println!(
                "已删除 profile “{}”及其系统凭据。",
                sanitize_terminal(&profile.name)
            );
        }
    }
    Ok(())
}

fn read_token(from_stdin: bool) -> Result<String> {
    let token = if from_stdin {
        let mut value = String::new();
        io::stdin()
            .read_to_string(&mut value)
            .context("无法从标准输入读取 PAT")?;
        value.trim_end_matches(['\r', '\n']).to_owned()
    } else {
        rpassword::prompt_password("Cloud Memos PAT（输入不会显示）: ")
            .context("无法读取隐藏 PAT")?
    };
    if token.trim().is_empty() {
        bail!("PAT 不能为空");
    }
    if token.contains(['\r', '\n']) {
        bail!("PAT 不能包含换行符");
    }
    Ok(token)
}

pub fn resolve_runtime_credentials(
    selector: Option<&str>,
    config_path: &Path,
    store: &impl SecretStore,
) -> Result<RuntimeCredentials> {
    let env_url = read_optional_env("CLOUD_MEMOS_URL")?;
    let env_token = read_optional_env("CLOUD_MEMOS_TOKEN")?;
    resolve_runtime_credentials_with_env(selector, config_path, store, env_url, env_token)
}

fn resolve_runtime_credentials_with_env(
    selector: Option<&str>,
    config_path: &Path,
    store: &impl SecretStore,
    env_url: Option<String>,
    env_token: Option<String>,
) -> Result<RuntimeCredentials> {
    match (env_url, env_token) {
        (Some(url), Some(token)) => {
            if token.trim().is_empty() {
                bail!("CLOUD_MEMOS_TOKEN 不能为空");
            }
            if token.contains(['\r', '\n']) {
                bail!("CLOUD_MEMOS_TOKEN 不能包含换行符");
            }
            return Ok(RuntimeCredentials {
                url: validate_instance_url(&url)?,
                token,
                mode: ProfileMode::ReadWrite,
                label: "环境变量".to_owned(),
            });
        }
        (Some(_), None) | (None, Some(_)) => {
            bail!("CLOUD_MEMOS_URL 与 CLOUD_MEMOS_TOKEN 必须成对提供");
        }
        (None, None) => {}
    }

    let config = Config::load(config_path)?;
    let profile = config.profile(selector)?;
    Ok(RuntimeCredentials {
        url: validate_instance_url(&profile.url)?,
        token: store.get(profile.id)?,
        mode: profile.expected_mode,
        label: profile.name.clone(),
    })
}

fn read_optional_env(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => bail!("{name} 不是有效的 UTF-8"),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use tempfile::tempdir;

    use super::*;

    #[derive(Default)]
    struct MemoryStore(Mutex<HashMap<Uuid, String>>);

    impl SecretStore for MemoryStore {
        fn set(&self, profile_id: Uuid, token: &str) -> Result<(), ConfigError> {
            self.0
                .lock()
                .expect("memory store lock")
                .insert(profile_id, token.to_owned());
            Ok(())
        }

        fn get(&self, profile_id: Uuid) -> Result<String, ConfigError> {
            self.0
                .lock()
                .expect("memory store lock")
                .get(&profile_id)
                .cloned()
                .ok_or_else(|| ConfigError::SecretStore("missing".to_owned()))
        }

        fn delete(&self, profile_id: Uuid) -> Result<(), ConfigError> {
            self.0
                .lock()
                .expect("memory store lock")
                .remove(&profile_id);
            Ok(())
        }
    }

    #[test]
    fn resolves_saved_profile_without_exposing_token_in_debug() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("config.toml");
        let store = MemoryStore::default();
        let profile = Profile {
            id: Uuid::new_v4(),
            name: "work".to_owned(),
            url: "https://memos.example.com/".to_owned(),
            expected_mode: ProfileMode::ReadOnly,
        };
        let mut config = Config::default();
        config.add(profile.clone()).expect("add profile");
        config.save(&path).expect("save profile");
        store
            .set(profile.id, "cm_pat_never_print")
            .expect("save token");

        let runtime = resolve_runtime_credentials_with_env(None, &path, &store, None, None)
            .expect("resolve credentials");
        assert_eq!(runtime.mode, ProfileMode::ReadOnly);
        assert!(!format!("{runtime:?}").contains("cm_pat_never_print"));
    }

    #[test]
    fn requires_paired_environment_variables() {
        let directory = tempdir().expect("temporary directory");
        let result = resolve_runtime_credentials_with_env(
            None,
            &directory.path().join("missing.toml"),
            &MemoryStore::default(),
            Some("https://memos.example.com".to_owned()),
            None,
        );
        assert!(result.is_err());
    }
}
