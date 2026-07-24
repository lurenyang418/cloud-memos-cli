# 开发

## 环境

安装 rustup 后，仓库会自动使用 `rust-toolchain.toml` 中的 Rust `1.95.0`。上游 Cloud Memos 不是
子模块或构建依赖；只有在升级兼容基线时才按 [COMPATIBILITY.md](COMPATIBILITY.md) 审阅固定提交。

## 质量门禁

提交前运行：

```console
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

测试分层：

- 模块单元测试：配置、URL、脱敏、终端清理和渲染辅助。
- `tests/app_state.rs`：快捷键与状态机。
- `tests/http_client.rs`：mock HTTP、安全重定向、scope 和冲突。
- Ratatui `TestBackend`：宽屏、窄屏、过小、空、错误、编辑、确认、冲突界面。
- `tests/live_compat.rs`：明确忽略、人工提供凭据的 staging 兼容测试。

不要让普通测试访问真实钥匙串或网络。

## 代码边界

- `api.rs`：唯一的 `/api/v1` HTTP 客户端。
- `config.rs`：非敏感 profile 与系统凭据抽象。
- `security.rs`：URL、origin、脱敏和终端清理。
- `app.rs`：无持久化的 TUI 状态机。
- `ui.rs`：Ratatui 渲染，不发请求。
- `editor.rs` / `terminal.rs`：外部编辑器和终端生命周期。

新增写操作必须同时具备：本地只读门禁、`INSUFFICIENT_SCOPE` 降级、适当确认、错误脱敏和 mock
测试。涉及正文的功能不得引入磁盘缓存。

## CI 与依赖

`.github/workflows/ci.yml` 在三种操作系统运行全部门禁并保留 14 天的原生 CLI artifact；
`Cargo.lock` 必须提交以保证 CLI 可重复构建。Dependabot 维护 Cargo 与 GitHub Actions 版本。

`.github/workflows/release.yml` 可人工运行以验证四个平台的发布构建。推送与 `Cargo.toml` 版本一致的
`v*` tag 时，它会生成 Linux x86_64、macOS arm64、macOS x86_64 的 `.tar.gz` 和 Windows
x86_64 的 `.zip`、SHA-256 校验文件及 GitHub Release。每个归档包含同名顶层目录，并在上传前
从归档中执行一次 `cloud-memos --version`。发布 job 是唯一具有 `contents: write` 权限的 job。
