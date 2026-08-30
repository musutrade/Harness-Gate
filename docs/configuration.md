# harness-gate schema v2 配置参考

本文档说明 `.harness-gate/flow.toml` 和审计规则文件的完整配置模型。首次接入请先阅读上一级的 [操作手册](../README.md)，再按需查阅本参考。

## 1. 文件与加载顺序

默认项目配置是仓库内的 `.harness-gate/flow.toml`。`harness-gate` 从当前目录向父目录查找该文件，也可以显式指定：

```bash
harness-gate --project-root /path/to/project config check
harness-gate --project-root /path/to/project --config config/ci-flow.toml config check
```

配置路径必须位于项目根内部。相对路径按项目根解析；绝对路径、`..` 越界路径和指向仓库外的符号链接会被拒绝。

默认情况下只读取当前项目的 `.harness-gate/flow.toml`。配置字段声明的
环境变量只覆盖对应字段；需要选择其他项目或工作流文件时，显式使用
CLI 的 `--project-root` 和 `--config`。

使用以下命令查看配置是否有效以及环境覆盖后的结果：

```bash
harness-gate config check
harness-gate config check --format json
harness-gate config print --resolved
harness-gate schema export
```

`schema export` 将当前配置模型导出到 `schema/flow.schema.json`，可提交到编辑器或 CI
进行静态补全和结构检查。配置字符串支持一次性的环境变量插值：`${NAME}` 要求变量已设置，
`${NAME:-default}` 在变量缺失时使用默认值。不支持递归插值或表达式。插值发生在 TOML 解析前，
随后仍按现有专用环境变量覆盖规则处理。`paths.audit_config` 是例外：必须使用字面量的
仓库相对路径，不支持环境插值，以确保审核策略始终属于当前项目。

`config check` 会在启动 service、子进程或写入报告前完成静态验证。默认输出面向
终端；`--format json` 将 stdout 固定为一个版本化诊断对象，方便 CI 和编辑器集成。每项
错误包含稳定的 `HGCFG-*` ID、字段路径、原因、修复建议，以及文件输入可用时的行/列；
不会显示插值后的环境变量值、连接字符串或模板内容。JSON Schema 只验证结构，环境插值、
环境覆盖、仓库路径、跨字段引用和资源安全必须由 `config check` 验证。

### 编辑器 Schema 关联

仓库中的 `schema/flow.schema.json` 是 `.harness-gate/flow.toml` 的权威本地 Schema。
先在仓库根执行 `harness-gate schema export`（CI 会检查其与已提交文件一致），再使用以下
本地关联；两种方式都不要求远程 URL：

**VS Code（Even Better TOML）** 在工作区 `.vscode/settings.json` 中加入：

```json
{
  "evenBetterToml.schema.associations": {
    "./schema/flow.schema.json": [".harness-gate/flow.toml"]
  }
}
```

**Taplo** 在仓库根的 `.taplo.toml` 或个人 Taplo 配置中加入：

```toml
[[schema.associations]]
url = "./schema/flow.schema.json"
include = [".harness-gate/flow.toml"]
```

编辑器应先展示结构性错误，再运行 `harness-gate config check --format json` 获取路径、
依赖与资源安全诊断。

## 2. 命名和路径约束

所有 project、component、profile、step、parser、service、doctor check 和 path alias ID 必须：

- 非空；
- 只包含小写 ASCII 字母、数字、`.`、`-`、`_`；
- 例如 `api.tests`、`frontend`、`pre_commit` 合法，`API Tests` 不合法。

环境变量名只能包含大写 ASCII 字母、数字和下划线。`program` 必须是 PATH 中的可执行文件名，不能包含 `/` 或 `\\`。

仓库路径必须是非空相对路径，不能包含父目录跳转。以下占位符可用于参数、Doctor 路径和部分配置值：

| 占位符           | 含义                                                       |
| ---------------- | ---------------------------------------------------------- |
| `{root}`         | 项目根的绝对路径                                           |
| `{reports}`      | 报告目录的绝对路径                                         |
| `{audit_config}` | 审计规则文件的绝对路径                                     |
| `{<alias>}`      | `[paths.aliases.<alias>]` 解析后的绝对路径                 |
| `{host_port}`    | 仅 Docker service 的 `connection` 使用，由随机宿主端口替换 |

