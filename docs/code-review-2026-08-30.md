# Harness-Gate 代码评审报告（可用性 / 性能 / 扩展性）

**评审日期**: 2026-08-30
**评审版本**: v0.3.2（主分支 `01f0bba`）
**评审维度**: 可用性、性能、扩展性
**依据**: 项目文档（README.zh-CN.md、docs/configuration.md）、`.harness-gate/flow.toml` 与 `.harness-gate/audit.toml` 门禁配置、`tools/harness-gate/src` 源码
**前序报告**: 2026-08-28 代码评审与重构方案（均为本地评审材料，未纳入发布树）

---

## 项目概览

| 项目 | 数据 |
| ---- | ---- |
| 代码规模 | 约 12,666 行 Rust |
| 源文件数 | 79 个 `.rs` |
| 模块数 | 14 个顶层模块 |
| 二进制体积 | 当前 `rustc 1.97.1` 本地重建：`release` 10,982,024 bytes，`release-small` 7,956,664 bytes；CI 基线按目标平台和 toolchain 分系列记录 |
| 关键依赖 | clap、serde、toml、schemars、rayon、globset、regex、tera、ureq |

自 v0.2.0 评审以来已落地的能力：`schemars` JSON Schema 导出、`tera` 报告模板与 JUnit 输出、`ureq` Webhook 通知、`service/runtime.rs` 容器运行时抽象（Docker / Podman）、`verify/{plan,scheduler}.rs` 执行计划与并行调度、`ExecutionConfig` 并发配置，以及 Phase 1 覆盖率/关键路径/性能证据。

当前工作树中的 `.harness-gate/flow.toml` 只声明了两个 `git diff --check` 步骤和两项 doctor 检查，`[scope] unmatched = "all"`；该文件尚未纳入 `origin/main`，因此不能当作仓库发布配置的结论。

---

## 一、可用性评审 ★★★★★ (5/5)

### 优点

#### 1. 配置即代码，扩展不需要改 Rust

新增步骤只需追加 TOML 片段：

```toml
[[steps]]
id = "security.trivy"
label = "Container security scan"
component = "backend"
profiles = ["ci"]
program = "trivy"
args = ["fs", "--severity", "HIGH,CRITICAL", "."]
cwd = "{root}"
log = "trivy.log"
timeout_secs = 300
```

`program + args[]` 不经过 Shell 解析，既避免注入，也让配置在不同平台上行为一致。

#### 2. 严格且可机读的配置校验

