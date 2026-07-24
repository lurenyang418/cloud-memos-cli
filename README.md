# Cloud Memos CLI

`cloud-memos` 是 [Cloud Memos](https://github.com/lurenyang418/cloud-memos) 的独立 Rust
终端客户端。它只调用实例的 `/api/v1` HTTP API，不访问 D1、R2 或 Cloudflare 管理接口，也不把
Cloud Memos 源码作为构建依赖。

当前版本是 `0.1.0`。兼容基线为 Cloud Memos `v0.3.0`、API `/api/v1`、上游提交
`0864c2327135779e4ac5baf0a407c082a35a1f42`。

## 功能

- “我的记录、成员动态、归档、回收站”四个视图，支持 cursor 分页、搜索、标签和可见性筛选。
- 新建、编辑、可见性、置顶、归档、回收站恢复、永久删除、历史版本查看和恢复。
- 宽屏双栏 Markdown 详情与窄屏列表/详情切换；窗口太小时给出明确提示。
- 内置多行编辑器，以及不经过 shell 的 `$VISUAL` / `$EDITOR` 安全外部编辑。
- 破坏性操作确认、未保存内容确认，以及保留本地草稿的 `409 VERSION_CONFLICT` 处理。
- macOS Keychain、Windows Credential Manager 和 Linux Secret Service 保存 PAT。
- HTTPS 强制、loopback HTTP 开发例外、跨源重定向阻断和终端控制序列清理。

附件只显示文件名、MIME 类型和大小，不提供上传、下载或删除。成员动态始终只读。项目不包含附件
操作、ZIP 导入导出、管理员功能、离线同步或脚本化 Memo CRUD 子命令。

## 安装

从 [GitHub Releases](https://github.com/lurenyang418/cloud-memos-cli/releases) 下载与平台匹配的
归档，使用同名 `.sha256` 文件校验后，将 `cloud-memos` 或 `cloud-memos.exe` 放入 `PATH`。

也可以使用 Rust `1.95.0` 从源码安装：

```console
cargo install --path . --locked
cloud-memos --help
```

Linux 桌面还需要可用的 Secret Service 实现，例如 GNOME Keyring 或 KDE Wallet。完整平台说明见
[安装文档](docs/INSTALLATION.md)。

## 配置 profile

在隐藏提示中输入 PAT：

```console
cloud-memos profile add work https://memos.example.com
cloud-memos profile list
cloud-memos profile use work
cloud-memos
```

只读 PAT 应明确记录预期模式：

```console
cloud-memos profile add reading https://memos.example.com --mode read-only
```

自动化环境可通过 `--token-stdin` 输入 PAT。程序会先调用 `/api/v1/session` 验证实例、PAT 和用户
状态，然后才保存 profile。不要把 PAT 放在命令行参数、shell 历史、配置文件或 issue 中。

也可使用不落盘的临时凭据；两个变量必须成对设置：

```console
CLOUD_MEMOS_URL=https://memos.example.com \
CLOUD_MEMOS_TOKEN=... \
cloud-memos
```

环境变量优先于 `--profile` 和当前 profile。profile 的格式、系统路径、删除语义及故障排查见
[profile 文档](docs/PROFILES.md)。

## 快捷键

| 按键 | 行为 |
| --- | --- |
| `←` / `→`、`Tab` | 切换四个视图 |
| `↑` / `↓`、`j` / `k` | 导航 |
| `PageDown` / `PageUp` | 下一页 / 上一页 |
| `Enter` | 窄屏切换列表与详情 |
| `/`、`t`、`f` | 搜索、标签筛选、可见性筛选 |
| `n`、`e` | 新建、编辑 |
| `Ctrl+S`、`Esc` | 保存编辑、取消 |
| `Ctrl+E` | 使用 `$VISUAL` / `$EDITOR` |
| `v`、`p`、`a` | 切换可见性、置顶、归档或取消归档 |
| `d`、`u` | 删除、从回收站恢复 |
| `h` | 查看并恢复历史版本 |
| `r`、`?`、`q` | 刷新/重试、帮助、退出 |

若写请求返回 `INSUFFICIENT_SCOPE`，当前运行会自动降级为只读，并立即禁用后续写操作。profile
中的预期模式不会自动改写。

并发编辑发生冲突时，界面同时展示本地草稿和最新服务端正文，可选择继续合并编辑、确认使用最新
版本号覆盖重试，或取消并把草稿继续保留在内存中。退出前若仍有暂存草稿，程序会再次确认。

## 安全与隐私

配置文件只保存 profile UUID、名称、URL、预期模式和当前选择；PAT 单独存入系统凭据存储。程序不
持久化 Memo 正文、列表缓存或草稿，也不会在错误和调试格式中输出 PAT。安全模型和漏洞报告方式见
[安全文档](docs/SECURITY.md)。

仅接受：

- `https://` 实例；
- 开发用途的 `http://localhost`、`http://127.0.0.0/8` 或 `http://[::1]`。

实例 URL 必须是站点根路径。客户端最多跟随五次同源安全重定向，跨源、协议降级和其他明文 HTTP
重定向会在发送凭据前停止。

## 开发与验证

```console
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked
```

CI 在 Linux、macOS、Windows 上执行同一套格式、Clippy、测试和 release build。默认 CI 不持有
真实实例凭据。兼容 fixture、手动 staging 测试和升级步骤见
[兼容性文档](docs/COMPATIBILITY.md)，贡献流程见 [开发文档](docs/DEVELOPMENT.md)。

## 许可证

本项目采用 [MIT License](LICENSE)。
