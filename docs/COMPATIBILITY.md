# 上游兼容性

## 固定基线

| 项目 | 值 |
| --- | --- |
| 上游 | <https://github.com/lurenyang418/cloud-memos> |
| tag | `v0.3.0` |
| commit | `0864c2327135779e4ac5baf0a407c082a35a1f42` |
| API | `/api/v1` |
| OpenAPI version | `0.3.0` |

`fixtures/cloud-memos-v0.3.0/` 保存脱敏响应及元数据。fixture 不是构建依赖，不包含真实 PAT、用户、
实例域名或 Memo 内容。

## 默认自动测试

mock HTTP 测试覆盖 Bearer header、401/403、scope 降级、cursor 参数、超时、同源与跨源重定向、
409 草稿保留。默认 GitHub Actions 只使用 mock，不持有真实实例凭据。

## 手动实例测试

准备本地或 staging PAT：

```console
CLOUD_MEMOS_TEST_URL=https://staging.example.com \
CLOUD_MEMOS_TEST_TOKEN=... \
cargo test --test live_compat -- --ignored --nocapture
```

默认只验证 session 和列表读取。仅对允许创建测试数据的专用实例启用写入：

```console
CLOUD_MEMOS_TEST_URL=https://staging.example.com \
CLOUD_MEMOS_TEST_TOKEN=... \
CLOUD_MEMOS_TEST_WRITE=1 \
cargo test --test live_compat -- --ignored --nocapture
```

写入测试只操作自己刚创建的探针 Memo，并依次验证创建、乐观锁编辑、移入回收站和永久删除。不要
对生产实例启用。仓库还提供仅 `workflow_dispatch` 的 “Cloud Memos compatibility” workflow；
需在受保护的 `compatibility` environment 中配置 `CLOUD_MEMOS_TEST_URL` 和
`CLOUD_MEMOS_TEST_TOKEN` secrets。

## 升级上游

上游 API 变化时必须显式完成：

1. 选择发布 tag 与精确 commit，不跟踪 `main`。
2. 阅读该提交的 OpenAPI、共享类型、schema、鉴权 middleware 和 Memo routes。
3. 更新 `src/model.rs`、`src/api.rs` 及行为测试。
4. 用脱敏响应替换 fixture，并同步 `metadata.json`。
5. 更新本页和 README 的兼容基线。
6. 运行默认矩阵测试，再对 staging 运行只读及可选写入兼容测试。

不为兼容升级修改上游 Worker、数据库 migration、Cloudflare 资源或 secret。