`config/model.rs:8` 起的模型全量标注 `#[serde(deny_unknown_fields)]`，配合 `JsonSchema` 派生：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlowConfig {
    pub version: u32,
    pub project: ProjectConfig,
    pub paths: PathsConfig,
    // ...
}
```

拼错字段立即失败，`harness-gate schema export` 产出的 schema 可直接用于编辑器补全和 CI 校验。`config check --format json` 输出稳定字段路径，适合编辑器与流水线消费。

#### 3. 错误码体系

`E1000` 通用、`E11xx` 审计、`E12xx` 秘密扫描、`E13xx` scope、`E14xx` 验证。`verify/mod.rs:56` 的 `CodedError` 实现把子模块错误码透传，不掩盖根因：

```rust
Self::Scope(error) => error.code(),
Self::Secrets(error) => error.code(),
Self::Audit(error) => error.code(),
```

CI 日志可按错误码聚合告警，用户可按码查文档。

#### 4. 四种 Scope 模式覆盖真实工作流

`WorkingTree`（日常开发）、`Staged`（pre-commit hook）、`Base(REF)`（PR 验证）、`All`（发布门禁）。默认无变更时不选择 component，需要全量验证必须显式 `--all`，避免误以为跑过了全量。

#### 5. 安全默认值

固定门禁不可由项目配置关闭：外部步骤前必须过 secret scan 和 audit；配置、报告、路径别名不得逃出项目根；禁止 `sh -c`；容器必须声明健康检查并在退出时清理；多个 service 不得注入同名变量。秘密扫描报告只记录文件名，不把凭据写进终端或 JSON。

#### 6. 迁移与初始化的原子写入

`init` 和 `config migrate` 都使用同目录临时文件加原子重命名，不留半写文件，默认拒绝覆盖，`--force` 才允许替换。迁移不删除源文件，由人工决定何时清理。

### 待改进

#### 1. CLI 使用文档仍可加强（中优先级）

当前 crate 是二进制目标（没有公开 `lib` target），docs.rs 主要展示 README，Rust 模块内部的 `pub fn` 并不是稳定的外部 API。现有 README 已能完成安装和基本使用，但配置概念与错误修复路径仍可更快被新用户理解。

**建议**：优先补充按场景组织的 CLI 示例、最小配置和错误码修复提示；只有在未来拆出稳定 library target 后，再投入公共 Rust API 文档。

#### 2. 概念负载偏高

用户需同时理解 component、profile、scope rule、step、gate、parser、service、path alias 八个概念及其交叉关系。README 已有阅读导航，但仍需通读较长篇幅才能配出第一个可用文件。

**建议**：增加 `harness-gate init --interactive` 向导，或在 `config check` 失败时输出最小可用配置片段作为修复提示。

#### 3. 本仓库自身门禁未纳入版本控制

工作树里的 `.harness-gate/flow.toml` 只做 whitespace 检查，但它当前是未跟踪文件，干净仓库并没有这份自门禁配置。

**建议**：为本仓库补 `workflow` component，把 fmt、clippy、nextest 配成 `hook` 与 `full` 两档。

#### 4. 依赖阻塞步骤的报告可见性（P1）

验证报告此前只列出已执行步骤；依赖失败后被跳过的后代节点不会出现在
`steps` 中，用户只能看到总体失败，难以区分“未执行”和“执行失败”。建议在
机器报告中增加可选的 `skipped_steps`（节点 ID、标签、阻塞原因），并在 Markdown
和 JUnit 输出中保留对应状态；成功报告应继续保持现有字段形状。

---

## 二、性能评审 ★★★★☆ (4/5)

### 优点

#### 1. 编译配置经过实测调优

`Cargo.toml:44` 起的 profile 注释记录了实测数据，而非照搬默认值：

- `[profile.dev] debug = 1`：全量构建 19.5s → 17.9s，`target/debug` 511 MB → 425 MB。
- `[profile.release] codegen-units = 4`：历史测量中 79.31s → 68.96s（-13.0%）；`cu=16` 因 fat LTO 成为串行瓶颈反而回退到 78.1s。历史体积数字不再作为当前基线，当前体积以锁定 toolchain 的实际构建为准。
- `[profile.release-small] codegen-units = 1`：分发构建用编译时间换体积。

这种把测量结果写进注释的做法，让后续调参有基线可比。

#### 2. 审计扫描已并行

`rayon` 已是正式依赖，审计扫描按文件并行，整文件正则匹配后再映射回起始行号。审计是全量扫描，也是最容易成为瓶颈的环节，这里的并行化选得准。

#### 3. 服务复用与失败缓存

`ServiceManager` 以 `resources` 注册表和 `ResourceState` 状态机管理服务：同一 service 被多个步骤引用时可复用，启动失败后后续步骤快速失败，不重复付出容器启动与健康检查的代价。

#### 4. 进程管理有硬超时

步骤允许 1–3600 秒，service 启动 1–300 秒，doctor 单项默认 15 秒。超时终止整个子进程组，避免僵留进程拖住流水线。

### 剩余瓶颈

#### 已处理：Scope 路径分类的重复编译

前序报告指出 `classify_paths` 在 `paths × rules` 双层循环内重复构建 `GlobSet`。当前已由 `Project` 在发现阶段构建 `CompiledScopeRules`，后续 scope 检测复用缓存；独立的 `FlowConfig::classify_paths` 仍保留一次性 helper 语义。

该改动不改变 component/unmatched 结果，收益主要体现在同一进程内多次 `scope`、`verify`、`hook` 调用以及规则较多的配置。

#### 2. 容器启动开销固定存在（P2）

单个 Docker service 的创建加健康检查轮询通常 1.5–6 秒。当前已有的缓解手段是服务复用和 `external_env` 走外部实例。

**建议**：并行调度落地后，把无依赖的 service 启动与门禁扫描重叠执行，用 secret scan 与 audit 的时间掩盖容器预热。

#### 3. 性能证据需要跨平台分系列（P1）

Phase 1 已固化 Linux 的运行期基线，CI 也记录目标、toolchain、并发度和二进制大小。由于 macOS/Windows 的基线此前没有同等覆盖，不能把 Linux 数字直接外推到其他平台；应保留各平台独立 series，并在回归时只做同平台比较。

---

## 三、扩展性评审 ★★★★☆ (4/5)

### 优点

#### 1. 模块边界与依赖方向清晰

```text
config → scope → verify → { process, service } → { report, notify }
```

14 个顶层模块各有独立 `mod.rs` 与 `tests.rs`，`audit/scanner/` 和 `config/validation/` 进一步按职责拆成子目录。依赖单向，未见环状引用。

#### 2. 枚举驱动的扩展点

`ServiceConfig`、`ParserConfig`、`DoctorCheckKind` 均为 `#[serde(tag = "kind")]` 标注的枚举。新增变体时编译器会强制所有 `match` 补齐分支，不会静默漏处理。`DoctorCheckKind` 已有 9 个变体，覆盖 command、path、glob、env、env-or-file、git-config、git-remotes、version、service。