步骤的 `cwd` 必须是单个 `{root}` 或 `{<alias>}`，不能写成 `{root}/backend`。需要子目录时先声明 path alias。

## 3. 顶层结构

```toml
version = 2

[project]
# ...

[paths]
# ...

[policy]
# ...

[execution]
# parallel = false
# max_parallel = 4

[notifications]
# [[notifications.webhooks]]
# url = "${HARNESS_GATE_WEBHOOK_URL}"
# on_failure = true
# on_success = false

[[doctor.checks]]
# ...

[services.example]
# ...

[parsers.example]
# ...

[[scope.rules]]
# ...

[[steps]]
# ...
```

| 区域                | 必需 | 用途                        |
| ------------------- | ---- | --------------------------- |
| `version`           | 是   | 当前固定为 `2`              |
| `[project]`         | 是   | 项目标识和默认 profile      |
| `[paths]`           | 是   | 报告、审计规则和路径别名    |
| `[policy]`          | 否   | 声明不可缺失的步骤          |
| `[execution]`       | 否   | 控制依赖就绪步骤的并行调度  |
| `[scope]`           | 否   | 未匹配变更路径的处理策略    |
| `[[doctor.checks]]` | 否   | 本机环境体检                |
| `[services.*]`      | 否   | 外部环境或 Docker 临时服务  |
| `[parsers.*]`       | 否   | 从测试日志计算结果数        |
| `[[scope.rules]]`   | 是   | 变更路径到 component 的映射 |
| `[[steps]]`         | 是   | 实际执行的命令步骤          |
| `[report_templates]` | 否  | 可选 HTML 模板和 JUnit 输出配置 |
| `[notifications]`   | 否   | 可选 HTTP Webhook 通知         |

未知字段会直接导致解析失败，避免拼写错误被静默忽略。

## 4. `[project]`

```toml
[project]
name = "orders-api"
default_profile = "full"
hook_profile = "hook"
```

| 字段              | 说明                                      |
| ----------------- | ----------------------------------------- |
| `name`            | 项目 ID，也用于临时容器名称               |
| `default_profile` | `harness-gate verify` 未传 `--profile` 时使用 |
| `hook_profile`    | `harness-gate hook` 使用的快速 profile        |

两个 profile 都必须至少被一个步骤引用。profile 不需要单独声明，它由步骤的 `profiles` 集合产生。

## 5. `[paths]` 和 aliases

```toml
[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[paths.aliases.api]
path = "services/api"
env = "ARC_FLOW_API_DIR"

[paths.aliases.web]
path = "apps/web"
```

| 字段                | 说明                               |
| ------------------- | ---------------------------------- |
| `reports`           | JSON、Markdown 和步骤日志目录      |
| `audit_config`      | auditor 规则文件                   |
| `secrets_config`    | Secret Scan 规则及占位符策略文件   |
| `aliases.<id>.path` | 仓库内目录或文件路径               |
| `aliases.<id>.env`  | 可选；覆盖 alias 路径的环境变量    |

`root`、`reports`、`audit_config`、`host_port` 是保留 alias 名称。

通用覆盖变量：

| 环境变量                                    | 覆盖字段             |
| ------------------------------------------- | -------------------- |
| `REPORT_DIR` 或 `HARNESS_GATE_REPORTS`      | `paths.reports`        |
| `HARNESS_GATE_SECRETS_CONFIG`               | `paths.secrets_config` |

`paths.audit_config` 始终从当前项目的 `flow.toml` 读取，且必须是字面量的
仓库相对路径，不能使用 `${...}` 环境插值。`PROJECT_ROOT`、
`HARNESS_GATE_CONFIG`、`AUDITOR_CONFIG` 和 `HARNESS_GATE_AUDIT_CONFIG` 不参与
项目或审核配置发现，避免共享终端、IDE 或 CI 环境中的一个项目影响另一个项目。
需要选择其他项目或工作流文件时，显式使用 `--project-root` 和 `--config`。

## 6. `[execution]`

