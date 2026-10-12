# v0.4.0 与当前主线对比

2026-09-12 从 GitHub 拉取 `origin/main` 和 tags 后核对。结论：无需重新发布 Core；验收使用正式 Core v0.4.0 和当前主线的采集器实现。此前将“采集器缺少正式交付”描述成可能需要更新 Core，范围不准确。

| 对比项 | 结果 |
| --- | --- |
| v0.4.0 提交 | `ee544690662645806cda7dd4cd2c9192566f7929` |
| 当前主线 | `6b4fbe90e38d5019d4a51fd936ab319f78264df8` |
| 主线新增提交 | 5 个合并提交：#237、#238、#241、#242、#243 |
| `tools/harness-gate` | 完全一致，包含 Rust 实现、Cargo 清单/锁文件、测试与 presets |
| Core 目录 Git tree | 两端均为 `2cb1323af14f4d8c3578c232c8e8c39f9eb510b4` |
| Core `install.sh`、`.github`、`.harness-gate`、`schema` | 无差异 |
| `tools` 中变更 | 22 文件，新增 2520 行、删除 26 行；均为采集器 Python 实现、schema、独立交付/发布工具及相关测试 |

主要功能差异在采集器：新增 adapter v2 请求到 native 测量的绑定与投影、已安装运行时/官方 Core/宿主 ABI 的交付检查，以及独立生产签名和安装入口。v0.4.0 的 Core 已包含消费该协议的接口，因此这些差异不要求重新构建或升级 Core。

341 个文件的整体 diff 包含大量历史验收 JSON、日志和二进制证据分片，不能据此判断 Core 落后。当前工作目录原本停在旧功能分支 `4288c98`，本次以远端主线作比较，没有将其误当最新代码。

## 本次检查

从固定主线提交导出 `tools/quality`、`tools/release` 和 `schema` 到独立证据目录，执行：

- `test_rust_collector_project.py`：6 项通过。
- `test_rust_collector_delivery.py`：4 项通过。
- `test_collector_production.py`：12 项通过。

合计 22 项通过。这些是请求、交付及生产工具的契约测试，包含模拟/fixture，不等于真实 Arc Admin 采集通过、真实生产签名完成或已发布独立采集器。

源码快照与完整日志位于 `/home/gem/Harness-Gate/target/current-main-acceptance-20260912/`。已安装正式 Core 的路径、签名验证和 Arc Admin 导入结果见 [本机验收记录](installation-acceptance-20260912.zh-CN.md)。

## 执行决定

不创建内容不变的 Core v0.4.1，不重复 Core 发布流程。后续验收固定使用已验签的 v0.4.0 二进制和上述主线采集器；独立采集器的真实打包/签名/安装，以及 Arc Admin 项目配置和运行验收仍需完成。若验收发现 Core 缺陷并修复，再依据真实代码差异发布新 Core 版本。
