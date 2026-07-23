# Profile 与凭据

## 命令

```console
cloud-memos profile add <name> <instance-url> [--mode read-write|read-only]
cloud-memos profile add <name> <instance-url> --token-stdin
cloud-memos profile list
cloud-memos profile use <name>
cloud-memos profile remove <name>
cloud-memos --profile <name>
```

`add` 默认 `read-write`，并在保存前执行带 Bearer PAT 的 `GET /api/v1/session`。实例必须返回
`ACTIVE` viewer。Cloud Memos `v0.3.0` 的 session 响应不包含 PAT scope，因此“预期模式”由用户
声明；它只控制本地写操作是否初始启用。

服务器返回 `403 INSUFFICIENT_SCOPE` 时，本次运行立即降级为只读。磁盘上的 profile 不自动改变，
因为同一个 PAT 的 scope 可能由实例管理员在其他位置调整。

## 配置位置与内容

默认位置遵循平台配置目录：

- macOS：`~/Library/Application Support/cloud-memos-cli/config.toml`
- Linux：`$XDG_CONFIG_HOME/cloud-memos-cli/config.toml`，未设置时通常是
  `~/.config/cloud-memos-cli/config.toml`
- Windows：用户 Roaming AppData 下的 `cloud-memos-cli/config.toml`

测试和受控开发可用 `CLOUD_MEMOS_CONFIG` 指定替代路径。配置以原子方式写入；Unix 权限设为
`0600`。格式只包含：

```toml
current_profile = "00000000-0000-0000-0000-000000000001"

[[profiles]]
id = "00000000-0000-0000-0000-000000000001"
name = "work"
url = "https://memos.example.com/"
expected_mode = "read-write"
```

PAT 不在此文件中。系统凭据的 service 是 `cloud-memos-cli`，用户名是 `profile:<uuid>`。

## 环境变量覆盖

`CLOUD_MEMOS_URL` 与 `CLOUD_MEMOS_TOKEN` 必须同时出现，且优先于 `--profile` 和当前选择。环境变量
模式初始视为读写，仍会在服务器拒绝写 scope 时降级。环境覆盖不会创建或修改 profile。

URL 只能是 HTTPS 根 URL，loopback 开发实例可用 HTTP。用户名、密码、query、fragment 和非根
路径均被拒绝。

## 删除与恢复

`profile remove` 会删除系统凭据并更新当前选择；如果删除的是当前 profile，则第一个剩余 profile
成为当前选择。删除操作不会调用 Cloud Memos 服务端，也不会撤销 PAT；如需彻底撤销，请同时在
Cloud Memos Web 设置中撤销 API token。

若系统凭据被外部工具删除但 profile 仍存在，启动会报告凭据存储错误。重新添加一个新 profile，
确认工作后再删除旧条目。