默认配置保持串行执行。只有显式设置 `parallel = true` 才会并发运行没有依赖路径的步骤：

```toml
[execution]
parallel = true
max_parallel = 4
```

| 字段           | 默认值 | 说明 |
| -------------- | ------ | ---- |
| `parallel`     | `false` | 是否启用受依赖关系约束的并行调度 |
| `max_parallel` | 并行时为 `4` | 同时运行的步骤上限，显式值必须为 `1` 到 `64` |

`max_parallel = 0`、负数或大于 `64` 会在执行前拒绝。并行步骤仍受
`depends_on`、服务资源预检和运行时服务锁约束；secret scan 与 architecture audit
的默认门禁顺序不会被绕过。每个步骤使用独立日志文件，主线程按稳定计划顺序合并结果和打印输出，
因此完成时间先后不会改变 CLI 或报告顺序。取消、超时和失败会阻止后续依赖节点，并清理已启动的进程
与服务。

当依赖失败导致节点未执行时，JSON 报告会额外写入 `skipped_steps`，其中包含节点
`id`、`label` 和阻塞 `reason`；成功报告会省略空数组以保持原有字段形状。Markdown
报告使用 `SKIPPED` 行，JUnit 报告使用 `<skipped>` testcase。被跳过的节点不会被计为
已执行失败，但会使整个验证结果保持失败。

## 7. `[policy]`

```toml
[policy]
required_steps = ["api.format", "api.clippy", "api.tests"]
```

`required_steps` 中的每个 ID 都必须出现在 `[[steps]]` 中，而且不能重复。这用于防止项目基础门禁被误删。它不规定步骤属于哪个 profile；profile 仍由步骤自身配置。

## 8. `[[doctor.checks]]`

每项检查都有公共字段：

```toml
[[doctor.checks]]
id = "tool.git"
label = "git"
required = true
timeout_secs = 15
help = "install Git and ensure it is on PATH"
kind = "command"
program = "git"
args = ["--version"]
```

| 字段           | 默认值 | 说明                                    |
| -------------- | ------ | --------------------------------------- |
| `id`           | 无     | 唯一检查 ID                             |
| `label`        | 无     | 终端和 JSON 报告中的显示名称            |
| `required`     | `true` | `true` 失败计为 FAIL，`false` 计为 WARN |
| `timeout_secs` | `15`   | 单项检查硬超时，范围 1 到 300 秒        |
| `help`         | 无     | 失败时追加的修复提示                    |
| `kind`         | 无     | 检查类型                                |

支持的 kind：

| kind          | 字段                                     | 行为                                           |
| ------------- | ---------------------------------------- | ---------------------------------------------- |
| `command`     | `program`, `args`                        | 执行命令并要求退出码为 0                       |
| `path`        | `path`, `path_type`                      | 检查任意路径、文件或目录                       |
| `glob`        | `pattern`                                | 要求 glob 至少命中一个路径                     |
| `env`         | `name`                                   | 要求环境变量存在                               |
| `env-or-file` | `env`, `path`, `contains`                | 环境变量存在，或文件中有以 `contains` 开头的行 |
| `git-config`  | `key`, `expected`                        | 要求 Git 配置等于预期值                        |
| `git-remotes` | 无                                       | 检查 Git remote 配置                           |
| `version`     | `program`, `args`, `path`, `trim_prefix` | 比较命令输出与版本文件                         |
| `service`     | `service`                                | 检查 service 的外部变量或 Docker 可用性        |

示例：

```toml
[[doctor.checks]]
id = "node.version"
label = "Node version"
kind = "version"
program = "node"
args = ["--version"]
path = "{root}/.node-version"
trim_prefix = "v"

[[doctor.checks]]
id = "frontend.dependencies"
label = "frontend dependencies"
kind = "path"
path = "{web}/node_modules"
path_type = "directory"
help = "run `cd apps/web && npm ci`"

[[doctor.checks]]
id = "test.database"
label = "test database"
required = false
kind = "service"
service = "test-postgres"
```

`path_type` 可取 `any`、`file`、`directory`，默认 `any`。命令、Git 配置、remote 和 service 探测都受 `timeout_secs` 约束，超时时会终止整个子进程组。CI 中通常使用 `harness-gate doctor --strict`，把 WARN 也视为失败。

