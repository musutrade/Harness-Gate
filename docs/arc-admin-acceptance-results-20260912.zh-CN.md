# Arc Admin 本机验收结果

本机执行与测量验证已完成；正式采集器安装及完整质量门禁接入尚未通过。修复代码在 [PR #244](https://github.com/musutrade/Harness-Gate/pull/244)，验收时 Core 保持正式 v0.4.0。修复提交 `ed834e1` 的必需 CI 汇总已通过，PR 已转为可审阅，尚未合并；Generic Quality Shadow 也已通过。

| 检查 | 实际结果 |
| --- | --- |
| 原项目门禁 | 全部 25 个执行步骤通过，secret/audit 也通过；含后端测试、85 个前端测试、浏览器 E2E、真实全栈冒烟 |
| 必需步骤失败负例 | 注入失败命令后，整体 FAIL，未吞掉失败 |
| 真实原生采集 | 插桩编译完成；API 1、OpenAPI 2、permission-template 5 个测试通过 |
| 测量完整性 | 1,778 个函数；生产目标映射完整；行覆盖 5990/8025、区域覆盖 18155/46315、函数覆盖 1124/1778 |
| 通用证据投影 | 全量真实函数数据通过来源/摘要/类型验证；产物 5,731,679 字节、响应 7,445,940 字节 |
| 正式 Core 读取真实证据 | 两个数值验收对照分别得到 pass/fail，整体按预期 fail；不是项目质量策略的通过声明 |
| 真实产物篡改负例 | 修改产物副本一个字节后，Core 非零退出并生成 measurement_error 报告，覆盖预置的旧 PASS |
| 修复后回归 | runtime 12、transport 7、delivery 4、production helper 12、lifecycle 21 项通过 |
| 隔离运行 | 无系统 Python/C 编译器、禁止网络的容器完成私有 C 编译/归档/OpenSSL 链接，以及新鲜 Rust 捕获和两次一致重导出 |
| 正式安装 | 实际执行后被拒绝：候选包缺 provenance.json、release-inventory.json、release-inventory.sig |

本次修复两个实际缺陷：

1. 私有工具包遗漏 `cc1` 等 C 编译前提，导致 Arc Admin 的 `ring` 依赖无法构建。补齐工具、包归属明确的头文件及依赖后复测通过。
2. 每个函数重复附带完整原生报告，会产生约 79.24 GB 产物。改为每个函数保存自身事实，并绑定完整报告摘要、capture anchor 和测量身份。没有放宽 Core 的 16 MiB stdout 或 64 MiB 产物预算，也没有修改质量阈值。

原 Arc Admin 工作区未改动；导入配置、故障注入、编译及测试在隔离 worktree 中执行。一次性 PostgreSQL 和容器已清理，原始日志、捕获、修复后的 runtime 仍保留。

完整 machine receipt（含 23 项证据文件的 SHA-256）：
`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/acceptance.json`。

修复工作区：`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/harness-gate`。

还不能将上述结果称为完整正式验收通过：本次现有项目执行报告的 `quality` 为空；本机原生测量与数值对照没有替代生产签名的安装、项目 workflow state、认证 baseline 或正式质量决策。这些界限在 receipt 中分别记录，未用测试签名、虚构基线或临时放宽门禁补齐。

架构补充：Rust 专属编译与测量工具在独立采集器；Core 与项目执行配置分开。当前 Core 没有通过 Cargo `[features]` 对这些能力做可选编译开关拆分。

## Rust 插件发布复核

以修复后的私有 runtime 和正式 Core v0.4.0 复跑 9 项原生驱动测试，全部通过且无跳过，涵盖宏生成、泛型实例、闭包、async、cfg 条件编译、生产/测试排除、篡改拒绝和正式 Core 集成。另复跑 11 项采集器交付及测量契约测试，全部通过。

Rust 采集器已有独立版本、进程协议和安装切换能力；这不等于正式签名发行及项目端安装验收已完成。发布目标应为独立采集器 RC，而非仅更新 Core 版本。GitHub 当前配置为 dry-run 发布 workflow；生产模板要求专用 runner（仓库当前注册数为 0），并要求与待发布源提交及产物摘要绑定的审核材料。生产发布尚未执行。

## 本机 runner 已接入（后续更新）

已安装并校验官方 runner v2.337.0，本机以 `harness-gate-local-release` 加入组织现有专用组（group 4，runner 10）。GitHub 返回 online，服务日志为 Listening for Jobs；主机 ABI 与之前验收记录完全一致。该组仅允许本仓库主分支的采集器发布 workflow；runner 为单次任务模式，执行一次后自动注销。组织 runner 不计入仓库级注册 runner 列表。

服务为 `systemctl --user status harness-gate-release-runner.service`，可用 `systemctl --user stop harness-gate-release-runner.service` 停止监听；未设置开机自动启动。本次已验证注册、连接与监听，尚未执行真实 workflow 任务或签名发布。机器回执：`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/runner-ready.json`。runner 这一项已就绪，无需租赁服务器。


## 2026-09-12 07:25 UTC：项目配置与插件包分离修复

权限与本机 runner 已就绪。私有签名阶段修复已合并 PR #245。真实 Arc 重采集发现安装路径变化会改变编译器函数 owner 标识，因此原 v1 清单固定调用方配置的设计无法复用。v2 清单将配置留在 Core 认证的主机状态中，保留工具、代码、ABI、采集身份和 v1 严格校验。修复 PR：https://github.com/musutrade/Harness-Gate/pull/246 ，当前 CI 运行中。

24 项契约/交付/投影测试、60 项发布测试和文档一致性通过。真实运行测试首轮 11 项通过，另 1 项因操作时遗漏离线依赖目录报错，补齐输入后单独重跑通过。原始失败日志已保留，未改写为成功。详细收据：`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/plugin-v2-verification/validation-receipt.json`。

这些结果属于开发验证；新插件尚未正式签名、发布或完成签名安装验收。


## 签名安装与真实 Core 接入验收通过

PR #246 已合并，main CI 第二轮通过。签名任务 https://github.com/musutrade/Harness-Gate/actions/runs/34686097402 成功。候选插件 `0.1.0-rc.1` 的本机安装、无系统 Python 的隔离容器安装、12 项运行契约测试、Arc Admin 真实采集及两次一致导出均通过。Core 已认证并接收 1,778 个函数证据，数值控制 pass/fail 及重复采集拒绝均符合预期。

验收脚本的硬链接和额外 Python 缓存问题均已修正，初次失败日志保留；签名包未修改。机器可读收据：`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/plugin-v2-verification/technical-acceptance.json`。

当前仅技术验收完成，公开 GitHub Release 尚未执行；最终发布收据链接、保留引用和完整依赖许可证审阅记录仍需补齐。不代表 Arc Admin 已采用质量政策或历史基线。

## 2026-09-12：发布材料补齐，最终发布等待环境审批

上述缺项现已补齐。六类真实验收收据、对应源码与许可证通知、用户重构建/重链接材料、安装器和保留记录已公开在不可变材料发布：https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-materials-7165558-v1 。18 个附件的 GitHub 端 SHA-256 和大小均与本地一致，六类收据的公开地址已无认证下载并校验。

逐文件工程许可证审阅覆盖 2,684 个插件文件及 695 个安装器文件，实际审阅者记为 Codex，未代填用户或独立人工审阅身份。最终审阅记录在独立不可变发布：https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-review-7165558-v1 ，8 个附件上传后均核对服务端摘要。原材料发布不可追加，故审阅记录使用独立标签，没有覆盖任何版本或资产。

重复安装拒绝、未知版本回滚拒绝、失败后恢复和缺失 bootstrap 拒绝均实跑通过。按锁定输入重新组装的三个未签名资产与已验收候选逐字节一致。完整生产发布预检通过，审批包 SHA-256 为 `86833481f8e28d8af05880f92e0d20e40e264b304943b07cf07a9364eacb1c37`，源提交为 `7165558f5e9f5ca5a0de8c9cd33a11d733909f48`。

正式插件发布任务：https://github.com/musutrade/Harness-Gate/actions/runs/34692373716 ，当前等待 `rust-collector-release` 环境审批。此前审批属于私有签名任务；本次最终发布是新的受保护任务。使用现有授权令牌提交本次审批，GitHub 返回 HTTP 403 `Resource not accessible by personal access token`，需要获授权用户在该运行页面点击 Review deployments。尚不能声称插件二进制已公开发布。

本机单次 runner（id 11）已在线，后台 `harness-gate-final-publication-monitor.service` 已启动：审批后跟踪工作流，在草稿上传期间补入源码/通知链接，完成后核对六资产及其与验收候选的一致性并保存结果。最终记录目录：`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/plugin-v2-verification/publication-materials`。本次没有修改源码、放宽门禁或替项目采用质量政策/历史基线。


## 2026-09-12：代批权限已生效

用户补充令牌 Deployments 写权限后，本次发布任务 34692373716 的环境审批 API 成功，deployment id 为 6409350085。本机 runner 已完成源码检出、锁定输入重组装、现有标签一致性核对和真实 RSA/Sigstore 签名，开始上传六个正式资产。签名后的三个内容资产与此前 Arc Admin 已验收候选逐字节一致，回执为 `publication-materials/final-signed-payload-equivalence.json`。最终公开与回下载验证结果另行记录。


## 2026-09-12：发布下载网络故障恢复

发布运行 34692373716 在 13:18:33 UTC 回下载时收到 HTTP/2 `stream ID 7; PROTOCOL_ERROR; received from peer`，最终状态 failure。构建、批准、标签核对、RSA/Sigstore 签名及全部六资产上传已成功，二进制仍为草稿。原始失败日志和环境审批记录已保存。

操作侧从现有已签名草稿继续恢复，使用用户指定的 `192.168.0.34:7890`、HTTP/1.1、分段重试及断点续传。未修改源提交、版本、签名资产、审批包或生产校验器；草稿回下载及公开后回下载仍分别调用原 `production_collector_release.py verify`。只有草稿完整校验通过才公开。原工作流失败状态保留，不将操作侧恢复称为工作流重跑成功。结果目录：`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/plugin-v2-verification/publication-recovery`，后台服务 `harness-gate-publication-recovery.service`。


## 2026-09-12 14:08 UTC：发布及最终回下载校验完成

独立 Rust 插件 `0.1.0-rc.1` 已于 13:43:56 UTC 公开发布：https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-v0.1.0-rc.1 。草稿及公开后两次独立回下载均调用原生产校验器通过，六资产大小、SHA-256 和真实 RSA/Sigstore 签名验证完成；最终通过时间为 14:08:13 UTC，恢复服务正常退出。发行内容与已完成本机/隔离容器安装、Arc Admin 真实采集及 Core 认证接入验收的候选一致。

最终机器回执：`/mnt/dev-ssd/workspaces/harness-gate-acceptance-20260912/plugin-v2-verification/publication-recovery/recovery-result.json`；公开回下载校验：同目录 `published-verification.json`。原 Actions 运行 34692373716 因 HTTP/2 下载中断保持 failure；本次成功属于已审批签名后的操作侧恢复，未将原运行改称成功。发布与本次约定的安装/采集验收已完成，不代表 Arc Admin 已采用项目质量政策或历史基线。


## 2026-09-12：原发布流程优化与新安装入口验证

PR #247、#248 已合并，最终源提交为 `62313a251f1862707a887480cbf593130da2ba4a`。继续使用原 Core `release.yml`（包含 crates.io 发布）及受保护的 Rust collector 工作流；安装器采用独立新标签 `rust-collector-installer-v0.1.0-rc.2`，承载原有已签名插件 `0.1.0-rc.1`，没有覆盖旧版本。Core 新版本为 `0.4.1`。此段记录本机验证，公开发布结果待后续回执。

原 `install.sh` 默认仅安装 Core，新增 `--with-rust`、`--rust-only`、缓存目录和 Rust 离线安装参数。工具链与插件分别压缩、按摘要缓存，重建原始归档后仍执行真实 RSA/Sigstore 验证。原 1,061,498,880 字节插件归档的传输层合计为 282,243,324 字节；首次安装包含 bootstrap 和官方 cosign 时总下载约 444 MB，安装后磁盘占用并未等比例缩小。保持原有已验收主机 ABI 限制。

67 项 Python 发布测试、shell 安装完整性测试、Cargo 格式检查和实际版本命令测试通过。使用最终安装器实跑离线安装、缓存复用、重复安装和默认目录安装；Core 也已验证复用 Rust 缓存中的官方 cosign。最终安装器 SHA-256 为 `0a1136ede55682d1163ac8588a2d5d637b5ce57b1952b3d1885fc585e942219b`。默认插件安装在 `/home/gem/.local/share/harness-gate/rust-collector/versions/0.1.0-rc.1`，缓存位于 `/home/gem/.cache/harness-gate/collector`。未修改 Arc Admin 项目政策或历史基线。

本机验收收据：`/mnt/dev-ssd/workspaces/harness-gate-friendly-acceptance.json`；发布操作记录目录：`/mnt/dev-ssd/workspaces/harness-gate-friendly-publication`。


## 2026-09-12：优化版正式发布与本机升级完成

最终 main CI 34704575706 全部通过；新安装器发布工作流 34706631425、Core 发布工作流 34706873399 均为 success。Core `0.4.1` 已公开在 https://github.com/musutrade/Harness-Gate/releases/tag/v0.4.1 ，原流程的 crates.io 发布任务成功，直接读取官方 sparse index 确认 `harness-gate` 版本 `0.4.1` 存在且 `yanked=false`，包摘要为 `0a03984be110b4b174c0c4b8ad68925695bfde950549c490cf47e2f904fb20f6`。

新安装器已公开在 https://github.com/musutrade/Harness-Gate/releases/tag/rust-collector-installer-v0.1.0-rc.2 ，12 个附件公开；下载后的安装脚本摘要与本机验收版本一致，真实 Sigstore 签名验证通过。公开脚本本机运行通过，自动下载私有 bootstrap，复用验证工具缓存并完整验证已安装插件。旧插件 `rust-collector-v0.1.0-rc.1` 的发行说明已补新安装入口，原说明和签名资产保留。

使用新的原入口 `install.sh --version v0.4.1 --install-dir /home/gem/.local/share/harness-gate/versions/v0.4.1/bin` 从正式 GitHub Release 下载并验证 Core，二进制 SHA-256 为 `fb2d79704b4fd31b7372484b5af1f20651906f21020ff1f42027ff9bc39dc0dd`。确认原全局路径仍为本轮建立的 0.4.0 符号链接后，原子切换到已验证的 0.4.1，旧二进制保留；`/home/gem/.cargo/bin/harness-gate --version` 返回 `harness-gate 0.4.1`。本机未通过 cargo 重装；crates.io 发布由工作流与公开索引独立确认。

最终验收记录 `/mnt/dev-ssd/workspaces/harness-gate-friendly-acceptance.json` 状态为 pass，公开下载、签名、安装与 crates.io 索引回执保存在 `/mnt/dev-ssd/workspaces/harness-gate-friendly-publication`。
