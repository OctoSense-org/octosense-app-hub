# Developing this Hub app

> **Any coding agent, or none.** These instructions work the same for Codex, Claude Code, Cursor, Gemini CLI, GitHub Copilot or a person at a terminal: every step is a shell command or a file edit, and nothing here needs a particular agent, model or vendor. `AGENTS.md` is the one source of truth; `CLAUDE.md` and `GEMINI.md` only import it for agents that look for those names.

Before changing this app, read the shared
[first-app guide](https://github.com/OctoSense-org/OctoSense-App-Hub/blob/main/docs/FIRST-APP.md),
[publishing contract](https://github.com/OctoSense-org/OctoSense-App-Hub/blob/main/docs/PUBLISHING.md),
[icon guidelines](https://github.com/OctoSense-org/OctoSense-App-Hub/blob/main/docs/ICONS.md),
and [development guide map](https://github.com/OctoSense-org/OctoSense-App-Hub/blob/main/docs/DEVELOPMENT.md).
If unavailable online, use the corresponding files in a local Hub checkout and
record its revision; do not invent missing requirements.

- This repository owns the app; `bundle/` is the release artifact directory.
  Keep authoring instructions, source tooling, keys and test evidence outside it.
- Use the image-to-card flow in OctoScript-App-Design-Flow
  (`flows/image-to-card/FLOW.md`) for UI output and the L0 reference in
  OctoSense-System-Apps (`apps/appcard/a2app-l0/framework/l0.md`) for bindings. Do not
  hand-write L0 unless the app owner requests that approach.
- Maintain the app's own icon at the path declared by its complete listing.
  Avoid separate launcher/store artwork copies.
- Ask for only the capabilities needed by implemented features. The starter
  has no network, storage or assistant grants.
- Replace placeholder identity, publisher URLs, artwork and platform claims.
  Use real native captures for listing screenshots.
- Verify native behavior and appearance separately from bundle admission.
  Report exactly which platforms and flows were tested.
- Run `hub stamp` after every bundle edit, then `hub check`. Keep scan packets
  outside the bundle. Sign only final bytes; subsequent edits require a new
  stamp/signature. Keep publisher keys private and outside the bundle.

Add app-specific requirements, build commands and tests here as the app grows.
For an existing repository, merge this guidance with its existing instructions.