## 9. `[services.*]`

### 8.1 Environment service

适合由 CI、开发机或密钥管理系统提供现成连接值：

```toml
[services.test-redis]
kind = "environment"
source_env = "CI_REDIS_URL"
inject_env = "TEST_REDIS_URL"
```

步骤启动前读取 `source_env`，并以 `inject_env` 注入子进程。变量不存在时步骤失败。

### 8.2 Docker-compatible container service

```toml
[services.test-postgres]
kind = "docker"
runtime = "docker" # docker | podman
image = "postgres:16-alpine"
image_env = "ARC_FLOW_POSTGRES_IMAGE"
external_env = "TEST_DATABASE_URL"
inject_env = "TEST_DATABASE_URL"
external_value_policy = "isolated-postgres"
startup_timeout_secs = 30
timeout_env = "ARC_FLOW_DATABASE_TIMEOUT_SECS"
container_port = 5432
environment = { POSTGRES_USER = "test", POSTGRES_PASSWORD = "test", POSTGRES_DB = "app_test" }
healthcheck = ["pg_isready", "-U", "test", "-d", "app_test"]
connection = "postgres://test:test@127.0.0.1:{host_port}/app_test"
```

| 字段                   | 必需 | 说明                                           |
| ---------------------- | ---- | ---------------------------------------------- |
| `runtime`              | 否   | 容器 CLI，`docker`（默认）或兼容的 `podman` |
| `image`                | 是   | 本机已有的 OCI 镜像；运行时使用 `--pull=never` |
| `image_env`            | 否   | 覆盖镜像名                                     |
| `external_env`         | 否   | 若该变量已设置，直接使用其值并跳过 Docker      |
| `inject_env`           | 是   | 注入测试步骤的变量名                           |
| `external_value_policy` | 否  | 外部值安全策略；测试 PostgreSQL 使用 `isolated-postgres` |
| `startup_timeout_secs` | 是   | 整个 Docker 启动过程的秒数，范围 1 到 300      |
| `timeout_env`          | 否   | 覆盖启动超时                                   |
| `container_port`       | 是   | 容器监听端口；宿主端口随机绑定到 `127.0.0.1`   |
| `environment`          | 否   | 传给容器的环境变量                             |
| `healthcheck`          | 是   | `docker exec` 后的参数列表，不能为空           |
| `connection`           | 是   | 注入值，必须包含 `{host_port}`                 |

服务按需启动，同一轮验证内复用。选定 runtime 的 daemon 探测、容器创建、端口查询和健康检查共享一个启动截止时间；验证成功、失败、超时或收到中断信号时都会在独立清理超时内尝试 `<runtime> rm --force`。镜像不会自动拉取，先用 `<runtime> pull <image>` 准备。

`isolated-postgres` 会要求 URL 使用 `postgres`/`postgresql` 协议，数据库名以 `_test` 或
`-test` 结尾，并拒绝与当前 `DATABASE_URL` 指向同一数据库。默认只允许本机回环地址；确需
使用已确认隔离的远程测试库时，额外设置 `ARC_FLOW_ALLOW_REMOTE_TEST_DATABASE=1`。

一个步骤可以依赖多个服务：

```toml
services = ["test-postgres", "test-redis"]
remove_env = ["DATABASE_URL", "REDIS_URL"]
```

每个 service 必须注入不同变量；`remove_env` 也不能删除 service 正在注入的变量。

跨步骤预检同样会在任何执行前运行：没有直接或传递 `depends_on` 顺序的两个步骤，不能引用同一
service，也不能引用两个向子进程注入相同 `inject_env` 的 service。为同一 service 建立明确
顺序时，将后续步骤依赖于前一个步骤：

```toml
[[steps]]
id = "api.integration"
# ...
services = ["test-postgres"]
depends_on = ["api.setup"]
```

不要依赖 profile、当前串行执行或偶然的启动顺序来规避这项规则；它们不能证明未来并行运行
时互斥。校验器不会自动加入依赖或重命名变量。

## 10. `[parsers.*]`

解析器用于防止命令退出码为 0、实际却没有执行任何测试：

