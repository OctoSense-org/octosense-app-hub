# App developer guide map

English | [简体中文](DEVELOPMENT.zh-CN.md)

For a downloadable Hub app, begin with [Build your first Hub app](FIRST-APP.md).
The Hub owns publication requirements: the bundle format, the gate, signing
and submission. Authoring lives in other repositories:

| Repository | What it owns |
| --- | --- |
| [OctoScript-App-Design-Flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow) | The app-development harness: quickstart, script API, the script-app template, the `tools/octo` command, the design flows (`flows/`) and example apps (`examples/`). Formerly Octoscript-AppCard. |
| [OctoSense-System-Apps `apps/appcard`](https://github.com/OctoSense-org/OctoSense-System-Apps/tree/main/apps/appcard) | The AppCard assistant runtime and the L0 card language (`a2app-l0/framework/l0.md`). |
| [OctoSense-System-Apps](https://github.com/OctoSense-org/OctoSense-System-Apps) | The first-party system apps (News, Photos, Maps, Camera, Mail), each a script app bundle under `apps/<name>/bundle/`, and Mail's host service under `apps/mail/host-service/`. |

| Task | Guide |
| --- | --- |
| Package, validate, sign and submit a bundle | [Publishing](PUBLISHING.md) |
| Set up an app-owned icon and bundled artwork | [Icons](ICONS.md) |
| Start a card app repository with metadata and agent instructions | [App starter](../templates/app/README.md) |
| Start a script app from a runnable template | [Quickstart](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/QUICKSTART.md) and [`templates/script-app/`](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/templates/script-app) |
| Write a script app: state, handlers, storage, requests, host services | [Script-app flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/script-app/FLOW.md) and [Script API](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/SCRIPT-API.md) |
| Turn UI designs into native cards | [Image-to-card flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/image-to-card/FLOW.md) |
| Understand card data, state, events, copy, themes and views | [L0 language](https://github.com/OctoSense-org/OctoSense-System-Apps/blob/main/apps/appcard/a2app-l0/framework/l0.md) and the [L0 notes](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/docs/l0) |
| Prepare shared Makepad/Octoscript dependencies | [Native workspace](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/NATIVE-WORKSPACE.md) |
| Test real native input, capture frames and clean up test instances | [Native instrument](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/core/NATIVE-INSTRUMENT.md) |
| Read worked examples | [Examples](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/examples) and the [system apps](https://github.com/OctoSense-org/OctoSense-System-Apps) |

## Choose the appropriate delivery path

**Hub card app:** `page.card`, its data and kit, local artwork, manifest and
listing. The host lowers the card to widgets: presentation, no logic of its
own. It runs within the host's existing capabilities.

**Hub script app:** `main.splash`, local artwork, manifest and listing. A
Splash program with its own state, handlers, requests and storage, run in its
own isolate under the policy its manifest resolves to. It reaches the network
only through the hosts it declares, and the person's world (location, camera,
mail) only through the capabilities it is granted and the host services the
shell offers.

Both are store bundles. Neither may carry native code: new Rust/JNI code,
Python services and browser controllers are not installed by either format.

**System app:** a script app bundle that ships inside a shell release instead
of the store, packed at build time, under an id in the reserved `os.`
namespace. The first-party ones are in OctoSense-System-Apps; a store bundle
may not take an `os.` id.

**Built-in native app:** source integrated into a shell release. Use the native
workspace and the owning app's build instructions. Shared icon conventions
still apply, but an icon declaration alone does not make the app
Hub-installable.

**Agent-generated app type:** specifications and lint rules teaching the agent
to compose a new kind of app, in OctoSense-System-Apps (`apps/appcard`). Those specifications are
not themselves a store bundle.

Some workflow examples include native services or website integration. Check
that every behaviour of a proposed Hub app can run in the contained host; do
not assume copying a service project's source directory makes it installable.

## Running a bundle locally: `card-host`

`card-host` (this workspace, `crates/card-host`) runs one bundle, card or
script app, under exactly the policy its manifest resolves to, with the same
admission order a device uses: admit, resolve, apply, evaluate.

```sh
cargo build --release -p octosense-card-host --bin card-host
card-host --bundle <dir> [--app-data <dir>] [--allow-unsigned] [--stamp] [--system] [--static <prefix>=<dir>]...
```

| Flag | Effect |
| --- | --- |
| `--bundle <dir>` | The bundle to run (default: the current directory). |
| `--app-data <dir>` | Where the app's storage jail is made, at `<dir>/<app id>/`; host services keep their state in `<dir>/.host/`. Default: `$TMPDIR/octosense-card-apps`. |
| `--allow-unsigned` | Admit a manifest with no signature. `card-host` verifies no publisher keys, so a **signed** manifest is refused even with this flag: run an unsigned development copy. |
| `--stamp` | Rewrite the manifest's `integrity.bundle_blake3` to match the directory before admitting. Without it, a bundle whose bytes changed since the last `hub stamp` is refused. |
| `--system` | Admit as a system app is admitted: by digest only, under the system ceilings. An empty digest is filled in memory. For developing a system app. |
| `--static <prefix>=<dir>` | Serve `<dir>`'s files at `<prefix>/...` from memory, as a shell serves a system app's compiled-in artwork (Photos uses `--static photos=<dir>`). |

The log line `card-host: <id> <version> admitted — capabilities …, hosts …`
is what the app got. A refusal is logged as `card-host: refused: …` and
nothing is drawn.

`card-host` registers no host services. A script app that calls
`host.request("mail.…", …)` gets `no service answers "mail" on this device`
here; develop a service-backed app in a shell that links the service.

### Driving it: `MAKEPAD_REMOTE`

Launch with `MAKEPAD_REMOTE=<port>` (or `--remote`, which picks a free port
and logs it) to get a localhost HTTP control surface. Every route is a GET
and answers one line of JSON; coordinates are window-local layout points.

| Route | What it does |
| --- | --- |
| `/snap[?q=text]` | Visible widgets with their rects and text, ready to click; `q` filters by id, type or text. |
| `/click?x=&y=` | Click at a point. |
| `/t?t=TEXT` | Type text into the focused widget. |
| `/k?k=down\|up&c=KeyA` | Press or release a key (`ReturnKey`, `Backspace`, `Escape`, …); `/k?t=TEXT` types text. |
| `/g` | Grab the window; answers `{"png": "<path>", …}`. `/g?raw=1` sends the PNG bytes. |
| `/quit` | Shut the app down. Always end a session with it. |

Add `&wait=1` to an input route to answer after the next frame is drawn, so a
following `/g` sees the result. `GET /` prints the full list.

```sh
MAKEPAD_REMOTE=8151 card-host --bundle my-app/bundle --allow-unsigned --app-data /tmp/my-app-data &
sleep 7
curl -s 127.0.0.1:8151/snap
curl -s '127.0.0.1:8151/click?x=200&y=280&wait=1'
curl -s 127.0.0.1:8151/g          # {"png":"/…/grab-w0-00001.png",…}
curl -s 127.0.0.1:8151/quit
```

A hidden native window still requires the platform's graphical session.

## Keep instructions in sync

Use the starter's `AGENTS.md` as a short entry point to these shared documents.
Merge it with an existing repository's instructions instead of overwriting them.
Keep app-specific behaviour, data sources and tests in that app's repository.
Record the Hub and runtime revisions used for a release. When working offline,
an explicit versioned copy of the guides is preferable to an untracked copy
that silently becomes stale.

For native tests, follow the native-instrument guide. Passing a compilation or
packaging stage does not imply visual acceptance, working input, or platform
coverage.
