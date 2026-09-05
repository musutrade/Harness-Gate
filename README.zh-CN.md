# Harness-Gate

[![CI](https://github.com/musutrade/Harness-Gate/actions/workflows/ci.yml/badge.svg)](https://github.com/musutrade/Harness-Gate/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/harness-gate.svg)](https://crates.io/crates/harness-gate)
[![Documentation](https://docs.rs/harness-gate/badge.svg)](https://docs.rs/harness-gate)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

[English](https://github.com/musutrade/Harness-Gate/blob/main/README.md) | [简体中文](https://github.com/musutrade/Harness-Gate/blob/main/README.zh-CN.md)

`Harness-Gate` 是可复用的 Rust 开发工作流与架构门禁 CLI。这是一个独立工具，提供完整的质量门禁和工作流管理能力。

它统一负责 changed paths、secret scan、architecture audit、环境体检、外部命令编排、测试结果计数、超时与中断处理，以及临时服务生命周期。Git hook 只保留启动器，流程判断不依赖 Shell 脚本。

## 阅读导航

- 快速开始：看[安装](#安装)和[快速开始](#安装与快速开始)；
- 接入新项目：看[安装与快速开始](#安装与快速开始)和[内置预设](#内置预设)；
- 增加命令、组件或 CI profile：看[选择模型](#选择模型)、[schema v2 配置参考（中文）](https://github.com/musutrade/Harness-Gate/blob/main/docs/configuration.zh-CN.md)和 [JSON Schema 目录（英文）](https://github.com/musutrade/Harness-Gate/blob/main/schema/README.md)；
- 处理失败：看[验证与报告](#验证与报告)和[故障排查](#故障排查)；
- 扩展 Rust 引擎：看[无需改代码的范围](#无需改代码的范围)和[需要改-rust-的边界](#需要改-rust-的边界)。

## 工作模型

`harness-gate` 把项目流程拆成四类数据：

1. **scope rule**：把 Git 变更路径映射成 component；
2. **profile**：从同一 component 中选择不同强度的步骤，例如 `hook`、`full`、`ci`；
3. **step**：声明一个 `program + args[]` 外部命令、超时、日志、parser 和 service 依赖；
4. **gate**：固定先运行 secret scan 和 audit，成功后才允许外部步骤执行。

运行 `verify` 时的数据流：

```text
Git 变更文件
  -> scope.rules
  -> components
  -> component + profile 匹配的 steps
  -> secret scan
  -> architecture audit
  -> 按配置顺序执行 steps
  -> JSON / Markdown / 可选 HTML/JUnit 报告
  -> 可选 HTTP(S) Webhook 通知
```

component、profile、命令、路径、parser 和 service 都来自 TOML。常规项目迁移不需要在 Rust 中增加枚举或修改匹配分支。

## 安装

### 从 Crates.io 安装（推荐）

```bash
cargo install harness-gate
```

### 从 GitHub Release 安装（预编译二进制）

从不可变的 [GitHub Release tag](https://github.com/musutrade/Harness-Gate/releases/tag/v0.3.7) 下载适合你平台的二进制文件：

- **Linux (x86_64)**: `harness-gate-linux-amd64`
- **macOS (Intel)**: `harness-gate-macos-amd64`
- **macOS (Apple Silicon)**: `harness-gate-macos-arm64`
- **Windows (x86_64)**: `harness-gate-windows-amd64.exe`

安装脚本会先校验 checksum 清单和 Sigstore 证书，再原子替换目标文件。请从同一个不可变
tag 下载脚本并显式传入版本：

```bash
curl --fail --show-error --location --proto '=https' --tlsv1.2 \
  -o /tmp/harness-gate-install.sh \
  https://raw.githubusercontent.com/musutrade/Harness-Gate/v0.3.7/install.sh
bash /tmp/harness-gate-install.sh --version v0.3.7
harness-gate --version
```

默认安装到 `~/.local/bin`，也可以用 `--install-dir` 指定私有目录。脚本不会调用可变的
`releases/latest` API，也不再推荐执行 `raw/main` 安装命令。
安装预编译二进制需要本机安装 `cosign`，用于校验 keyless Sigstore 证书；源码安装还需要
`git` 和 Rust `cargo`。

### 从源码安装

```bash
git clone https://github.com/musutrade/Harness-Gate.git
cd Harness-Gate
cargo install --locked --path tools/harness-gate
```

## 开发命令

仓库使用 `cargo-nextest` 执行快速、隔离的测试：

```bash
cargo nextest run --manifest-path tools/harness-gate/Cargo.toml
cargo clippy --manifest-path tools/harness-gate/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path tools/harness-gate/Cargo.toml -- --check
```

`release` 保留默认的 panic unwind 行为以支持诊断；需要最小分发二进制时使用
`release-small`：

```bash
cargo build --manifest-path tools/harness-gate/Cargo.toml --release
cargo build --manifest-path tools/harness-gate/Cargo.toml --profile release-small
```

worker 在 scheduler 边界由 `catch_unwind` 转换为可发布的失败结果；service 或 lease
锁被 poison 时也会 fail closed，不能被当作可用资源。`release-small` 使用
`panic = "abort"`，调用方不能依赖该二进制的 unwind。稳定的 machine failure 注册表见
[docs/failure-codes.md](docs/failure-codes.md)。

Release 资产同时提供 `SHA256SUMS`、CycloneDX SBOM 和 Sigstore 签名 bundle。
安装前下载二进制、清单以及对应的 `.sig`/`.crt` 文件，先校验摘要，再校验
二进制和 SBOM 的签名：

```bash
sha256sum --check SHA256SUMS
cosign verify-blob --signature harness-gate-linux-amd64.sig \
  --certificate harness-gate-linux-amd64.crt \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github.com/musutrade/Harness-Gate/.github/workflows/release\.yml@refs/tags/v0\.3\.7$' \
  harness-gate-linux-amd64
cosign verify-blob --signature harness-gate.sbom.cdx.json.sig \
  --certificate harness-gate.sbom.cdx.json.crt \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github.com/musutrade/Harness-Gate/.github/workflows/release\.yml@refs/tags/v0\.3\.7$' \
  harness-gate.sbom.cdx.json
```

SBOM 记录源码提交、`Cargo.lock` 摘要和构建工具链。发布流程先生成一个显式的
`release-inventory.json`；checksum、Sigstore 签名/证书、GitHub provenance、校验和上传
都从同一 inventory 派生，并完整覆盖 SBOM。缺失、额外、未签名或未 attested 的资产会在
创建 release 前失败。发布资产不可覆盖；任何替换都必须使用新的版本 tag 和新的 provenance 证明。

## 安装与快速开始

验证安装并查看预设：

```bash
harness-gate --version
harness-gate presets
```

在目标项目中初始化：

```bash
harness-gate --project-root /path/to/new-project init --preset rust-api
harness-gate --project-root /path/to/new-project config check
harness-gate --project-root /path/to/new-project doctor
harness-gate --project-root /path/to/new-project cleanup --dry-run
harness-gate --project-root /path/to/new-project verify --all
```

推荐接入顺序：

1. 选择最接近技术栈的预设；
2. 修改 `.harness-gate/flow.toml` 中的路径、component、scope 和步骤；
3. 在 `.harness-gate/audit.toml` 增加项目自己的架构规则；
4. 在 `.harness-gate/secrets.toml` 增加业务或供应商特有的凭据规则；
5. 执行 `config check`，先解决引用和 schema 错误；
6. 执行 `doctor`，补齐本机工具、依赖、镜像或环境变量；
7. 在干净仓库执行 `verify --all`，确认所有 component 都能运行；
8. 在 CI 中使用相同命令，并按需安装只负责调用 `harness-gate hook` 的薄 hook。

`init` 会创建目标目录，但 `scope` 和 `verify` 需要目标目录是 Git worktree。项目已有配置时默认拒绝覆盖；只有确认目标内容可替换时才使用 `--force`。

## 内置预设

| 预设                    | 用途                        | 初始步骤                                |
| ----------------------- | --------------------------- | --------------------------------------- |
| `generic`               | 任意 Git 项目               | working tree 和 staged whitespace check |
| `rust-api`              | 单 Rust crate               | fmt、Clippy、check、test                |
| `angular-only`          | Angular/npm                 | lint、format check、test、build         |
| `angular-rust-postgres` | Angular + Rust + PostgreSQL | 双端检查、测试、构建和临时数据库        |

`init` 以同目录临时文件和原子重命名写入 `.harness-gate/flow.toml`、`.harness-gate/audit.toml`、`.harness-gate/secrets.toml` 和忽略报告目录的 `.harness-gate/.gitignore`，不会留下半写文件，也不会覆盖已有配置，除非显式传入 `--force`。`config migrate` 对目标配置使用相同的原子写入策略，并在缺失时生成 Secret Scan v2 默认规则。新建 audit v2 文件预置 Rust、TypeScript、JavaScript、SQL、TOML 和 YAML 的词法配置，可直接追加第一条规则。

预设是起点，不是运行时分支。初始化完成后，所有行为都由项目内 TOML 决定：可以重命名 component、增加 `ci` profile、换成 MySQL/Redis、调整目录或替换任意步骤，无需保留预设原有名称。

## 命令总览

| 命令                                                         | 用途                                  |
| ------------------------------------------------------------ | ------------------------------------- |
| `harness-gate presets`                                       | 列出内置项目预设                      |
| `harness-gate init --preset <name>`                          | 生成 schema v2 配置                   |
| `harness-gate doctor [--strict] [--json]`                    | 执行配置声明的环境检查                |
| `harness-gate cleanup [--dry-run] [--json]`                 | 检查或回收过期的 Harness-Gate 资源租约 |
| `harness-gate scope [--staged\|--base REF\|--all] [--json]`  | 列出变更和选择的 components           |
| `harness-gate secrets [--staged] [--json]`                   | 扫描已追踪文件内容中的高置信凭据模式，只输出文件名  |
| `harness-gate audit [--json]`                                | 执行正则架构规则并生成审计报告        |
| `harness-gate verify`                                        | 按工作区变更执行默认 profile          |
| `harness-gate verify --profile ci --all`                     | 对全部 components 执行指定 profile    |
| `harness-gate hook`                                          | 对暂存快照执行 hook profile           |
| `harness-gate step <id>`                                     | 通过 secrets/audit 后单独运行一个步骤 |
| `harness-gate config check`                                  | 校验 schema、引用、路径、环境覆盖和资源安全 |
| `harness-gate config check --format json`                    | 为编辑器和 CI 输出稳定的字段路径诊断    |
| `harness-gate config print --resolved`                       | 输出最终生效配置                      |
| `harness-gate config migrate`                                | 将 schema v1 转换成 v2                |
| `harness-gate schema export`                                 | 生成 flow.toml 的 JSON Schema          |
| `harness-gate adapter run --request <PATH> --trusted-key <PATH>` | 校验并执行一个进程外签名 adapter 请求 |
| `harness-gate parse-logs`                                    | 提取 JSON Lines ERROR trace 上下文    |

所有命令都支持全局 `--project-root <PATH>` 和 `--config <PATH>`。命令成功返回 0；配置错误、门禁失败、步骤失败、超时或中断返回非 0，适合直接用于 CI。

`cleanup --dry-run` 可以在重试前或共享 CI 主机上安全执行，只列出带有
`harness-gate` owner marker 的租约，并把结构化观察写入
`<reports>/cleanup.json`。lease schema `2` 记录规范化项目身份、逻辑资源、invocation、
完整 runtime labels、容器名和不可变 runtime object ID。过期只表示可以尝试回收；停止前
会重新 inspect，并比较租约文件名、项目、资源、invocation、labels 和不可变 ID。未知、
损坏、改名、活动或无法证明 ownership 的租约会保留，停止失败也会保留租约并返回非零状态。

交互式终端中的 `verify` 会显示进度条，并以颜色区分通过、警告和失败。重定向输出或 CI 保持纯文本；可使用 `--color auto`（默认）、`--color always` 或 `--color never` 控制颜色，`NO_COLOR` 会关闭自动颜色。

本仓库使用等价的 Cargo 别名：

```bash
harness-gate scope
harness-gate verify --components backend,frontend
harness-gate verify --profile full --all
```

## 典型工作流

### 日常开发

```bash
# 编码前确认范围
harness-gate scope

# 编码后只验证命中的组件
harness-gate verify

# 定向重复运行一个步骤；仍会先过 secrets 和 audit
harness-gate step frontend.tests
```

工作区没有变更时，默认 scope 不选择 component，`verify` 只运行固定门禁。需要确认整个仓库时必须显式传 `--all`。

### 提交前

```bash
git add <明确的文件清单>
harness-gate scope --staged
harness-gate hook
git commit -m "..."
```

仓库的 pre-commit 会自动执行 `harness-gate hook`。hook profile 只保留快速确定性检查，不代替完整测试。

### PR 或发布前

```bash
harness-gate verify --all
```

该命令忽略变更路径，选择配置中的所有 component，并执行默认 `full` profile。

### 比较某个基线

```bash
harness-gate scope --base origin/main
harness-gate verify --base origin/main
```

`--base REF` 使用 `REF...HEAD` 的已提交变更，不包含未提交工作区内容，适合 CI 或 PR 分支验证。

### 手动指定范围

```bash
harness-gate verify --components backend
harness-gate verify --components backend,frontend --profile full
```

显式 components 会覆盖自动 scope，并且不能与 `--staged`、`--base`、`--all` 同时使用。未知 component 或 profile 会立即失败。

## 错误码

运行失败会输出 `ERROR [E####]: message`。错误码可用于 CI 日志与问题定位，
消息会保留文件路径或 Git 命令等上下文。

| 错误码 | 分类 |
| ------ | ---- |
| `E1000` | 通用命令、项目或配置失败 |
| `E1101`-`E1103` | 审计配置、执行或日志解析 |
| `E1201`-`E1202` | 秘密扫描配置或执行 |
| `E1301`-`E1304` | Git scope、scope 配置、未匹配路径或报告 |
| `E1401`-`E1404` | 验证选择、取消、执行或报告 |

### 常见修复路径

新项目可以按以下顺序接入：

```bash
harness-gate init --preset generic
harness-gate config check
harness-gate doctor
harness-gate verify --all
```

`config check` 失败时，人类可读输出会包含稳定错误码、字段路径、修复
提示和最小 schema v2 `flow.toml` 骨架。按提示修改，或执行
`harness-gate init --preset generic` 重新生成配置，然后再次运行
`harness-gate config check`。编辑器和 CI 需要结构化结果时使用
`config check --format json`。

日常开发先运行 `harness-gate scope`，再运行 `harness-gate verify`；提交前
暂存明确文件，执行 `harness-gate scope --staged` 和 `harness-gate hook`；
PR 或发布前执行 `harness-gate verify --all`，失败时上传 reports 目录供定位。

## Schema v2 概览

项目根由 `.harness-gate/flow.toml` 标识，不要求固定的 `backend/`、`frontend/` 或工具目录。component 和 profile 都是配置中的小写字符串 ID。

```toml
version = 2

[project]
name = "example"
default_profile = "full"
hook_profile = "hook"

[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[paths.aliases.api]
path = "services/api"
env = "API_DIR"

[[scope.rules]]
patterns = ["services/api/**"]
components = ["api"]

[[steps]]
id = "api.clippy"
label = "API Clippy"
component = "api"
profiles = ["full", "hook", "ci"]
program = "cargo"
args = ["clippy", "--manifest-path", "{api}/Cargo.toml"]
cwd = "{root}"
log = "api_clippy.log"
timeout_secs = 180
```

可用占位符包括 `{root}`、`{reports}`、`{audit_config}` 和任意 `[paths.aliases.*]`。命令使用 `program + args[]`，不经过 Shell 字符串解析。

`REPORT_DIR` / `HARNESS_GATE_REPORTS`、`HARNESS_GATE_SECRETS_CONFIG` 和步骤或服务声明的 `*_env` 字段可覆盖相应配置值。审核配置始终由当前项目 `flow.toml` 中字面量、仓库相对的 `paths.audit_config` 决定，不接受进程级审核配置覆写或环境插值；切换项目或工作流文件时显式传入 `--project-root` 和 `--config`。

完整字段、默认值、限制和示例见 [schema v2 配置参考](https://github.com/musutrade/Harness-Gate/blob/main/docs/configuration.zh-CN.md)。英文参考和全部 JSON Schema 见 [English configuration reference](https://github.com/musutrade/Harness-Gate/blob/main/docs/configuration.md) 与 [schema catalog](https://github.com/musutrade/Harness-Gate/blob/main/schema/README.md)。修改配置后先运行：

```bash
harness-gate config check
harness-gate config print --resolved
```

## 主要扩展点

### Doctor

`[[doctor.checks]]` 支持 `command`、`path`、`glob`、`env`、`env-or-file`、`git-config`、`git-remotes`、`version` 和 `service`。`required = false` 的失败表现为 warning，其余为 failure。

```bash
harness-gate doctor            # warning 不影响退出码
harness-gate doctor --strict   # warning 也导致非 0
harness-gate doctor --json     # 给 CI 或其他工具消费
```

### 测试解析器

`[parsers.<id>]` 支持 `regex`、JUnit、TRX 和 JSON 标准结果；步骤通过 `parser = "<id>"` 引用。
解析器会把 malformed、零结果和部分结果记录为不同失败码，避免命令虽退出 0 却没有真正执行测试。

### 临时服务

`[services.<id>]` 支持两种 provider：

- `kind = "environment"`：从已有环境变量读取连接值并注入步骤；
- `kind = "docker"`：声明镜像、端口、环境、健康检查、连接串和目标环境变量。

Docker provider 可用于 PostgreSQL、MySQL、Redis 等服务，也可通过 `runtime = "podman"` 使用 Docker
兼容的 Podman CLI。容器使用随机宿主端口，验证结束或异常退出时自动删除；省略 `runtime` 时默认使用 Docker。
本次计划选中的服务会在 secret scan 和 architecture audit 门禁运行时预热，减少首个步骤的等待时间。

步骤可用 `services = ["test-postgres", "test-redis"]` 组合多个服务，并以 `remove_env = ["DATABASE_URL"]` 删除继承的运行时变量。每个 service 必须注入不同变量，避免静默覆盖。

### 项目策略

`[policy].required_steps` 声明项目不可缺失的基础步骤。策略属于项目配置，不再编译进通用引擎；新增 component、profile、普通命令、路径别名、regex parser 或 Docker service 都不需要修改 Rust 源码。

## 选择模型

### Working tree

默认 `scope` 合并以下文件集合：

- 未暂存修改：`git diff --name-only`；
- 已暂存修改：`git diff --cached --name-only`；
- 未忽略的未跟踪文件：`git ls-files --others --exclude-standard`。

这些路径按 `[[scope.rules]]` 匹配，命中的 component 去重后用于选择步骤。`[scope].unmatched` 控制未命中路径：`fail`（默认）立即失败并列出文件，`all` 选择全部 component，`ignore` 仅在明确接受漏测风险时忽略。

### Staged

`scope --staged` 和 `hook` 只读取暂存快照。secret scan 也通过 Git index 读取文件内容，而不是读取可能不同的工作区版本。

### Step input

配置步骤默认使用 `input = "snapshot"`，因此 `{root}`、path alias 和参数都相对于本次 invocation
的输入根解析。确实需要原始 checkout 或直接读取 Git 元数据的步骤，可以显式设置
`input = "repository"`；这是一项需要审阅的兼容能力，不会改变 scope、secret scan 或 architecture audit
的输入语义。invocation 报告会记录输入模式、source identity、execution root 和 configuration digest。

### All

`--all` 不依赖 Git diff，直接选择配置步骤中出现的全部 component。适合交付门禁和干净 checkout。

### Profile

profile 由步骤的 `profiles = [...]` 隐式声明。`verify` 使用 `[project].default_profile`，`hook` 使用 `[project].hook_profile`，`verify --profile <id>` 可选择其他 profile。

## 固定安全门禁

以下行为不由项目配置关闭：

1. 外部步骤前必须依次通过 secret scan 和 audit；
2. 配置、报告和路径别名不得逃出项目根；
3. 禁止 `sh -c`、`bash -lc` 等 Shell 命令字符串；
4. 未知引用、重复 ID、非法 glob/regex/占位符和越界超时直接失败；
5. service 容器必须声明健康检查并在结束时清理；
6. 多个 service 不得向同一步骤注入同名环境变量。
7. 审计扫描根必须存在且位于项目内，`..` 和逃出项目的符号链接会被拒绝；
8. Doctor、Git 探测和 Docker 生命周期命令都有硬超时，超时时终止整个子进程组。

secret scan 检查 Git 已追踪文件和未忽略的未跟踪文件内容。具体规则由 `[paths].secrets_config` 指向的 TOML 文件提供，默认覆盖 GitHub/GitLab/npm Token、AWS access key、JWT、命名签名密钥、PostgreSQL 凭据 URL、Webhook、企业微信/钉钉密钥、HTTP Basic Auth 和 PEM 私钥头。捕获值会经过占位符与低信息值过滤；报告只记录文件名和规则 ID，不把凭据内容复制到终端或 JSON。这是快速前置检查，不替代 gitleaks、TruffleHog 等专用扫描器。

为限制扫描过程的内存使用，secret scan 和 architecture audit 都拒绝超过 16 MiB 的单个输入文件，并将该错误作为门禁失败返回。

## v1 迁移

```bash
harness-gate --project-root /path/to/project config migrate \
  --input harness-gate/.codex/flow.toml \
  --output .harness-gate/flow.toml
```

迁移器保留源文件，把原有 backend/frontend、doctor、PostgreSQL、parser、scope 和 steps 转成 v2。目标文件已存在时需要显式 `--force`。

迁移后必须执行：

```bash
harness-gate config check
harness-gate doctor
harness-gate verify --all
```

确认新配置通过后，再由人工决定何时删除 v1 文件。迁移命令本身不会删除源文件。

## 验证与报告

`verify` 固定按以下顺序执行：

1. working tree 或 staged snapshot secret scan；
2. auditor 全量架构扫描；
3. 按配置顺序执行选中 component/profile 的步骤。

报告目录由 `[paths].reports` 决定。当前项目使用 `harness-gate/.codex/reports/`。

| 文件                  | 内容                            | 常见消费者         |
| --------------------- | ------------------------------- | ------------------ |
| `changed_files.txt`   | 当前 scope 的变更文件，每行一个 | Reviewer           |
| `scope.json`          | scope mode、文件和 components   | CI、自动化工具     |
| `secret_scan.json`    | 扫描模式和命中文件名            | 安全门禁           |
| `review_context.json` | 完整审计结果、规则、文件和行号  | 修复代理、Reviewer |
| `review_context.md`   | 截断的人类可读审计摘要          | 终端或 LLM 上下文  |
| `test_result.json`    | schema v1、scope、步骤状态/attempts、失败和 artifact 引用 | CI、统计 |
| `test_result.md`      | 简洁验证摘要和 `TEST_SUMMARY`   | 人工查看           |
| `test_result.html`    | 按仓库内模板渲染的可选 HTML      | CI、人工查看       |
| `<junit>.xml`         | 报告目录内配置的可选 JUnit XML   | CI 测试平台        |
| `logs/<step>.log`     | 外部命令完整 stdout/stderr      | 失败诊断           |

终端只展示摘要。步骤失败时先看 `test_result.md` 中的日志路径，再打开对应日志；不要只根据最后一行猜测根因。

`test_result.json` 遵循[版本化 machine-result schema](https://github.com/musutrade/Harness-Gate/blob/main/schema/machine-result.schema.json)。为兼容旧消费者仍保留
`passed`，新消费者应使用稳定的 `status`、步骤 `attempts`、结构化 `failures`、invocation-relative 的
`artifacts` 和 `evidence_complete`，不要解析 Markdown 或日志文本来判断结果。

机器结果还记录 parser 模式/版本和完整性。优先使用 JUnit、TRX 或 JSON
标准结果；malformed、零结果和部分结果分别映射为 `RESULT_PARSE_FAILURE`、
`RESULT_ZERO` 和 `RESULT_PARTIAL`。JUnit 只接受单个 `testsuite` 或 `testsuites`
根元素，TRX 只接受单个 `TestRun` 根元素（支持命名空间前缀）；缺根、多根或根元素外
非空内容均 fail closed。有界重试记录 `retry_count` 与 `flaky`，
分片记录 merge identity 并拒绝缺失或重复测试身份。有效的到期 waiver 使用
机器可区分的 `WAIVED`，并包含审批和补偿控制证据。

JSON producer 如果输出嵌套数组或数字摘要，应显式声明 `count_path`：

```toml
[parsers.json-results]
kind = "json"
count_path = "summary.total"
minimum = 1
```

未配置时只自动发现裸顶层数组，或对象包装层中的一个受支持结果数组
（`testcases`、`testCases`、`test_results`、`testResults`、`results`）。
`duration_ms` 等任意数字、metadata/attachments 等无关数组、非数组字段和多个
候选（即使长度相同）都会以 `RESULT_PARSE_FAILURE` 拒绝；显式路径无效时也不会
回退自动发现。

迁移时可重放串行请求、比较归一化结果并记录金丝雀/回滚：

```bash
harness-gate compat run --input request.json --output result.json --old-result frozen.json
harness-gate compat compare --old frozen.json --new result.json --output comparison.json
harness-gate compat canary --state migration-canary.json --slice team-a
harness-gate compat rollback --state migration-canary.json
```

这些命令保留原始摘要和 invocation 证据，不会删除已有报告。
P2 进程外签名 adapter 边界见 [ADR-0033](docs/adr/0033-signed-out-of-process-adapter-protocol.md)。可使用 host
执行单个签名请求：

```bash
harness-gate adapter run \
  --request adapter-request.json \
  --trusted-key adapter-key.json \
  --allow-resource test-database
```

host 使用协议 v2，在启动前校验覆盖完整请求的 Ed25519 签名（adapter 身份、invocation/step、
参数、输入、环境、能力、超时、配置摘要、产物根目录、nonce 和有效期）以及可执行文件
SHA-256；清空继承环境，执行协议级能力白名单，限制 stdout/stderr 和产物预算，并拒绝
malformed 结果或越过 invocation 根目录的产物。nonce 在 host policy（CLI 另有持久化 sidecar）
下只允许使用一次；超限、超时、取消、重放和其他协议错误统一记录为
`ADAPTER_PROTOCOL_FAILURE`。

请求是一个独立的 JSON 文档，不从 `flow.toml` 读取，也不会自动启用内置步骤的
adapter。每个受信任的 Ed25519 公钥重复传入一次 `--trusted-key`；只传入本次调用
允许的 `--allow-network`、`--allow-resource` 和 `--allow-environment` 能力。执行前应
确认请求、可执行文件、产物根目录位于项目或明确受管控的部署目录内，并审核签名者。
能力白名单只是协议层声明检查，不是操作系统级的网络、文件系统、资源或进程 sandbox；
进程组清理是有界的 best-effort 尝试，也不等于完整的 descendant containment 证明。每个
输出流和单个证据文件默认最多保留 16 MiB、adapter 产物合计最多 64 MiB，单次 invocation
证据合计最多 256 MiB，reader 有独立 deadline。

JSON Lines 应用日志可提取同一 trace 的上下文：

```bash
harness-gate parse-logs \
  --input path/to/application.jsonl \
  --output /tmp/error-context.txt
```

解析器优先选择第一条 `level = ERROR` 所在的 `trace_id`，支持从事件字段、`data`、当前 `span` 和 `spans` 中提取，再收集相同 trace 的结构化记录；没有 trace 时退化为原始日志最后 30 行。

解析采用有界流式读取并在输出前脱敏：错误前最多保留 20 条上下文，输出最多 30 条记录，不会把整份无限增长的 JSON Lines 日志载入内存。已启动 service 的清理失败也会使验证失败，并写入验证报告，避免把容器泄漏隐藏为成功。

## Git Hook

当前仓库使用：

```bash
git config core.hooksPath harness-gate/hooks
```

`pre-commit` 执行 `harness-gate hook`。hook profile 不运行数据库集成测试或 production build；交付前使用 `harness-gate verify --all`。

业务代码模板由 `.codex/templates/manifest.json` 统一登记。`hook` 和 `full` 流程会执行模板质量门禁，检查清单覆盖、占位符一致性和示例渲染结果；TypeScript 模板使用编译器诊断，Rust 模板使用 `rustfmt --check`，SQL 模板检查引号、注释、括号和语句终止符。对应入口为 `scripts/check-templates.mjs`，负向测试位于 `scripts/check-templates.test.mjs`。

独立安装方式的新项目可以创建同样的薄 hook：

```sh
#!/bin/sh
set -eu
root="$(git rev-parse --show-toplevel)"
cd "$root"
exec harness-gate hook
```

hook 只负责定位根目录和启动二进制，所有选择、门禁和步骤仍在 Rust 与 TOML 中。

## CI 集成

CI 推荐执行全量 profile，而不是依赖 runner 上的工作区 diff：

```bash
harness-gate config check
harness-gate doctor --strict
harness-gate verify --all
```

如需复用外部测试服务，向 job 注入 service 配置的 `external_env`；否则预拉取配置镜像并允许 runner 访问 Docker daemon。缓存 Cargo、npm 和构建目录只影响性能，不应跳过 `verify --all`。

无论成功或失败，都建议上传 `[paths].reports` 目录作为 artifact。这样可以保留审计行号、步骤耗时和完整日志。

可选的 `[[notifications.webhooks]]` 会在报告文件写入后发送 JSON。每个 URL 必须配置精确匹配的
`allowed_hosts`；凭据、通配符、解析后的 loopback/private/link-local/unspecified/multicast 地址和重定向都会
在配置或连接时拒绝。`on_failure` 默认开启，`on_success` 默认关闭；非 2xx 响应、连接错误或策略拒绝返回
报告错误 `E1404`，但不会删除已生成的报告。目的地证据只保留 scheme 和规范化主机。完整字段、模板路径
约束和 JUnit 示例见[配置参考](https://github.com/musutrade/Harness-Gate/blob/main/docs/configuration.zh-CN.md#报告模板junit-和通知)。多个 webhook 按配置顺序发送，首个失败会停止后续通知。

## 故障排查

### 没有 component 被选中

工作区没有变更时不会选择 component。变更路径没有命中 scope rule 时，默认 `unmatched = "fail"` 会列出遗漏文件并失败；修正 `[[scope.rules]]`，或为希望触发全量验证的项目设置 `unmatched = "all"`。只有明确接受未匹配文件不触发验证时才使用 `ignore`。

### `unknown component` / `unknown profile`

component 来自 `[[steps]].component`，profile 来自 `[[steps]].profiles`。运行 `harness-gate config check` 查看引用错误，或 `config print --resolved` 确认最终配置。

### Docker 或 Podman daemon 不可用

可以启动配置中选择的 Docker/Podman runtime，或者设置 service 的 `external_env`，例如 `TEST_DATABASE_URL`。
`doctor` 中 `required = false` 的 service 检查只产生 warning，但真正依赖该 service 的步骤仍会失败。

### Docker/Podman image 不存在

容器 provider 使用 `--pull=never`，不会在验证中隐式访问网络。按 Doctor 提示执行 `<runtime> pull <image>`，
或通过 `image_env` 指向本机已有镜像。

### 测试命令成功但 parser 失败

查看步骤日志，确认测试框架实际输出与 `patterns` 一致，并且 capture group 是整数。不要把 `minimum` 改为 0；应修正规则或测试命令，确保零测试不会被误判为成功。

### 步骤超时

先看日志判断是死锁、网络等待还是超时过短。需要项目或 CI 差异化时设置步骤或 service 的 `timeout_env`，不要复制两套配置。步骤允许 1 到 3600 秒，service 整个启动过程允许 1 到 300 秒；Doctor 单项检查用 `timeout_secs` 控制，默认 15 秒、范围 1 到 300 秒。

### 配置路径被拒绝

配置、报告、audit 文件和 path alias 必须位于项目根内。不要用 `..`、仓库外绝对路径或逃出仓库的符号链接；把需要的输入放到仓库内，或通过受控环境变量向步骤传值。

## 无需改代码的范围

以下变化只修改 `.harness-gate/flow.toml` 或 audit TOML：

- 新增或重命名 component、profile、step；
- 接入 Go、Java、Python、Node 或其他 CLI 工具；
- 调整 monorepo 路径和 scope；
- 增加 Doctor 检查；
- 增加 regex 测试结果解析器；
- 增加 Docker 或环境变量 service；
- 增加硬性规则、分层规则和逐行允许规则；
- 为 CI 增加独立超时、镜像或目录覆盖变量。

## 需要改 Rust 的边界

只有引擎出现新的行为类别时才需要修改源码，例如：

- 新增非 Docker 的服务生命周期 provider；
- 新增无法用 regex 表达的测试报告格式；
- 新增现有 Doctor kind 无法表达的检查协议；
- 修改凭据识别算法、进程取消机制或固定安全策略；
- 增加远程执行、并行调度或新的报告格式。

这类改动应同时增加单元测试、内置预设验证和配置兼容性说明，并提升版本号。

## 当前审计规则

`.harness-gate/audit.toml` 保存项目自己的 SQL、分层和模板约束。audit 配置当前 schema 为 v2，必须显式声明 `version = 2` 和 `[engine]`；规则扩展名没有对应 `comment_syntax` 时会 fail closed。旧版字符串 allowlist、缺失 engine 和版本升级方法见[配置迁移参考](https://github.com/musutrade/Harness-Gate/blob/main/docs/configuration.zh-CN.md#audit-v2-migration)。`arch_rules.allowed_patterns` 可声明逐行例外，不存在写死的 model trait 放行逻辑。

auditor 以整文件为单位执行正则检查并把命中映射回起始代码行：支持跨行规则、扩展名过滤、路径排除、显式类型的路径 allowlist 和起始行 allowed pattern。行注释、块注释及字符串定界符按扩展名配置，扫描时跟踪词法状态；正则默认启用 multi-line 模式。需要抽象语法树级判断时，应把 Clippy、ESLint 或其他语言 lint 工具配置为 step。

## 开发 harness-gate 本身

```bash
cargo fmt --manifest-path harness-gate/tools/harness-gate/Cargo.toml -- --check
cargo clippy --manifest-path harness-gate/tools/harness-gate/Cargo.toml \
  --locked --all-targets --all-features -- -D warnings
cargo test --manifest-path harness-gate/tools/harness-gate/Cargo.toml --locked
harness-gate verify --components workflow
```

修改配置模型时还应验证全部内置预设和 v1 migration 测试；修改 service provider 时必须执行 `harness-gate verify --all`，确认一次性容器会在成功和失败路径上清理。
