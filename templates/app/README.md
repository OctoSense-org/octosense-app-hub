# Hub app starter

English | [简体中文](README.zh-CN.md)

Copy this directory into a new app repository, following
[Build your first Hub app](https://github.com/OctoSense-org/OctoSense-App-Hub/blob/main/docs/FIRST-APP.md).
This is a **metadata scaffold**, not a runnable or publishable demo.

Included: schema-1 manifest and complete listing fields, an example SVG icon,
ignore rules, and agent instructions pointing to the shared authoring guides.

Before publication:

- Replace the app ID/name and all example listing/publisher values. List only
  platforms actually tested; choose the license for your own app.
- Replace `bundle/assets/icon.svg` with your app's artwork.
- Generate and review `bundle/page.card`, optional `page.data.json`, its `kit/`
  directory and local assets using the image-to-card flow in
  OctoScript-App-Design-Flow (`flows/image-to-card/FLOW.md`).
- Run it in the reference host and capture `bundle/screenshots/01-main.png`.
- Stamp, check, review, sign and submit the completed `bundle/` directory.

The initial digest is a placeholder; `hub stamp` writes the real value.
The declared screenshot is intentionally missing. A stamped but otherwise
untouched scaffold must fail `hub check`. Do not add a dummy screenshot just to
make it pass. The gate does not substitute for native rendering and input tests.

Keep this README, `AGENTS.md`, keys, tools, build output and application data
outside `bundle/`. The same applies to the generated review packet.
