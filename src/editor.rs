use std::{env, fs, io::Write, process::Command};

use anyhow::{Context, Result, anyhow, bail};
use tempfile::Builder;

const MAX_MEMO_BYTES: u64 = 100_000;

/// 使用 `$VISUAL` 或 `$EDITOR` 编辑正文，不经过 shell。
///
/// 正文仅写入由操作系统以私有权限创建的临时文件，函数返回时文件即删除。
pub fn edit_externally(content: &str) -> Result<String> {
    let command_line = env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("EDITOR")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| anyhow!("未设置 VISUAL 或 EDITOR"))?;
    let parts = shlex::split(&command_line).ok_or_else(|| anyhow!("编辑器命令引号不匹配"))?;
    let (program, arguments) = parts
        .split_first()
        .ok_or_else(|| anyhow!("编辑器命令为空"))?;

    let mut file = Builder::new()
        .prefix("cloud-memos-")
        .suffix(".md")
        .tempfile()
        .context("无法创建私有临时编辑文件")?;
    file.write_all(content.as_bytes())
        .and_then(|()| file.flush())
        .context("无法写入临时编辑文件")?;
    let path = file.into_temp_path();

    let status = Command::new(program)
        .args(arguments)
        .arg(&path)
        .status()
        .with_context(|| format!("无法启动外部编辑器 {program}"))?;
    if !status.success() {
        bail!("外部编辑器退出失败（{status}）");
    }

    let size = fs::metadata(&path).context("无法读取临时文件信息")?.len();
    if size > MAX_MEMO_BYTES {
        bail!("外部编辑器产生的正文超过 100,000 字节");
    }
    fs::read_to_string(&path).context("外部编辑器输出不是有效 UTF-8")
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn source_uses_command_directly_without_shell() {
        let source = fs::read_to_string(file!()).expect("read source");
        assert!(source.contains("Command::new(program)"));
        assert!(!source.contains("Command::new(\"sh\")"));
        assert!(!source.contains("Command::new(\"cmd\")"));
    }
}
