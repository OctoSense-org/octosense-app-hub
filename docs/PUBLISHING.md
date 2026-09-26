# Publishing an app to the OctoSense app hub

This is the shared publication contract for app authors and their coding tools.
Start with [Build your first Hub app](FIRST-APP.md), follow the
[icon guidelines](ICONS.md), and use the [development guide map](DEVELOPMENT.md)
for UI, state, runtime setup and native testing.

The [app starter](../templates/app/README.md) includes a short
[`AGENTS.md`](../templates/app/AGENTS.md) that links to these guides. Merge it
into an existing repository's instructions. Keep any offline copy versioned
against a known Hub revision instead of maintaining independent rules.

The admission rules below are enforced by code and reported as refusals or
warnings. Authoring and visual-review recommendations are separate: a gate
pass does not prove the UI renders, the icon is readable, or the listing is
truthful. See [current icon enforcement](ICONS.md#technical-requirements-and-current-enforcement).

## What an app is

A bundle is a directory of text and artwork that OctoSense runs in its own
sandboxed isolate, under the policy its manifest resolves to. It contains no
native code. An app that needs new native runtime code must be integrated
into a shell release; see the
[delivery paths](DEVELOPMENT.md#choose-the-appropriate-delivery-path).

The entry file decides the kind (`crates/app-policy/src/entry.rs`): a bundle
with `main.splash` at its root is a **script app**; otherwise it is a
**card app** and runs `page.card`.

A **card app** is an L0 card the host lowers to widgets: presentation, no logic.

```
my-app/
  manifest.json      what the app is and what it may do   (required)
  listing.json       what the store shows about it         (required)
  page.card          the L0 card, the app's screen          (required)
  page.data.json     the data bound into the card           (optional)
  kit/               the kit the card is lowered with       (required)
  assets/            icon and other local runtime artwork   (icon required)
  screenshots/       at least one PNG the listing names     (required)
```

Produce the card, data and kit with the
[image-to-card flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/image-to-card/FLOW.md)
in OctoScript-App-Design-Flow. Do not hand-write L0 unless asked; the language
is specified in [L0](https://github.com/OctoSense-org/OctoSense-System-Apps/blob/main/apps/appcard/a2app-l0/framework/l0.md).

A **script app** is a Splash program with its own state, handlers, storage and
requests, evaluated as it is.

```
my-app/
  manifest.json      what the app is and what it may do   (required)
  listing.json       what the store shows about it         (required)
  main.splash        the program                            (required)
  assets/            icon and other local artwork           (icon required)
  screenshots/       at least one PNG the listing names     (required)
```

Write it with the
[script-app flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/script-app/FLOW.md)
and the [script API](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/SCRIPT-API.md),
starting from its [`templates/script-app/`](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/templates/script-app).
The program names its own artwork through the `{{assets}}` placeholder
(`http_resource("{{assets}}/assets/logo.png")`): the host replaces it with a
loopback origin that serves this bundle and nothing else, and adds only that
origin to the app's hosts. The first-party
[system apps](https://github.com/OctoSense-org/OctoSense-System-Apps)
(`apps/<name>/bundle/`) are complete examples; their `os.` ids are reserved,
so a store copy needs an id of its own.

Keep developer instructions, source tools, keys, test data directories and
review packets outside the submitted bundle.

## Rules the gate enforces

| Rule | What is refused |
| --- | --- |
| Card/data assets stay local | Any `http://`, `https://`, `file://` or `../` in Card/data/text source. Ship artwork in the bundle. |
| Script network is declared | A `.splash` program may reach only declared HTTPS hosts, unless it requests `images` or `web`, which allow public HTTPS hosts. Name bundled assets with `{{assets}}`. Plain HTTP and local/parent paths are refused. |
| Allowed file types only | Anything other than `.card .json .l0 .octoscript .splash .svg .png .jpg .jpeg .webp .ttf .otf .txt .md`. |
| Size and shape | Payload over 8 MiB, manifest over 64 KiB, more than 2,048 entries, depth over 32, text over 1 MiB or Card source over 256 KiB. |
| Runnable entry | A Card needs `page.card` and its kit closure; a script app needs `main.splash`. Structural checks do not execute either entry. `hub test` performs native preparation/lifecycle. |
| Artwork | Corrupt PNG/JPEG/WebP, images over 4096×4096 or 64 MiB decoded, malformed/active SVG or external SVG resources. Icons must be square; bitmap icons are at most 1 MiB and 1024×1024. |
| No secrets | App source declaring password or one-time-code input. Host services own those sheets. |
| System ids are reserved | An id starting with `os.`; system apps ship with the device. |

| No symlinks | Any symlink in the bundle. |
| Digest matches | A manifest whose `integrity.bundle_blake3` does not match the directory. Run `hub stamp` after any change. |
| Manifest is exact | Unknown fields, an unknown capability, a `schema` other than 1, an id outside `[a-z0-9.-]{1,64}` not starting with `.`. |
| Hosts are bare | A host with a scheme, path, port, wildcard or credentials. `api.example.com` is right; `https://api.example.com/v1` and `*.example.com` are refused. |
| Hosts need `net` | Listing hosts without requesting the `net` capability. |
| Version is new | Re-publishing a version already in the catalog. |
| Listing present and complete | No `listing.json`; no icon or no screenshot; an unknown category, platform or age rating; a non-https privacy policy; or an icon or screenshot the listing names that is not in the bundle. |
| Publisher continuity | An update signed by a different key than the one on record for this app. |
| Public release signature | An unsigned first release or update; `--allow-unsigned` is a local development check only. Publisher and signature key IDs must agree. |

`hub check <bundle> --json` emits the same structural report as the library:
stable check codes, file/property paths and a typed resource inventory. Schema
properties and arbitrary app data are not resource loads. Display URLs remain
subject to the existing conservative text rule until runtime network conformance
is implemented. Structural checks do not prove native loading, successful app
behavior or visual quality. Run `hub test <bundle> --json` for native checks.
It uses the same Card preparation and Splash policy adapter as installed apps,
including resolved memory/instruction limits, a private temporary storage jail,
and startup/shutdown. Resolved local assets must exist; the current installed
font loader supports `makepad_widgets:resources/Inter.ttf` only.

Build the worker beside `hub` with
`cargo build --release -p octosense-app-hub -p octosense-app-validator --bins`.
`hub test` and `hub publish` use that sibling `app-validator`; an operator can
select a trusted executable with `--validator <path>`. Publisher-supplied reports
are never accepted as evidence. The Hub runs the worker itself against an owned
bundle snapshot, with a 10-second wall limit, bounded output and process resource
limits; the worker caps Rust allocations at 256 MiB. This runner requires Unix.
These limits are not an operating-system sandbox for arbitrary native code.

`hub test --json` returns `schema`, `stage`, `passed`, and `findings` on success
and refusal (with nonzero exit status for refusal). Structural failures retain
the same codes/paths as `hub check`; worker failures use
`runtime-validation-failed` with the cause in `detail`. A successful result also
contains `evidence`: payload/complete-manifest hashes, validator executable hash,
runtime/check versions, host target and completed checks.

Publication requires these checks again, uses the owned checked bytes, and writes
`<artifact>.validation.json` alongside the published pack. `--reviewed` is not accepted: operators record a reviewer identity and decision
ID, and native validation is always required. See [catalog operations](operations/catalog.md)
for the privileged publication/withdrawal commands and transaction inputs. Reports and temporary data stay outside the bundle.
This smoke check does not draw GPU frames or establish device, visual or functional
interaction quality. Test the app's declared behavior in the reference host and
on each claimed platform; the current static fixture declares no interactions.

## The manifest

```json
{
  "schema": 1,
  "id": "weather-card",
  "version": "1.0.0",
  "name": "Weather",
  "integrity": { "bundle_blake3": "<written by hub stamp>" },
  "capabilities": ["storage", "net"],
  "network": { "hosts": ["api.weather.example"] },
  "storage": { "max_bytes": 1048576 },
  "compute": { "instruction_budget": 5000000, "memory_bytes": 33554432 },
  "agent": {
    "profile": "workspace-write-never-ask",
    "tools": ["net.fetch", "storage.read"],
    "max_iterations": 4,
    "token_budget": 50000
  }
}
```

Ask for the least the app needs. Everything not requested is not granted, and
the store shows the person exactly what was requested, in plain words, before
they install.

**Capabilities** (closed list, `KNOWN_CAPABILITIES` in
`crates/app-policy/src/manifest.rs`). Anything else is refused.

| Capability | Grants | The store says |
| --- | --- | --- |
| `storage` | Read and write inside the app's own storage jail. | Keep its own data on this device |
| `net` | Requests to the hosts in `network.hosts`, and no others. | Reach only: *hosts* |
| `prompt` | Raise a prompt the person answers. | Ask you questions |
| `ledger.read` | Read the shared ledger; writing is always the app's own rows. | Read your shared data |
| `location` | The device's location. | Use your location |
| `camera` | The camera. | Use the camera |
| `clipboard` | The clipboard. | Use the clipboard |
| `images` | Show pictures from any public https host, not only `network.hosts` (a feed's thumbnails). | Show pictures from any website |
| `web` | Open any public https page in the system web view, which has no way back into the app. | Open web pages in a browser view |
| `microphone` | Record sound with a camera video. | Use the microphone |
| `library` | Offer captures to the system photo library, where other apps can see them; without it, captures stay in the app's storage. | Save to your photo library, where other apps can see it |
| `mail` | Read and send mail through the host's mail service, from accounts the person signs in to on the host's sheet. | Read and send mail from accounts you sign in to on the device |

Location, camera and clipboard are each a separate consent; none implies
another.

**Network**: `net` plus an exact host list. The list is enforced on every path
out of the isolate: the network module, artwork loading and data fetches. An
empty list with `net` reaches nothing. Hosts match exactly: listing
`example.com` does not allow `api.example.com`.

**Id**: `[a-z0-9.-]{1,64}`, not starting with `.`, never containing `..`.
Ids under `os.` are reserved for system apps.

**Quotas** are requests; the host clamps them to its ceilings (storage 16 MB,
20 000 000 instructions, 64 MB heap). Ask for less than the ceiling when you can.

**Agent** is optional; omit it and the app gets no assistant. `profile` is one
of `read-only`, `workspace-write`, `workspace-write-never-ask`. Full access does
not exist in this schema; do not add it. `tools` may name only what the host
offers contained apps: `ledger.read`, `ledger.write`, `net.fetch`,
`storage.read`, `storage.write`, `card.render`. Iterations clamp to 8, tokens
to 200 000. The agent's workspace is the app's own storage jail and its hosts are
the app's hosts; it cannot be given more than the app.

## The listing

`listing.json` is what a person sees in the store before installing. It is
reviewed with the bundle and travels in the signed catalog, so what a
reviewer read is what the store shows. The permissions shown beside it come
from the manifest, never from here: a listing cannot understate what the app
does.

```json
{
  "schema": 1,
  "subtitle": "One line under the name (80 characters)",
  "description": "What the app does, for a person deciding whether to install it (4000 characters).",
  "category": "photo-video",
  "keywords": ["camera", "viewfinder"],
  "screenshots": ["screenshots/01-photo-mode.png"],
  "icon": "assets/icon.svg",
  "platforms": ["macos", "android", "linux"],
  "publisher": {
    "name": "Your name or organisation",
    "support": "https://github.com/you/my-app/issues",
    "privacy_policy_url": "https://github.com/you/my-app/blob/main/PRIVACY.md"
  },
  "release_notes": "What changed in this version.",
  "age_rating": "all",
  "license": "Apache-2.0"
}
```

Rules the gate enforces: `category` is one of `productivity utilities
photo-video news weather travel finance health education entertainment games
social shopping lifestyle developer`; `platforms` names at least one of
`android ios macos windows linux openharmony web` (list what you tested; a
bundle runs wherever the OctoSense shell does); `age_rating` is one of
`all 12+ 16+ 18+`; `privacy_policy_url` is an https URL; every screenshot
and the icon is a PNG or SVG inside the bundle; at most 10 keywords and 8
screenshots; unknown fields are refused. An icon and at least one screenshot
are required: the icon is what the launcher shows once the app is installed,
and a screenshot is the one claim a reviewer can check against the running app.
Use one app-owned canonical icon across store and launcher surfaces; see
[ICONS.md](ICONS.md) for export limits, native rendering and small-size review.
The two-field icon declaration used by a built-in native app is not a complete
publishable listing.

To produce a screenshot, run an unsigned development bundle in the reference
host, `MAKEPAD_REMOTE=8151 card-host --bundle my-app --allow-unsigned &`, then
`curl 127.0.0.1:8151/g` (capture metadata, with the PNG's path) or
`curl -o 01-main.png '127.0.0.1:8151/g?raw=1'` (the PNG bytes), and
`curl 127.0.0.1:8151/quit`. Capture the actual app content at the host's
current dimensions; do not assume a fixed crop. `card-host` verifies no
publisher keys, so use an unsigned development copy before final signing. See
the [card-host reference](DEVELOPMENT.md#running-a-bundle-locally-card-host),
the [first-app walkthrough](FIRST-APP.md#4-run-the-unsigned-development-bundle-and-capture-it)
and the [native testing guide](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/core/NATIVE-INSTRUMENT.md).

The store also shows a **privacy summary derived from the manifest**: what
the app stores, which hosts it contacts, which device features it uses,
whether it runs an assistant. Do not restate it in the description; make
the manifest right instead.

## Host services and sheets

A script app does not hold sockets, credentials or devices it does not need.
For work that needs them it calls a **host service**
(`crates/appstore/src/services.rs`):

```
host.request("mail.list", {…}, fn(r){ … })
```

The isolate refuses the call unless the app's policy grants the family
(`mail` for `mail.*`). A granted call goes to the Rust service registered for
that family, which does the work and answers the app with data, never with
the means. A family no service answers fails at once with
`no service answers "<family>" on this device`; the reference `card-host`
registers none.

**Apps never collect secrets.** Input only the person should give (a
password, an account approval) is collected on a **sheet**: a host-owned
surface the runner draws over the app, in an isolate of its own under no
app's policy. Only a service can open one. Service methods that take a secret
live under `<family>.sheet.` and are accepted only from that sheet, before any
service sees the call. An app's own password fields take no input at runtime,
and the gate refuses a bundle that declares one. Service state (accounts,
secrets, caches) lives in `<app data>/.host`, outside every app's jail.

Mail is the worked example: the
[Mail bundle](https://github.com/OctoSense-org/OctoSense-System-Apps/tree/main/apps/mail/bundle)
requests `mail`, and its
[host service](https://github.com/OctoSense-org/OctoSense-System-Apps/tree/main/apps/mail/host-service)
signs in on its own sheet. A store app can request `mail` only where the
shell links a mail service.

## Commands

The `hub` command is the same binary the hub itself runs, so the report you see
locally is the report the hub acts on. Build it from this repository with
`cargo build --release -p octosense-app-hub --bin hub`.

```sh
hub stamp bundle
hub check bundle --allow-unsigned --json
hub test bundle --allow-unsigned --json
hub scan bundle --packet build/review.json
```

The CLI reports the grants the app requests and runtime findings. To check a
new release against published history, use `--catalog <catalog.json>
--anchor <trusted-anchor-public-hex>`. The anchor must come from trusted
configuration. Keep review reports outside the bundle. For screenshots,
restamp and sign after every artwork change.


`hub scan` writes the questions a reviewer answers: does the app do what its
name claims, do its grants (and, for a script app, the hosts it requests) match
what it visibly does, is anything deceptive, does any text address an
assistant rather than a person. Answer them honestly before submitting; the
hub's reviewer asks the same ones.

## The full sequence

From an app repository whose bundle is `bundle/`, with `hub` and `card-host`
built:

```sh
# 1. Digest the bundle as it is.
hub stamp bundle

# 2. Run it and capture a real screenshot (unsigned, see card-host above).
MAKEPAD_REMOTE=8151 card-host --bundle bundle --allow-unsigned --app-data .local-state &
sleep 7
mkdir -p bundle/screenshots
curl -s -o bundle/screenshots/01-main.png '127.0.0.1:8151/g?raw=1'
curl -s 127.0.0.1:8151/quit

# 3. The screenshot changed the bytes: restamp.
hub stamp bundle

# 4. The gate and the review packet, on the final unsigned bytes.
hub check bundle --allow-unsigned
mkdir -p build && hub scan bundle --packet build/review.json

# 5. A publisher key, made once and kept outside the repository.
export APP_PUBLISHER_ID="your-publisher-id"
export APP_SIGNING_KEY="/absolute/private/path/publisher.key"
test -e "$APP_SIGNING_KEY" || hub keygen "$APP_SIGNING_KEY"
APP_PUBLISHER_PUBLIC_KEY="$(hub pubkey "$APP_SIGNING_KEY")"

# 6. Sign, and check the signed bytes the way the hub will.
hub sign-manifest bundle --key "$APP_SIGNING_KEY" --key-id "$APP_PUBLISHER_ID"
hub check bundle --publisher-key "$APP_PUBLISHER_ID=$APP_PUBLISHER_PUBLIC_KEY"

# 7. Commit and tag the signed bundle in your repository, then submit (below).
```

## Signing

Signing is required for every public release, including the first. Local
unsigned `hub check` and `hub test` remain available with `--allow-unsigned`.
The issue route below is the current submission path; publisher self-service
is planned.


The signature covers the manifest, including `integrity.bundle_blake3`, so:

- **Stamp, then sign.** Signing an unstamped manifest signs the wrong digest.
- **Any edit after signing means restamp and re-sign.** Changing any file
  (a screenshot, the listing, one character of `main.splash`) makes
  `hub check` refuse with `digest: the bundle hashes to …, the manifest
  claims …`. Restamping alone then fails with `publisher-signature: the
  signature from key "<id>" does not match the manifest`. Run `hub stamp`,
  then `hub sign-manifest`, then `hub check --publisher-key` again.
- A signed bundle checked without `--publisher-key` is refused, by design.
- A new version needs a new `version` in the manifest; a published version is
  never replaced.

For an existing publisher, the gate also verifies against the public key in
the authenticated catalog. Passing a different `--publisher-key` under the same
ID cannot replace that binding, even for another app. V1 uses the publisher ID
as the signature key ID; `hub admin publish --publisher` must match it. Conflicting
historical bindings or an unknown rotation require operator reconciliation;
there is no key-override flag. Keep your signing key for updates. First-key
enrollment remains an operator responsibility until publisher accounts launch.

## Submitting

**What exists today.** The hub's published state is this repository,
[OctoSense-org/OctoSense-App-Hub](https://github.com/OctoSense-org/OctoSense-App-Hub):
`catalog.json` (the signed catalog stores read), `artifacts/` (the hub's
copy of each admitted bundle) and `index/` (one entry per admitted version).
The catalog and the artifacts are written by `hub admin publish`, which re-runs the
gate (and the scan, with a reviewer), copies the bundle and signs a new
catalog with the hub's working key; the maintainer adds the `index/` entry in
the same commit. Publishers do not hold that key.

**Not yet available.** There is no separate index repository and no
`octosense-org/publish-app` GitHub action. Do not add a release workflow that
uses them. When they exist, this section will say so and give the workflow.

**The route maintainers accept now:**

1. Push the signed bundle to your app's public repository and tag the commit
   (for example `v1.0.0`).
2. Open an issue in
   [OctoSense-org/OctoSense-App-Hub](https://github.com/OctoSense-org/OctoSense-App-Hub/issues)
   titled `Submit <app id> <version>`, giving:
   - the repository URL, the tag and the full commit SHA;
   - the bundle's path in that repository (usually `bundle/`);
   - your publisher id and public key (`hub pubkey`); public releases must
     be signed, including a first submission;
   - the complete output of `hub check` on that commit (with
     `--publisher-key`), and your answers to the `hub scan` questions.
3. Do not open a pull request that edits `catalog.json`, `index/` or
   `artifacts/`. A catalog not signed by the hub's key is refused by every
   store, and the admitted bytes must be the ones the gate checked.

A maintainer checks out that commit, runs `hub check` and `hub scan` on the
exact bytes, and, if both pass, runs
`hub admin publish <bundle> --catalog catalog.json --state-dir <private state> --expected-sequence <n> --idempotency-key <request> --anchor <public hex> --key <working key file> --anchor-cert <certificate> --publisher <id> --publisher-key <id>=<hex> --reviewed-by <operator> --review-id <decision> …`
and commits the result. A first submission, or a scan that asks for human
review, waits for a person. The issue is closed with the catalog sequence the
app appeared in, or with the findings to fix.

## What happens after

- The hub keeps its own copy of the bundle and signs a new catalog. Every
  OctoSense store verifies that catalog against an anchor it ships with, so a
  catalog nobody signed is never shown.
- The app runs in its own isolate with exactly the manifest's grants; a
  request outside them fails with an error, and the person sees why.
- Publishing v2 leaves approved installed v1 usable with v1's permissions. The store shows Open and Update separately; update consent refers to v2.
- A withdrawal targets an exact app/version. On the mobile shell's next verified catalog refresh, matching running instances close and further launches are refused. Unaffected versions keep working.
- Offline launches use the last authenticated catalog. A stale catalog can still approve an installed release, while freshness rules block new installs. A device cannot learn a new withdrawal until it receives a verified catalog. Missing releases and modified installed content are refused.
- Launch verification runs on a worker and retains an owned copy of the verified code/artwork, so an update cannot replace code underneath a starting app. App data stays in the app's existing jail.

## Do not

- Reference any server, CDN or local path from a card, or an undeclared host
  from a script app. Bundle the asset.
- Request `prompt`, `location`, `camera`, `microphone`, `clipboard`, `library`,
  `images`, `web` or `mail` unless a screen needs it; each is shown to the
  person as a separate line.
- Ask for a password, PIN or code in the app. A host service asks on its own
  sheet.
- Put instructions to an assistant in card text or data. The scan treats text
  addressed to an assistant as a reason to reject.
- Edit `integrity.bundle_blake3` by hand. Run `hub stamp`.
- Reuse a version number.
