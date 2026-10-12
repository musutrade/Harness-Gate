# 正式安装与采集验收流程

本方案依据仓库现有脚本与契约编写，状态为待执行。正式验收必须绑定指定发布版本、宿主输入和目标平台；离线测试通过不能替代真实签名、安装和项目采集证据。

2026-09-12 已按“最新版本、本机、Arc Admin”执行 Core 安装和项目导入预检，见 [本机执行记录](installation-acceptance-20260912.zh-CN.md)。完整采集验收仍待正式采集器发布及项目集成。

## 输入清单

| 输入 | 来源与约束 |
| --- | --- |
| `VERSION` | 发布负责人指定的精确 `v` 前缀 SemVer；不得使用 `latest` |
| 发布来源 | `musutrade/Harness-Gate` 的对应不可变 tag、提交、成功 CI 和 release run；按 [发布治理](release-governance.md) 留存 |
| 安装器 | 从已核对提交取得的 `install.sh`，记录其 SHA-256 |
| `INSTALL_DIR` | 独立、可写绝对路径；不得包含符号链接、`.`、`..`，不得允许组或其他用户写入 |
| 运行环境 | Bash、curl 或 wget、cosign、SHA-256 工具；记录 OS、架构、工具版本。Windows 使用受支持的 Bash 环境，不支持 Windows arm64 |
| `PROJECT_ROOT` | 已确定 commit 的项目真实绝对路径，含 flow、quality 和引用的 policy 配置 |
| `STATE` | 宿主生成的 `quality-trusted-state/v1`，绑定 profile、commit/base/target/run、配置摘要、subjects、series；详见 [编译契约](quality-compilation.md) |
| `KEYS` | 宿主维护的 JSON 公钥数组，元素为 `key_id` 和 base64 Ed25519 `public_key`；不得从采集器输出导入 |
| 请求与采集器 | 配置引用的完整已签名 adapter v2 请求、固定可执行文件及其摘要；签名覆盖的路径须为解析后的真实路径 |
| 产物与报告目录 | 采集前 artifact root 已存在且为空，state 的 artifacts 为空；报告输出位于 artifact root 之外且不覆盖输入 |
| 完整验证输入 | profile 配置的 workflow state、trusted_keys，以及策略需要时的 baseline_request；由宿主准备 |

发布包签名使用 GitHub Actions OIDC/Sigstore；采集请求签名使用宿主 Ed25519 密钥。这两套身份不能互相替代。正式私钥不得放入验收证据或使用测试 fixture 密钥替代。

独立 Rust collector 还有自身的发布包信任链，涉及 RSA 发布公钥、Sigstore 身份和宿主 ABI 信任文件；Core 的 `install.sh` 不安装该包。具体接口应依据所选采集器发布源码中的 `tools/release/production_installer.py` 和配套契约核对，不得拿 Core 的版本或签名当作采集器正式发布证明。

## 签名与状态的准备顺序

1. 固定项目源码、配置、pack/tool/runtime 版本、profile/scope 和本次 run；创建独立空 artifact root。
2. 宿主解析 subjects、series、selection 和 collector bindings，构造 [采集请求](quality-collectors.md) 的完整 input。
3. 按 adapter 签名协议生成新鲜时间、唯一 nonce、可执行文件摘要、能力和限制，并签名。`step_id` 必须匹配 binding ID，`invocation_id` 匹配可信 run。
4. 将签名请求写入配置指定路径，再计算完整请求字节的摘要，写入 state 的 `config_files`。binding 的 `config_digest` 按契约排除请求路径，避免签名自引用。
5. 发布宿主公钥白名单，执行 compile 检查。每次改源码、配置、路径、工具或运行上下文，都重新准备相关绑定与签名。

现有 `install.sh` 不生成采集请求；`collector_runner.py` 是非权威 Python 参考实现。仓库 fixture 的签名逻辑不能当作正式密钥管理入口。执行前须在验收记录填写宿主签名工具的实际命令、版本、负责人及输入输出路径；缺失时停止在此阶段，不将方案标为已完成端到端验收。

## 安装与采集命令

以下为 Linux/macOS Bash 模板，在仓库根目录执行。先设置清单中的变量；`EVIDENCE_DIR` 必须是新的绝对目录，位于项目和 artifact root 外。Windows 的安装后二进制名改为 `harness-gate.exe`。

