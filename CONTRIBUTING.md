# 贡献指南

感谢你对 Harness-Gate 的关注！我们欢迎各种形式的贡献。

## 开发环境设置

### 前置要求

- Rust 1.70 或更高版本
- Git
- Docker（用于测试服务集成）

### 克隆和构建

```bash
git clone https://github.com/yourusername/Harness-Gate.git
cd Harness-Gate
cargo build --manifest-path tools/harness-gate/Cargo.toml
cargo test --manifest-path tools/harness-gate/Cargo.toml
```

### 运行测试

```bash
# 运行单元测试
cargo test --manifest-path tools/harness-gate/Cargo.toml

# 运行集成测试
cargo test --manifest-path tools/harness-gate/Cargo.toml --test integration

# 格式检查
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check

# Clippy 检查
cargo clippy --manifest-path tools/harness-gate/Cargo.toml -- -D warnings
```

## 提交代码

### 提交信息格式

我们遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <subject>

<body>

<footer>
```

类型：
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式（不影响代码运行）
- `refactor`: 重构
- `test`: 测试相关
- `chore`: 构建过程或辅助工具的变动

示例：
```
feat(doctor): add Node.js version check

Add doctor check to verify Node.js version is 18 or higher for
frontend projects.

Closes #123
```

### Pull Request 流程

1. Fork 本仓库
2. 创建你的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交你的修改 (`git commit -m 'feat: add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

### PR 检查清单

- [ ] 代码通过 `cargo fmt` 格式化
- [ ] 代码通过 `cargo clippy` 检查
- [ ] 所有测试通过
- [ ] 添加了必要的测试
- [ ] 更新了相关文档
- [ ] 更新了 CHANGELOG.md

## 代码规范

### Rust 代码风格

- 遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- 使用 `rustfmt` 默认配置
- 使用 `clippy` 进行 lint 检查
- 避免不必要的 `unsafe` 代码
- 添加适当的文档注释

### 测试要求

- 新功能必须包含单元测试
- 修复 bug 应包含回归测试
- 测试覆盖率应保持在 80% 以上
- 集成测试应该是独立的，不依赖外部状态

## 报告问题

### Bug 报告

请包含以下信息：
- Harness-Gate 版本
- 操作系统和版本
- Rust 版本
- 重现步骤
- 预期行为
- 实际行为
- 相关日志或错误信息

### 功能请求

请描述：
- 功能的使用场景
- 预期的行为
- 可能的实现方案
- 是否愿意贡献代码

## 文档贡献

文档改进同样重要！包括：
- 修正错误
- 改进说明
- 添加示例
- 翻译

## 行为准则

请保持友好、尊重和包容。我们致力于为所有人提供一个友好的社区环境。

## 许可

贡献的代码将在 MIT 许可下发布。