```toml
[parsers.rust]
kind = "regex"
patterns = ['(?m)^running ([0-9]+) tests?$']
capture = 1
minimum = 1
```

| 字段       | 默认值 | 说明                          |
| ---------- | ------ | ----------------------------- |
| `kind`     | 无     | 当前支持 `regex`              |
| `patterns` | 无     | 一个或多个 Rust regex         |
| `capture`  | `1`    | 包含数值的 capture group 索引 |
| `minimum`  | `1`    | 所有匹配计数之和的最低值      |

每个正则都必须包含对应的 capture group。步骤成功后才解析日志；计数低于 `minimum` 时，该步骤改判为失败。

## 11. `[scope]` 和 `[[scope.rules]]`

```toml
[scope]
unmatched = "fail"
```

| `unmatched` 值 | 行为                                                       |
| -------------- | ---------------------------------------------------------- |
| `fail`         | 默认值；列出未命中的变更路径并失败                         |
| `all`          | 任一路径未命中时选择全部 component，适合通用或未知结构项目 |
| `ignore`       | 忽略未命中路径，仅适合已明确评估漏测风险的项目             |

`--all` 是显式全量模式，不读取工作区路径，因此不应用 `unmatched` 策略。

```toml
[[scope.rules]]
patterns = ["services/api/**", "shared/contracts/**"]
components = ["api"]

[[scope.rules]]
patterns = [".harness-gate/**", ".github/workflows/**"]
components = ["api", "web", "workflow"]
```

每条规则使用 glob 匹配仓库相对路径，命中后把所有 `components` 加入集合。规则可以重叠，最终 component 去重。每个 component 必须至少有一个步骤。`scope` 的文本和 JSON 输出都包含未匹配文件，便于排查规则覆盖缺口。

建议把共享契约、工作流配置和 CI 文件映射到所有受影响组件，避免只验证单端。

## 12. `[[steps]]`

```toml
[[steps]]
id = "api.tests"
label = "API tests"
component = "api"
profiles = ["full"]
program = "cargo"
args = ["test", "--manifest-path", "{api}/Cargo.toml", "--", "--nocapture"]
cwd = "{root}"
log = "api_tests.log"
timeout_secs = 300
timeout_env = "API_TEST_TIMEOUT"
parser = "rust"
services = ["test-postgres"]
remove_env = ["DATABASE_URL"]
```

Built-in safety gates may be declared explicitly with a closed `gate_type`
vocabulary. The current values are `secret-scan` and `architecture-audit`:

```toml
[[steps]]
kind = "builtin-gate"
id = "builtin.secret-scan"
label = "secret scan"
gate_type = "secret-scan"
profiles = ["full"]
```

When `kind` is omitted, the entry remains an external step for compatibility.
Built-in entries must use their reserved IDs and may not specify external-step
fields such as `program`, `args`, `services`, or `log`. Unknown gate types fail
closed before any command or service starts. If no built-in entries are
declared, verification synthesizes the legacy `secret scan -> architecture
audit -> external steps` chain internally without rewriting `flow.toml`.

| 字段           | 必需 | 说明                               |
| -------------- | ---- | ---------------------------------- |
| `id`           | 是   | 全局唯一步骤 ID                    |
| `label`        | 是   | 终端和报告显示名称                 |
| `component`    | 是   | 变更范围选择单位                   |
| `profiles`     | 是   | 该步骤参与的 profile，至少一个     |
| `program`      | 是   | PATH 中的裸命令名                  |
| `args`         | 是   | 独立参数数组，可使用路径占位符     |
| `cwd`          | 是   | 单个 `{root}` 或 path alias 占位符 |
| `log`          | 是   | 报告目录下的单个 `.log` 文件名     |
| `timeout_secs` | 是   | 运行超时，范围 1 到 3600 秒        |
| `timeout_env`  | 否   | 覆盖运行超时的环境变量             |
| `parser`       | 否   | 成功后使用的 parser ID             |
| `services`     | 否   | 运行前需要准备的 service ID 列表   |
| `remove_env`   | 否   | 创建子进程前删除的继承环境变量     |
| `depends_on`   | 否   | 必须先完成的 step ID；支持传递依赖并决定资源是否可并发 |
| `kind`         | 否   | `builtin-gate` 或省略（省略表示外部步骤） |
| `gate_type`    | 否   | 内置门禁类型：`secret-scan` 或 `architecture-audit` |

