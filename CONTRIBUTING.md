# 贡献指南

感谢改进 Cloud Memos CLI。提交前请先阅读 [开发文档](docs/DEVELOPMENT.md) 和
[安全模型](docs/SECURITY.md)。

请保持变更范围清晰，为行为变化添加单元、mock HTTP 或 `TestBackend` 测试，并同步用户文档。
不要提交 PAT、实例 secret、真实 Memo、用户邮箱或未经脱敏的响应。兼容性变更必须固定上游 tag
和 commit，不自动跟踪上游 `main`。

Pull request 应说明：

- 用户可见行为；
- 安全与数据持久化影响；
- 已运行的验证命令；
- 是否需要人工 staging 兼容测试。

所有贡献按仓库的 MIT License 提供。
