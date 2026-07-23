# 安全模型

## 信任边界

客户端信任用户选择的本机系统凭据存储和明确配置的 Cloud Memos origin。它不信任 Memo 正文、
附件元数据、API 错误消息、重定向目标或代理响应。

## PAT 处理

- `profile add` 通过隐藏输入或标准输入接收 PAT；PAT 不是命令行参数。
- 配置只保存非敏感 profile 元数据。
- PAT 保存到 macOS Keychain、Windows Credential Manager 或 Linux Secret Service。
- Authorization header 标记为敏感；错误消息、服务端 error body 和 `Debug` 输出会按当前 PAT
  做替换脱敏。
- 程序没有请求日志，也不会打印 HTTP header。
- 环境变量覆盖不落盘，但环境变量本身可能被进程管理器、崩溃收集器或同权限调试工具读取。

不要在 issue、CI 日志、shell 历史或截图中粘贴 PAT。怀疑泄露时，应立即在 Cloud Memos Web 中
撤销 token，而不只是删除本地 profile。

## 传输约束

生产实例必须使用 HTTPS。HTTP 仅允许 `localhost`、IPv4 loopback 或 IPv6 `::1`。URL 必须指向
origin 根路径，不允许内嵌用户名/密码、query 或 fragment。

重定向策略最多允许五次同源跳转；任何 origin 变化、非 HTTPS 远程目标或协议变化都会停止。
因此 Bearer PAT 不会因客户端自动跟随而发送到另一个 origin。

## 终端与编辑器

远端正文、作者、附件、状态和错误在显示前移除 ANSI/ECMA-48 转义及终端控制字符。Markdown 由
pulldown-cmark 解析后转换为 Ratatui 样式，不把 HTML 当作终端指令执行。

外部编辑器通过 `Command` 直接启动，不经过 shell。正文写入操作系统创建的私有临时文件，编辑器
退出后读取并删除；输出限制为 100,000 字节。`VISUAL` / `EDITOR` 本身仍属于用户信任的本地命令。

## 本地数据

程序只在内存中保存当前页正文、编辑草稿和冲突副本。退出后不持久化正文、列表缓存或草稿。系统
崩溃、断电或强制结束进程会丢失未保存草稿，这是“不做离线持久化”的预期取舍。

## 报告漏洞

请使用仓库的私密安全报告渠道；不要在公开 issue 中包含 PAT、实例 URL 中的私密租户信息、Memo
正文或可复现账号。报告应包含版本、平台、最小复现步骤和已脱敏的错误。