命令直接通过 `program + args[]` 启动，不执行 shell 拼接。`sh -c`、`bash -lc` 等命令字符串会被配置校验拒绝；管道、重定向和条件逻辑应拆成多个步骤，或封装成项目内受版本控制的可执行程序。

步骤选择条件是：component 已被 scope 选中，并且步骤包含当前 profile。配置顺序就是执行顺序；任一步失败后，报告判为失败，但仍继续执行不依赖该故障 service 的后续步骤。同一 service 启动失败会被缓存，依赖它的步骤快速失败，不会反复等待启动超时。

## 13. Secret Scan 规则文件

`[paths].secrets_config` 指向独立、受版本控制的 TOML 文件。预设会生成一套通用高置信规则，项目可在不重新编译 `harness-gate` 的情况下增加供应商或业务密钥规则。配置版本、规则 ID、正则、捕获组和最小长度都会在扫描前校验；配置缺失、空规则或无效捕获组会直接让门禁失败。

```toml
version = 2

[placeholders]
minimum_unique_characters = 4
maximum_nonalphanumeric_characters = 2
prefixes = ["${", "{{", "<"]
markers = ["change-me", "replace-me", "placeholder"]
exact = ["password", "secret"]

[[rules]]
id = "named-signing-secret"
kind = "value"
pattern = '''(?i)signing_secret\s*=\s*([A-Za-z0-9_-]{12,})'''
capture = 1
minimum_length = 12
```

规则类型：

- `direct`：正则命中即报告，适合有固定前缀或结构的 Token、JWT、私钥头。
- `value`：只对指定捕获组执行长度、占位符和字符多样性判断，适合命名密钥及厂商 Webhook Token。
- `postgres-url`：分别捕获用户名、密码、主机和数据库；`local_test_policy` 可显式配置临时数据库允许的主机、库名后缀及用户名密码约束。
- `webhook-url`：解析捕获到的 URL，检查配置的敏感查询参数或高信息路径末段。

扫描报告只包含命中文件名，不会复制密钥内容。占位符策略只作用于捕获值；`direct` 应仅配置误报概率足够低的模式。

## 14. 审计规则文件

`[paths].audit_config` 指向独立 TOML 文件。空规则文件可写为：

```toml
version = 2

[engine]
ignore_filename = ".auditignore"
json_report_filename = "review_context.json"
markdown_report_filename = "review_context.md"
markdown_max_bytes = 4096
markdown_occurrences_per_rule = 3

[engine.comment_syntax.rs]
line = ["//"]
block = [{ start = "/*", end = "*/", nested = true }]
strings = [
  { start = 'r###"', end = '"###' },
  { start = 'r##"', end = '"##' },
  { start = 'r#"', end = '"#' },
  { start = 'r"', end = '"' },
  { start = '"', end = '"', escape = '\' },
]

[paths]
exclude = ["target", "node_modules", "dist", ".git"]
```

内置空 preset 还预置 `sql`、`ts`、`tsx`、`js`、`jsx`、`toml`、`yaml` 和 `yml` 的注释与字符串定界符。每条规则使用的扩展名都必须存在对应的 `[engine.comment_syntax.<扩展名>]`；缺少时 auditor 会拒绝运行，避免把注释示例当成真实代码。

<a id="audit-v2-migration"></a>

### Audit v2 迁移

audit v2 是 `harness-gate` 3.0.0 的破坏性配置升级。旧配置不会被静默套用新语义：缺少 `version`、缺少 `[engine]`、未知版本、未知字段或字符串 allowlist 都会 fail closed，并在错误中指向本节。

旧配置可能依赖隐式 engine 默认值，并让字符串内容同时承担路径和正则语义：

```toml
[[hard_rules]]
name = "SQL writes stay in repositories"
severity = "blocker"
paths = ["api"]
extensions = ["rs"]
patterns = ['(?i)INSERT\s+INTO']
allowlist = ["services/api/src/repositories", "^services/api/generated/.*\.rs$"]
```