```bash
set -Eeuo pipefail
: "${VERSION:?指定正式版本}"
: "${INSTALL_DIR:?指定独立安装目录}"
: "${PROJECT_ROOT:?指定项目真实绝对路径}"
: "${STATE:?指定宿主可信状态绝对路径}"
: "${KEYS:?指定宿主公钥文件绝对路径}"
: "${EVIDENCE_DIR:?指定新的证据目录绝对路径}"
[[ "$EVIDENCE_DIR" = /* ]]
mkdir -m 700 "$EVIDENCE_DIR"
git rev-parse HEAD > "$EVIDENCE_DIR/installer-source-commit.txt"
git -C "$PROJECT_ROOT" rev-parse HEAD > "$EVIDENCE_DIR/project-commit.txt"
git -C "$PROJECT_ROOT" status --porcelain > "$EVIDENCE_DIR/project-status.txt"
uname -a > "$EVIDENCE_DIR/platform.txt"
cosign version > "$EVIDENCE_DIR/cosign-version.txt" 2>&1
python3 -c 'import hashlib; print(hashlib.sha256(open("install.sh", "rb").read()).hexdigest())' \
  > "$EVIDENCE_DIR/installer.sha256"
bash install.sh --version "$VERSION" --install-dir "$INSTALL_DIR" \
  2>&1 | tee "$EVIDENCE_DIR/install.log"
HG="$INSTALL_DIR/harness-gate"
"$HG" --version | tee "$EVIDENCE_DIR/installed-version.txt"
"$HG" quality compile --repository-root "$PROJECT_ROOT" --state "$STATE" \
  --output "$EVIDENCE_DIR/compiled.json" 2>&1 | tee "$EVIDENCE_DIR/compile.log"
"$HG" quality collect --repository-root "$PROJECT_ROOT" --state "$STATE" \
  --trusted-keys "$KEYS" --output "$EVIDENCE_DIR/collection.json" \
  2>&1 | tee "$EVIDENCE_DIR/collect.log"
```

安装器核验 SHA256SUMS 和平台二进制的签名、证书身份与摘要后原子替换目标文件。核对日志中的身份绑定精确 tag 和 `.github/workflows/release.yml`，核对安装版本与选定版本一致。完整资产清单、SBOM、证明和 release run 证据另外按发布治理归档；安装日志本身不覆盖全部发布资产验收。

`quality-collection/v1` 仅表示测量完成。完整验收随后按 [验证组合](quality-verification.md) 配置 workflow 输入，在项目目录执行已安装二进制的 `verify --profile <已配置的完整配置档> --all`。此前 collect 已消费 nonce 并写入产物，因此 verify 必须使用新的 run、签名请求和空产物目录，或严格按已配置的认证 retained 流程复用。不得直接重放请求、删除 nonce 存储或把 collection 文件冒充 evaluate 所需 evidence 数组。

## 负例与判定

所有负例使用一次性项目副本和独立安装目录，逐例记录变更、命令、退出码、日志与输出是否存在。签名后的请求改动若要进入特定下游检查，应先更新宿主 state 的请求摘要，否则只能证明较早的摘要检查阻断。

| 场景 | 期望 |
| --- | --- |
| 发布二进制被篡改、签名失败、错误 tag 身份、缺少资产 | 安装非零退出，已有目标二进制字节保持不变 |
| 安装目标或父目录为符号链接、目录可被他人写入 | 安装拒绝 |
| 不在公钥白名单、签名后请求被修改、可执行文件摘要不符 | 采集拒绝，不发布可用 collection |
| 已使用 nonce 再次执行、过期请求 | 拒绝；重放用例先恢复空产物根并保留 nonce 存储，避免仅触发产物检查 |
| 采集器失败、遗漏或额外产物、产物摘要错误 | 非零退出，无部分或旧的成功输出可复用 |
| 配置/source/context/series 与可信输入不一致 | compile/collect/verify 在相应边界阻断 |
| 必需 baseline 或 workflow state 缺失 | 完整 verify 阻断，不降级成传统步骤通过 |
| 测量成功但必需策略阈值失败 | collection 可成功，完整 verify 必须失败 |

现有离线安装负例可直接执行：

```bash
bash tools/release/tests/test_install.sh
```

该脚本模拟下载器、cosign 和构建工具，覆盖篡改、签名、身份、路径、缺失资产、权限与中断等安装边界；不能证明真实 OIDC 证书有效或项目采集负例已通过。

## 审阅交付物与完成条件

提交版本/提交/run 对照表、工具版本、安装与采集日志、宿主输入与请求摘要、公钥标识、产物清单及摘要、完整 verify 的 invocation 报告，以及每个负例的独立结果。受保护的宿主输入按项目访问策略留存，私钥不归档。

只有真实安装成功、签名身份正确、源与配置绑定一致、完整报告顶层 `status`、`passed`、`evidence_complete` 及完整质量状态均符合契约、全部必需负例符合预期，才可判定通过。待提供的版本、宿主签名入口、密钥、项目状态或 baseline 必须逐项标记“待提供”；未运行项标记“未执行”，不能用 fixture PASS 补齐。
