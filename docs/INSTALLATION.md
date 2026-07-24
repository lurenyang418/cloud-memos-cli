# 安装

## 支持范围

- Rust：`1.95.0`，edition 2024
- macOS：系统 Keychain
- Windows：Credential Manager
- Linux：实现 Freedesktop Secret Service 的桌面密钥环
- 终端：UTF-8，建议至少 `40×10`，宽度达到 100 列时启用双栏

仓库中的 `rust-toolchain.toml` 会让 rustup 自动选择正确工具链。

## 预编译二进制

[GitHub Releases](https://github.com/lurenyang418/cloud-memos-cli/releases) 提供：

- Linux x86_64
- macOS Apple Silicon
- macOS Intel
- Windows x86_64

Linux 与 macOS 使用 `.tar.gz`，Windows 使用 `.zip`；每个归档都有同名 `.sha256`
文件。macOS 可运行 `shasum -a 256 -c <file>.sha256`，Linux 可运行
`sha256sum -c <file>.sha256`。解压后进入同名目录，将 `cloud-memos` 或
`cloud-memos.exe` 放入 `PATH`。

## 从源码安装

```console
# 进入本仓库根目录后
cargo install --path . --locked
cloud-memos --version
```

也可不安装，直接运行：

```console
cargo run --locked -- profile list
cargo run --locked
```

## 平台凭据存储

### macOS

PAT 保存在登录 Keychain，service 为 `cloud-memos-cli`，account 为
`profile:<uuid>`。首次访问时 macOS 可能显示系统授权提示。

### Windows

PAT 保存在 Windows Credential Manager。若企业策略禁止应用使用 Credential Manager，profile
操作会失败并保持配置中不含 PAT。

### Linux

需要正在运行的 Secret Service，例如 GNOME Keyring 或 KDE Wallet。无图形会话、SSH-only
服务器和容器通常没有会话 D-Bus 或已解锁的密钥环，此时 profile 操作会返回“系统凭据存储不可用”。
客户端不会退化为明文文件；可改用成对的 `CLOUD_MEMOS_URL` / `CLOUD_MEMOS_TOKEN` 临时环境变量。

构建使用 vendored D-Bus 支持以减少系统开发包依赖，但运行时仍需要 Secret Service 服务。

## 更新与卸载

更新源码后重新运行：

```console
cargo install --path . --locked --force
```

卸载二进制：

```console
cargo uninstall cloud-memos-cli
```

二进制卸载不会删除 profile。请在卸载前逐个运行 `cloud-memos profile remove <name>`，以同时删除
系统凭据。配置文件位置见 [PROFILES.md](PROFILES.md)。