迁移时先加入 `version = 2`，从当前 `empty.audit.toml` 复制完整 `[engine]` 和所需扩展名的 `comment_syntax`，再逐项明确 allowlist 类型：

```toml
version = 2

[engine]
ignore_filename = ".auditignore"
json_report_filename = "review_context.json"
markdown_report_filename = "review_context.md"
markdown_max_bytes = 4096
markdown_occurrences_per_rule = 3

[engine.comment_syntax.rs]
line = ["//"]
block = [{ start = "/*", end = "*/", nested = true }]
strings = [
  { start = 'r###"', end = '"###' },
  { start = 'r##"', end = '"##' },
  { start = 'r#"', end = '"#' },
  { start = 'r"', end = '"' },
  { start = '"', end = '"', escape = '\' },
]

[[hard_rules]]
name = "SQL writes stay in repositories"
severity = "blocker"
paths = ["api"]
extensions = ["rs"]
patterns = ['(?i)INSERT\s+INTO']
allowlist = [
  { kind = "path-prefix", path = "services/api/src/repositories" },
  { kind = "regex", pattern = '^services/api/generated/.*\.rs$' },
]
```

字符串 allowlist 无法可靠推断原意，因此不自动迁移。完成转换后运行 `harness-gate config check` 和 `harness-gate audit`，确认路径引用、正则和报告配置有效。

### 13.1 Hard rule

```toml
[[hard_rules]]
name = "SQL writes stay in repositories"
severity = "blocker"
paths = ["api"]
extensions = ["rs", "sql"]
patterns = ['(?i)INSERT\s+INTO', '\.execute\s*\(']
allowlist = [
  { kind = "path-prefix", path = "services/api/src/repositories" },
  { kind = "path-prefix", path = "services/api/migrations" },
  { kind = "path-prefix", path = "services/api/tests" },
]
exclude_patterns = []
```

`paths` 可以引用审计文件 `[paths]` 中的 alias。`allowlist` 必须显式使用 `path-prefix` 或 `regex` 类型，避免根据字符串内容猜测语义。`exclude_patterns` 用正则排除文件路径。

### 13.2 Architecture rule

```toml
[[arch_rules]]
name = "handlers do not query SQL"
layer = "handler"
paths = ["services/api/src/handlers"]
extensions = ["rs"]
forbidden_patterns = ['sqlx::(query|query_as|query_scalar)!?\\s*\\(']
allowed_patterns = []
suggestion = "move SQL into a repository"
allowlist = []
exclude_patterns = []
```

`allowed_patterns` 匹配违规起始行，适合明确的 trait impl 或框架样板；不要用过宽正则隐藏真实违规。`exclude_patterns` 与 hard rule 一样匹配文件路径。任意审计违规都会阻止后续外部步骤。

每个规则的 `paths` 必须解析到项目根内已经存在的目录。路径中的 `..`、逃出项目的绝对路径或符号链接都会被拒绝；目录遍历或文件读取失败也会让审计失败，避免扫描缺失时误报通过。审计报告统一记录仓库相对路径。

auditor 是确定性的整文件正则扫描器，不是语言 parser。正则默认启用 multi-line 模式，因此 `^`/`$` 仍按代码行匹配，`\s` 可以跨行；需要让 `.` 跨行时应在规则中显式使用 `(?s)`。报告定位到匹配起始行。`[engine.comment_syntax.<扩展名>]` 可配置行注释、块注释和字符串定界符；扫描器会跟踪这些词法状态，避免把字符串中的注释标记当成真实注释。需要抽象语法树级判断时，应使用项目语言自己的 lint 工具，并把该工具配置成一个 step。

## 15. 最小完整示例

