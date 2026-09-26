# OctoSense app hub

English | [简体中文](README.zh-CN.md)

The index of apps published for OctoSense, the signed catalog every OctoSense
store reads, the hub's own copy of each admitted bundle, and the code that
runs the hub and the store. No app code lives here; each app stays in its
publisher's repository.

| Looking for | Repository |
| --- | --- |
| How to build an app: quickstart, script API, script-app template, design flows, examples | [OctoScript-App-Design-Flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow) |
| The AppCard assistant runtime and the L0 card language | [OctoSense-System-Apps `apps/appcard`](https://github.com/OctoSense-org/OctoSense-System-Apps/tree/main/apps/appcard) |
| The first-party system apps (News, Photos, Maps, Camera, Mail) | [OctoSense-System-Apps](https://github.com/OctoSense-org/OctoSense-System-Apps) |
| The bundle format, the gate, signing, submission and the store | this repository |

| Path | What it is |
| --- | --- |
| `catalog.json` | The signed catalog. Stores verify it against the anchor below before showing anything. |
| `index/<app>-<version>.json` | One admitted entry per app version: its manifest, publisher, source and status. Created by `hub admin publish`; absent while no app is published. |
| `artifacts/<app>-<version>.bundle/` | The hub's copy of the bundle, exactly the bytes that were reviewed. Created by `hub admin publish`. |
| `artifacts/<app>-<version>.bundle.pack.json` | The same bundle as one file, which stores download. |
| `docs/FIRST-APP.md` | First-app walkthrough for a card app or a script app: author, package, run, capture, validate and submit. |
| `docs/PUBLISHING.md` | The bundle, listing, capabilities, host services, gate rules, signing and submission contract. |
| `docs/reference/manifest-v2.md` | The explicit v2 runtime contract, compatibility checks and dual-catalog rollout. |
| `docs/ICONS.md` | Canonical icon ownership, export constraints and visual review. |
| `docs/DEVELOPMENT.md` | Where authoring lives, delivery paths, and `card-host` with its remote-control routes. |
| `templates/app/` | Card app repository scaffold with metadata, example icon and linked agent instructions. |
| `crates/app-policy` | The signed manifest and listing, admission, and resolution into an isolate's settings and an agent session profile (ADR 0002). |
| `crates/app-hub` | The index, the signed catalog, the gate, the agent scan, the device client and the `hub` command (ADR 0003). |
| `crates/appstore` | The store as an OctoSense module, the `card` module that runs an installed app as its own client, system apps (`os.` ids) and host services with their sheets. |
| `crates/appstore-app` | The store as a standalone app (`appstore`). |
| `crates/card-host` | The reference contained host for one bundle, card or script app (`card-host`). |
| `crates/app-host` | A one-window host that runs any OctoSense AppModule as a standalone app. |
| `crates/app-hub-app` | The shell integration every OctoSense shell links: the native store module, the `card` runner module, the system apps named by `OCTOSENSE_SYSTEM_APPS`, installed apps and icons ([README](crates/app-hub-app/README.md)). |

The crates build against the pinned OctoSense forks of Makepad and Octoscript,
resolved from sibling checkouts (`../makepad`, `../octoscript-makepad`,
`../octoscript`) as the launcher workspace does. `cargo test --workspace`
runs the policy, gate, signing and store tests headless; `cargo run -p
octosense-app-hub --bin hub` is the publishing tool.

## Trust anchor

Stores trust this anchor and follow its certificate to the working key that
signs the catalog. Rotating the working key needs no store release.

```
6000284a069ba7cada2925094074e8e0baae07e25d1b7fc31f396c993f363e11
```

## Pointing a store here

A store build reads this hub and trusts this anchor by default. The
variables override them, for a mirror or a development hub; spelled out,
the defaults are:

```sh
OCTOSENSE_HUB=https://raw.githubusercontent.com/OctoSense-org/OctoSense-App-Hub/main/ \
OCTOSENSE_HUB_ANCHOR=6000284a069ba7cada2925094074e8e0baae07e25d1b7fc31f396c993f363e11 \
appstore
```

## Publishing an app

An app is a Card app (`page.card`) or a script app (`main.splash`). Start
with [Build your first Hub app](docs/FIRST-APP.md), the
[starter](templates/app/README.md), and [Publishing](docs/PUBLISHING.md).
Stamp the bundle, capture a real screenshot, restamp, run `hub check` and
`hub test`, sign the manifest, then submit the tagged commit through the
[documented issue route](docs/PUBLISHING.md#submitting). A maintainer records
the review decision and publishes the exact validated bytes through the
durable operator transaction. A submission service and publisher accounts
are planned; neither is active. Withdrawals retain release history and reach
stores on their next verified catalog refresh.


## Apps

| App | Version | Category | Runs on | Publisher | Allowed to | Status |
| --- | --- | --- | --- | --- | --- | --- |
| _none yet_ | | | | | | |

The camera card that exercised the pipeline was removed on 20 Sep 2026:
Camera is a system app that ships with the ROM (like Calendar, News and
Photos), not a store app. Its repository stays at
[ymote/camera-card](https://github.com/ymote/camera-card) as a worked
example of a publishable bundle.