#### 3. 容器运行时抽象已落地

`service/runtime.rs` 存在，配置层支持 `runtime = "docker" | "podman"`，省略时默认 Docker。这是前序报告的中期建议，现已交付。容器 provider 使用 `--pull=never`，验证过程不隐式访问网络。

#### 4. 报告与通知已从核心解耦

`report_templates`（Tera 模板 + JUnit XML）与 `notifications.webhooks` 均为独立配置块。Webhook 在报告落盘后发送，非 2xx 返回 `E1404` 但不删除已生成报告——失败隔离做得对。

#### 5. 配置版本迁移有保障

v1 → v2 迁移器保留源文件，目标已存在需 `--force`，并在缺失时补齐 Secret Scan v2 默认规则。用户升级成本可控。

### 限制

#### 1. 强制 Gate 链需要明确扩展边界（P1）

内置 secret scan 和 architecture audit 已经是 `PlanNodeKind::Builtin`，并由
`verify/plan.rs` 注入计划；当前仍强制 secret scan → audit → 外部步骤的安全链。
这不是待完成的 DAG 基础设施，而是不能被项目配置绕过的安全不变量。

后续若增加自定义 Gate，应只允许它们作为显式计划节点接入，并保留这条前置链；
同时补充自定义 Gate 的错误码、报告和失败传播契约，避免为了“可排序”而削弱门禁。

#### 2. 无插件系统（P1）

所有能力编译进单一二进制。企业自定义审计引擎、专有日志格式、内部通知渠道目前只能 fork。

方案权衡：

| 方案 | 优点 | 缺点 | 建议 |
| ---- | ---- | ---- | ---- |
| 进程插件 | 最安全，任意语言实现 | IPC 开销 | 先做这个 |
| WASM | 沙箱隔离，跨平台 | 生态较新，宿主 API 需设计 | 第二步评估 |
| 动态库 | 性能最好 | ABI 不稳定，平台相关，无隔离 | 不建议 |

重构方案已把插件排在内置 Gate、报告、运行时接口稳定之后，这个排序合理——过早固化插件 API 会锁死内部结构。

#### 3. Git 强依赖（P2）