```toml
version = 2

[project]
name = "example-api"
default_profile = "full"
hook_profile = "hook"

[paths]
reports = ".harness-gate/reports"
audit_config = ".harness-gate/audit.toml"
secrets_config = ".harness-gate/secrets.toml"

[paths.aliases.app]
path = "."

[policy]
required_steps = ["app.format", "app.tests"]

[scope]
unmatched = "all"

[[doctor.checks]]
id = "tool.cargo"
label = "cargo"
kind = "command"
program = "cargo"
args = ["--version"]

[parsers.rust]
kind = "regex"
patterns = ['(?m)^running ([0-9]+) tests?$']
capture = 1
minimum = 1

[[scope.rules]]
patterns = ["**"]
components = ["app"]

[[steps]]
id = "app.format"
label = "Rust format"
component = "app"
profiles = ["full", "hook"]
program = "cargo"
args = ["fmt", "--", "--check"]
cwd = "{app}"
log = "rust_fmt.log"
timeout_secs = 120

[[steps]]
id = "app.tests"
label = "Rust tests"
component = "app"
profiles = ["full"]
program = "cargo"
args = ["test", "--", "--nocapture"]
cwd = "{app}"
log = "rust_tests.log"
timeout_secs = 300
parser = "rust"
```

完成配置后依次执行：

```bash
harness-gate config check
harness-gate doctor
harness-gate scope --all
harness-gate verify --all
```

### 步骤日志唯一性

每个 `log` 都必须是唯一的单个 `.log` 文件名。即使两个步骤已有 `depends_on` 顺序，也不能复用
同一日志：串行复用同样会覆盖报告证据。错误诊断会同时指出两个 `steps[*].log` 字段。

### 报告模板、JUnit 和通知

HTML 模板和 JUnit 输出是可选的；默认 JSON/Markdown 文件保持不变：

```toml
[report_templates]
root = "templates/harness-gate"
template = "templates/harness-gate/verification.tera"
junit = "junit.xml"
```

`root` 和 `template` 必须同时存在；二者必须是仓库内路径，不能是绝对路径、Windows 前缀、
`..` 跳转或 NUL。模板根必须存在且为目录，模板必须存在且为 `.html` 或 `.tera` 普通文件；
解析符号链接后仍必须在仓库和模板根内。模板根不能等于、包含或被 `paths.reports` 包含。
模板路径相对于仓库根，模板支持 `{{ timestamp }}`、`{{ profile }}`、`{{ summary }}` 和
`{{ components }}` 替换，也可使用 Tera 的 `include`、`extends` 和 `block`。模板上下文同时提供
完整的 `report` 对象及其 `timestamp`、`profile`、`scope`、`steps` 和 `passed` 字段。输出固定为报告目录中的 `test_result.html`。`junit` 路径相对于报告目录，
必须是报告目录内的 `.xml` 文件名或子路径；运行时还会检查现有目录和符号链接不会把输出重定向到目录外。
模板输入仍是只读且受路径隔离校验。

### 容器运行时和 Webhook

Docker service 可显式选择 Docker 兼容的 Podman CLI，省略时默认为 Docker：

```toml
[services.postgres]
kind = "docker"
runtime = "podman" # docker | podman
```

Webhook 在报告写入成功后发送报告 JSON。仅启用的结果类型会发送；URL 建议使用环境变量插值：

```toml
[[notifications.webhooks]]
url = "${HARNESS_GATE_WEBHOOK_URL}"
on_failure = true
on_success = false
```

Webhook 只支持 `http` 和 `https`，非 2xx 响应或连接错误会使本次验证返回报告错误（`E1404`）。
`on_failure` 默认开启，`on_success` 默认关闭；至少启用一个结果类型。通知失败不会改写已经生成的报告。
多个 webhook 按配置文件中的顺序发送；第一个请求失败后立即停止，后续 endpoint 不会被调用。

### v1 到 v2 配置迁移和安全修复

v1 配置使用迁移命令生成 v2 文件，源文件不会被删除：

```bash
harness-gate --project-root /path/to/project config migrate \
  --input legacy.flow.toml \
  --output .harness-gate/flow.toml
harness-gate --project-root /path/to/project config check
```

v2 版本号保持为 `2`，不会自动升级。若原本可加载的 v2 文件因为服务、注入变量或日志关系被
新预检拒绝，请根据 `config check` 的主字段、related 字段和 `help:` 修复：添加显式的
`depends_on`、使用不同的 `inject_env` 或 service、或为每个步骤指定独立日志。工具绝不会
静默重排、插入依赖或改写配置。
