# 本机 Arc Admin 安装验收记录（2026-09-12）

用户选择最新版本、本机、Arc Admin，其余输入由环境核查。已完成正式 Core 安装及离线配置导入预检；完整采集与项目验收未完成。

## 已确定输入与结果

| 项目 | 实际值或结果 |
| --- | --- |
| 最新正式 Core | GitHub releases/latest 返回 `v0.4.0`，https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.0 |
| tag 解引用提交 | `ee544690662645806cda7dd4cd2c9192566f7929`，本地 tag 与 GitHub API 一致 |
| 安装器 | 从该 tag 的 `install.sh` 导出执行 |
| 机器 | Linux x86_64 |
| 安装二进制 | `/home/gem/.local/share/harness-gate/versions/v0.4.0/bin/harness-gate` |
| 二进制 SHA-256 | `8e3df8303ca8f650d4ef768b29cfefb60ca115122a47b116245cc19e4649bbdd` |
| 版本命令 | `harness-gate 0.4.0` |
| 签名验证 | SHA256SUMS 与平台二进制均 `Verified OK`，安装退出码 0 |
| 验签工具 | 本机已有 cosign v3.1.3，其可执行文件摘要与官方 release digest 一致：`4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71` |
| Arc Admin | `/home/gem/arc-admin`，提交 `2faa1ca6c1a2b1a45956540e97e68a01532a40f7` |
| 项目工作区 | 已跟踪文件无改动；已有未跟踪 `.angular/` 未修改 |
| 配置检查 | 退出码 1 / `E1000`：缺少 `.harness-gate/flow.toml` |
| 执行配置导入 | 在证据目录内复制原始 `.arc-flow/flow.toml` 后执行 `config import --execution-only`，退出码 0；明确返回 `Authority transfer BLOCKED` |

安装日志包含一次 Sigstore TUF trusted_root 获取失败告警；cosign 随后继续使用 individual targets，并对两个对象验证成功。没有使用跳过签名、跳过透明日志或忽略身份参数。网络下载中的重试日志保留。

本次安装在独立版本目录。原 PATH 中 `/home/gem/.cargo/bin/harness-gate` 仍报告 `0.1.0`；后续命令必须使用上述新二进制绝对路径，避免误用旧版。

## 已找到的宿主材料

`/mnt/dev-ssd/symphony-inputs/gh239/production-trust/` 已有采集器发布公钥、`trust-v2.json`、ABI 记录与 key-provisioning 回执。GitHub 环境 secret 元数据确认 `rust-collector-release` 中存在 `COLLECTOR_RSA_PRIVATE_KEY`，创建和更新时间均为 2026-09-11T14:55:11Z。本次仅查询元数据，未读取或生成私钥，未签名或发布候选包。

这些材料属于采集器发行信任，不等于 Arc Admin 的 Ed25519 请求签名、可信 state、baseline 或最终兼容性批准。

## 当前阻断与后续顺序

1. 查询最近发布后，独立采集器仅有 `rust-collector-materials-6b4fbe9-v1` 预发布。其说明明确为 unsigned candidate 的源码与 notices 材料，非批准的二进制发布。需先完成真实采集器候选包的正式签名、兼容性与发布验收；本次没有可作为“最新正式采集器”的包可安装。
2. 已导出的 Arc Admin 执行配置仍要求核对环境覆盖、secret/audit 配置、hook/prelude、服务生命周期及运行结果；导入成功不建立运行等价性。不能直接用 generic preset 替换项目既有门禁。
3. 正式采集器可用后，绑定安装路径与真实项目 source/config/tool/runtime，建立项目质量配置、宿主请求签名入口、公钥白名单和可信 state；需要基线的策略还须准备认证 baseline。
4. 执行新鲜采集、完整 verify 和独立负例，归档 invocation 报告，再判定完整验收结果。此前离线安装测试结果不能替代这些未执行项。

上述缺口是发布与集成工作，不要求用户手工编造 state、提供私钥或选择测试密钥。此记录不批准远端发布或门禁迁移。

## 本地证据

原始证据目录：`/home/gem/Harness-Gate/target/installation-acceptance-20260912/`。

- `core-release.json`：最新正式发布 API 返回值。
- `install-v0.4.0.sh`、`install.log`：固定 tag 安装器与真实安装日志。
- `arc-admin-config-check.log`：项目缺配置的真实诊断。
- `arc-admin-import.log`、`arc-admin-import/.harness-gate/flow.toml`、`flow.import.json`：本机当前配置的导入与限制报告；未写入 Arc Admin。

复核已安装版本：

```bash
/home/gem/.local/share/harness-gate/versions/v0.4.0/bin/harness-gate --version
```
