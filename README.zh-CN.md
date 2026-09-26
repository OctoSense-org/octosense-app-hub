# OctoSense app hub

[English](README.md) | 简体中文

这里是为 OctoSense 发布的应用索引、每个 OctoSense 商店都会读取的签名目录、App Hub 为每个已准入应用包保存的副本，以及运行 hub 和商店的代码。这里不存放任何应用代码，每个应用都留在其发布者自己的仓库中。

| 想找 | 仓库 |
| --- | --- |
| 如何开发应用：快速上手、脚本 API、脚本应用模板、设计流程、示例 | [OctoScript-App-Design-Flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow) |
| AppCard 助手运行时与 L0 卡片语言 | [OctoSense-System-Apps `apps/appcard`](https://github.com/OctoSense-org/OctoSense-System-Apps/tree/main/apps/appcard) |
| 第一方系统应用（新闻、相册、地图、相机、邮件） | [OctoSense-System-Apps](https://github.com/OctoSense-org/OctoSense-System-Apps) |
| 应用包格式、准入检查、签名、提交与商店 | 本仓库 |

| 路径 | 说明 |
| --- | --- |
| `catalog.json` | 签名目录。商店在展示任何内容之前，先用下方的信任锚验证它。 |
| `index/<app>-<version>.json` | 每个应用版本一条已准入条目：清单、发布者、源码位置与状态。由 `hub publish` 生成；尚无已发布应用时不存在。 |
| `artifacts/<app>-<version>.bundle/` | App Hub 保存的应用包副本，与审核时的字节完全一致。由 `hub publish` 生成。 |
| `artifacts/<app>-<version>.bundle.pack.json` | 打成单个文件的同一应用包，商店下载的就是它。 |
| `docs/FIRST-APP.md` | 第一个应用（卡片应用或脚本应用）的完整演练：编写、打包、运行、截图、验证与提交。 |
| `docs/PUBLISHING.md` | 应用包、商店信息、能力、宿主服务、准入规则、签名与提交的完整规范。 |
| `docs/ICONS.md` | 规范图标的归属、导出约束与视觉审查。 |
| `docs/DEVELOPMENT.md` | 应用编写指南所在的仓库、交付路径，以及 `card-host` 及其远程控制路由。 |
| `templates/app/` | 卡片应用仓库脚手架，包含元数据、示例图标和链接好的 Agent 指引。 |
| `crates/app-policy` | 签名清单与商店信息、准入，以及解析为隔离环境设置和 Agent 会话配置（ADR 0002）。 |
| `crates/app-hub` | 索引、签名目录、准入检查、Agent 扫描、设备端客户端和 `hub` 命令（ADR 0003）。 |
| `crates/appstore` | 作为 OctoSense 模块的商店；把已安装应用作为独立客户端运行的 `card` 模块；系统应用（`os.` 前缀 id）；以及宿主服务及其面板。 |
| `crates/appstore-app` | 作为独立应用的商店（`appstore`）。 |
| `crates/card-host` | 隔离运行单个应用包（卡片应用或脚本应用）的参考宿主（`card-host`）。 |
| `crates/app-host` | 单窗口宿主，可把任意 OctoSense AppModule 作为独立应用运行。 |
| `crates/app-hub-app` | 每个 OctoSense Shell 都会链接的集成：原生商店模块、`card` 运行模块、由 `OCTOSENSE_SYSTEM_APPS` 指定的系统应用、已安装应用和图标（[README](crates/app-hub-app/README.md)）。 |

这些 crate 基于固定版本的 OctoSense Makepad 与 Octoscript 分支构建，和启动器工作区一样，从同级检出目录（`../makepad`、`../octoscript-makepad`、`../octoscript`）解析依赖。`cargo test --workspace` 以无界面方式运行策略、准入检查、签名和商店测试；`cargo run -p octosense-app-hub --bin hub` 是发布工具。

## 信任锚

商店信任这个锚，并沿着它的证书找到为目录签名的工作密钥。轮换工作密钥不需要发布新版商店。

```
6000284a069ba7cada2925094074e8e0baae07e25d1b7fc31f396c993f363e11
```

## 让商店指向这里

商店构建默认读取本 hub 并信任上述锚。下面的环境变量可以覆盖它们，用于镜像或开发用的 hub；写全之后，默认值如下：

```sh
OCTOSENSE_HUB=https://raw.githubusercontent.com/OctoSense-org/OctoSense-App-Hub/main/ \
OCTOSENSE_HUB_ANCHOR=6000284a069ba7cada2925094074e8e0baae07e25d1b7fc31f396c993f363e11 \
appstore
```

## 发布应用

应用分为卡片应用（`page.card`）和脚本应用（`main.splash`）。从[开发你的第一个 Hub 应用](docs/FIRST-APP.md)开始。完整规范见[发布](docs/PUBLISHING.md)，图标素材见[图标](docs/ICONS.md)；[开发指南导航](docs/DEVELOPMENT.zh-CN.md)链接了其他仓库中的编写与测试指南。

为应用包打戳，截图，重新打戳，运行 `hub check` 和 `hub scan`，为清单签名，然后在本仓库开一个 issue 提交（[提交](docs/PUBLISHING.md#submitting)）。目前还没有发布用的 Action，也没有独立的索引仓库：由维护者对你所打 tag 的那个提交的确切字节运行 `hub publish`，并提交签名后的目录。用 `hub withdraw` 撤回某个版本，每个商店在下次拉取时都会遵守；`hub remove` 用于删除本不该发布的条目。

## 应用

| 应用 | 版本 | 分类 | 运行平台 | 发布者 | 允许的权限 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| _暂无_ | | | | | | |

曾用于跑通发布流程的相机卡片已于 2026 年 9 月 20 日移除：相机是随 ROM 出厂的系统应用（与日历、新闻、相册一样），不是商店应用。它的仓库仍保留在 [ymote/camera-card](https://github.com/ymote/camera-card)，作为可发布应用包的完整示例。