`ensure_git_worktree` 直接调用 `git rev-parse --is-inside-work-tree`，非 Git 仓库无法使用 scope 与 verify。抽象 `VcsProvider` trait 可解，但 Git 覆盖绝大多数目标用户，优先级应低于并行与门禁统一。

#### 4. 单体二进制，无 feature 开关（P2）

4.2 MB 二进制包含全部能力，只用 whitespace 检查的项目同样承担 Docker、Tera、ureq 的体积。

**建议**：

```toml
[features]
default = ["container", "notify", "templates"]
container = []
notify = ["dep:ureq"]
templates = ["dep:tera"]
```

这属于打磨项，不影响功能正确性。

#### 5. 服务适配器覆盖率仍是信息项（P1）

CI 已对 config/verify/process/audit/scope/secrets 核心边界执行 80% 覆盖率门槛，
关键路径执行 95% 门槛；`service` 适配器在 `tools/quality/coverage.py` 中仍标记为
informational，主要依赖服务契约测试。建议继续增加 Docker/Podman 启动失败、清理和
外部实例切换的可重复测试，并明确没有容器运行时时的验证范围。

---

## 四、量化评分

| 维度 | 得分 | 依据 |
| ---- | ---- | ---- |
| 可用性 | 5/5 | 配置严格校验、错误码体系、四种 scope 模式、原子写入、安全默认值 |
| 性能 | 4.5/5 | 并行调度、审计并行、服务复用和 Linux 运行期基线已落地；跨平台基线和体积口径仍需治理 |
| 扩展性 | 4/5 | 模块边界清晰、枚举扩展点、运行时/报告/通知已解耦；强制 Gate 扩展边界与插件仍待设计 |
| 代码质量 | 4.5/5 | 类型安全、错误透传、无环依赖；评审文档与历史体积数据曾出现漂移 |
| 架构健康度 | 4.5/5 | 调度、覆盖率和关键路径已有证据；服务适配器和 skipped 结果的可观测性仍可加强 |

**综合**：★★★★☆ (4.5/5)

---

## 五、改进优先级

### P0/P1（当前迭代）

1. **统一二进制体积口径** — 用干净 checkout、锁定 toolchain 和两个 profile 重新测量，更新 Cargo 注释、基线和发布说明；不要在未解释 4 MB/8 MB 差异前做体积优化。
2. **补齐跨平台性能证据** — CI 对 Ubuntu、macOS、Windows 分别产出 baseline artifact，只在同一 target/toolchain/fixture series 内比较。
3. **保留 skipped 步骤证据** — 在 JSON、Markdown、JUnit 中展示被依赖失败阻塞的节点，同时保持成功报告兼容。
4. **验证 Scope matcher 缓存收益** — 实现已完成，后续只需在基准数据中观察启动延迟变化。

### P2（后续迭代）

5. **强化服务适配器测试** — 覆盖 Docker/Podman 启动失败、清理和外部实例路径。
6. **提交本仓库自门禁配置** — 若决定让 Harness-Gate 自举，提交 `.harness-gate/flow.toml` 并配置 fmt、Clippy、nextest；否则从评审范围移除这项结论。
7. **进程插件、feature 开关、VcsProvider** — 在实际扩展需求出现后再设计稳定接口。

---

## 六、结论

Harness-Gate 的并行执行、覆盖率门槛和 Linux 性能基线已经落地，当前主要风险不再是“功能尚未实现”，而是证据的一致性和失败状态的可见性。优先统一二进制体积数据、完成三平台基线，再处理报告中的 skipped 节点和低成本的 Scope matcher 缓存。插件、feature 拆分和非 Git VCS 都应等真实需求出现后再投入。

---

*本报告基于 v0.3.2（主分支 `01f0bba`）源码、项目文档与门禁配置生成，日期 2026-08-30；体积数字注明了测量 toolchain，跨平台数据以 CI artifact 为准。*
