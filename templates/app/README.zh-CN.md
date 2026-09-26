# Hub 应用起步模板

[English](README.md) | 简体中文

按照[开发你的第一个 Hub 应用](https://github.com/OctoSense-org/OctoSense-App-Hub/blob/main/docs/FIRST-APP.md)，把这个目录复制到新的应用仓库中。这是一个**元数据脚手架**，不是可运行或可发布的演示。

包含内容：schema 1 清单和完整的商店信息字段、一个示例 SVG 图标、忽略规则，以及指向共享编写指南的 Agent 指引。

发布之前：

- 替换应用 ID/名称，以及所有示例商店信息和发布者信息。只列出实际测试过的平台；为你自己的应用选择许可证。
- 用你的应用图标替换 `bundle/assets/icon.svg`。
- 使用 OctoScript-App-Design-Flow 中的图像到卡片流程（`flows/image-to-card/FLOW.md`），生成并审查 `bundle/page.card`、可选的 `page.data.json`、它的 `kit/` 目录和本地资源。
- 在参考宿主中运行它，并截取 `bundle/screenshots/01-main.png`。
- 对完成后的 `bundle/` 目录打戳、检查、审查、签名并提交。

初始摘要只是占位符；`hub stamp` 会写入真实值。声明的截图是有意缺失的。一个打过戳、其他部分原封不动的脚手架必须无法通过 `hub check`。不要为了让它通过而加入假截图。准入检查不能替代原生渲染和输入测试。

把本 README、`AGENTS.md`、密钥、工具、构建产物和应用数据放在 `bundle/` 之外。生成的审查包也一样。
