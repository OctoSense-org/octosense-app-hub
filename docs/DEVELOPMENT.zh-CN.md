# 应用开发指南导航

[English](DEVELOPMENT.md) | 简体中文

要开发可下载的 Hub 应用，请从[开发你的第一个 Hub 应用](FIRST-APP.md)开始。App Hub 负责发布要求：应用包格式、准入检查、签名与提交。编写相关内容在其他仓库中：

| 仓库 | 负责内容 |
| --- | --- |
| [OctoScript-App-Design-Flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow) | 应用开发工具集：快速上手、脚本 API、脚本应用模板、`tools/octo` 命令、设计流程（`flows/`）和示例应用（`examples/`）。原名 Octoscript-AppCard。 |
| [OctoSense-System-Apps `apps/appcard`](https://github.com/OctoSense-org/OctoSense-System-Apps/tree/main/apps/appcard) | AppCard 助手运行时与 L0 卡片语言（`a2app-l0/framework/l0.md`）。 |
| [OctoSense-System-Apps](https://github.com/OctoSense-org/OctoSense-System-Apps) | 第一方系统应用（新闻、相册、地图、相机、邮件），每个都是位于 `apps/<name>/bundle/` 下的脚本应用包；邮件的宿主服务位于 `apps/mail/host-service/`。 |

| 任务 | 指南 |
| --- | --- |
| 打包、验证、签名并提交应用包 | [发布](PUBLISHING.md) |
| 设置应用自有的图标和随包素材 | [图标](ICONS.md) |
| 用元数据和 Agent 指引搭建卡片应用仓库 | [应用起步模板](../templates/app/README.zh-CN.md) |
| 从可运行模板开始一个脚本应用 | [快速上手](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/QUICKSTART.md)和 [`templates/script-app/`](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/templates/script-app) |
| 编写脚本应用：状态、处理函数、存储、请求、宿主服务 | [脚本应用流程](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/script-app/FLOW.md)和[脚本 API](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/SCRIPT-API.md) |
| 把 UI 设计转成原生卡片 | [图像到卡片流程](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/image-to-card/FLOW.md) |
| 理解卡片的数据、状态、事件、文案、主题和视图 | [L0 语言](https://github.com/OctoSense-org/OctoSense-System-Apps/blob/main/apps/appcard/a2app-l0/framework/l0.md)和 [L0 笔记](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/docs/l0) |
| 准备共享的 Makepad/Octoscript 依赖 | [原生工作区](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/NATIVE-WORKSPACE.md) |
| 测试真实的原生输入、截取画面并清理测试实例 | [原生测试工具](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/core/NATIVE-INSTRUMENT.md) |
| 阅读完整示例 | [示例](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/examples)和[系统应用](https://github.com/OctoSense-org/OctoSense-System-Apps) |

## 选择合适的交付路径

**Hub 卡片应用：** `page.card` 及其数据和 kit、本地素材、清单和商店信息。宿主把卡片转换为控件：只负责呈现，自身没有逻辑。它在宿主已有的能力范围内运行。

**Hub 脚本应用：** `main.splash`、本地素材、清单和商店信息。这是一个 Splash 程序，有自己的状态、处理函数、请求和存储，在自己的隔离环境中、按其清单解析出的策略运行。它只能通过自己声明的主机访问网络，只能通过被授予的能力和 Shell 提供的宿主服务接触用户的世界（位置、相机、邮件）。

两者都是商店应用包。两者都不能携带原生代码：新的 Rust/JNI 代码、Python 服务和浏览器控制器都无法通过这两种格式安装。

**系统应用：** 随 Shell 版本发布而非通过商店分发的脚本应用包，在构建时打包，使用保留的 `os.` 命名空间下的 id。第一方系统应用位于 OctoSense-System-Apps；商店应用包不能使用 `os.` id。

**内置原生应用：** 源码集成进 Shell 版本。请使用原生工作区和所属应用的构建说明。共享的图标约定仍然适用，但仅声明图标并不会让应用可以通过 Hub 安装。

**Agent 生成的应用类型：** 位于 OctoSense-System-Apps（`apps/appcard`）中的规格说明和 lint 规则，教 Agent 组合出一种新的应用。这些规格说明本身不是商店应用包。

有些工作流示例包含原生服务或网站集成。请确认所提议的 Hub 应用的每一项行为都能在隔离宿主中运行；不要以为复制某个服务项目的源码目录就能让它变得可安装。

## 在本地运行应用包：`card-host`

`card-host`（本工作区的 `crates/card-host`）运行单个应用包（卡片应用或脚本应用），严格按照其清单解析出的策略，并采用与设备相同的准入顺序：准入、解析、应用、执行。

```sh
cargo build --release -p octosense-card-host --bin card-host
card-host --bundle <dir> [--app-data <dir>] [--allow-unsigned] [--stamp] [--system] [--static <prefix>=<dir>]...
```

| 参数 | 作用 |
| --- | --- |
| `--bundle <dir>` | 要运行的应用包（默认：当前目录）。 |
| `--app-data <dir>` | 应用的存储隔离目录建在 `<dir>/<app id>/`；宿主服务把状态保存在 `<dir>/.host/`。默认：`$TMPDIR/octosense-card-apps`。 |
| `--allow-unsigned` | 准入没有签名的清单。`card-host` 不验证任何发布者密钥，因此即使加了这个参数，**已签名**的清单也会被拒绝：请运行未签名的开发副本。 |
| `--stamp` | 在准入之前，重写清单的 `integrity.bundle_blake3` 使其与目录一致。不加这个参数时，自上次 `hub stamp` 以来字节有变化的应用包会被拒绝。 |
| `--system` | 按系统应用的方式准入：只校验摘要，适用系统上限。空摘要会在内存中补齐。用于开发系统应用。 |
| `--static <prefix>=<dir>` | 从内存中把 `<dir>` 的文件以 `<prefix>/...` 提供，就像 Shell 提供系统应用编译进来的素材一样（相册使用 `--static photos=<dir>`）。 |

日志行 `card-host: <id> <version> admitted — capabilities …, hosts …` 就是应用实际获得的权限。拒绝会记录为 `card-host: refused: …`，并且不绘制任何内容。

`card-host` 不注册任何宿主服务。调用 `host.request("mail.…", …)` 的脚本应用在这里会得到 `no service answers "mail" on this device`；依赖服务的应用请在链接了该服务的 Shell 中开发。

### 远程驱动：`MAKEPAD_REMOTE`

用 `MAKEPAD_REMOTE=<port>` 启动（或用 `--remote`，它会选一个空闲端口并打印出来），即可获得一个 localhost HTTP 控制接口。每个路由都是 GET，返回一行 JSON；坐标是窗口内的布局点。

| 路由 | 作用 |
| --- | --- |
| `/snap[?q=text]` | 可见控件及其矩形和文本，可直接点击；`q` 按 id、类型或文本过滤。 |
| `/click?x=&y=` | 在某点点击。 |
| `/t?t=TEXT` | 向当前焦点控件输入文本。 |
| `/k?k=down\|up&c=KeyA` | 按下或松开按键（`ReturnKey`、`Backspace`、`Escape` 等）；`/k?t=TEXT` 输入文本。 |
| `/g` | 截取窗口；返回 `{"png": "<path>", …}`。`/g?raw=1` 直接返回 PNG 字节。 |
| `/quit` | 关闭应用。每次会话结束都要调用它。 |

在输入类路由后加 `&wait=1`，会在下一帧绘制完成后才返回，这样随后的 `/g` 能看到结果。`GET /` 会列出全部路由。

```sh
MAKEPAD_REMOTE=8151 card-host --bundle my-app/bundle --allow-unsigned --app-data /tmp/my-app-data &
sleep 7
curl -s 127.0.0.1:8151/snap
curl -s '127.0.0.1:8151/click?x=200&y=280&wait=1'
curl -s 127.0.0.1:8151/g          # {"png":"/…/grab-w0-00001.png",…}
curl -s 127.0.0.1:8151/quit
```

隐藏的原生窗口仍然需要平台的图形会话。

## 保持指引同步

把起步模板的 `AGENTS.md` 当作通往这些共享文档的简短入口。与已有仓库的指引合并，而不是覆盖它们。应用特有的行为、数据来源和测试保留在该应用自己的仓库中。记录每次发布所用的 Hub 与运行时版本。离线工作时，明确带版本号的指南副本，比一份悄悄过时、未被跟踪的副本更可取。

原生测试请遵循原生测试工具指南。通过编译或打包阶段，并不代表视觉验收、输入可用或平台覆盖已经达成。
