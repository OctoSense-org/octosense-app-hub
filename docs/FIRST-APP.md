# Build your first Hub app

This walkthrough takes one app from an empty repository to a bundle the gate
passes. A Hub app is one of two kinds:

- a **card app**: `page.card` in L0, its data and kit. The host lowers it to
  widgets; it has no logic of its own.
- a **script app**: `main.splash`, a Splash program with its own state,
  handlers, storage and requests.

Both are submitted the same way. Native binaries ship with the shell and follow
a different build process; see the [development guide map](DEVELOPMENT.md).

## 1. Prepare the tools and an app repository

Use a Rust toolchain and the shared Makepad/Octoscript checkouts described in
the [native workspace guide](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/NATIVE-WORKSPACE.md).
The Hub workspace's Cargo manifests declare its expected revisions and sibling
source overrides. Check those revisions before building; do not update a dirty
shared checkout just to satisfy a guide.

From the Hub repository:

```sh
cargo build --release -p octosense-app-hub -p octosense-app-validator --bins
cargo build --release -p octosense-card-host --bin card-host
```

Then choose absolute paths:

```sh
export HUB_REPO="/absolute/path/to/OctoSense-App-Hub"
export HUB_BIN="$HUB_REPO/target/release/hub"
export CARD_HOST_BIN="$HUB_REPO/target/release/card-host"
export APP_REPO="/absolute/path/to/my-app"
```

If Cargo uses a custom target directory (`CARGO_TARGET_DIR`), set the binary
paths to its actual outputs.

Start the repository from a template, into a **new** destination:

- Card app: the [app starter](../templates/app/README.md) in this repository,
  `test ! -e "$APP_REPO" && cp -R "$HUB_REPO/templates/app" "$APP_REPO"`. It
  holds metadata and an example icon, and deliberately no UI, kit or
  screenshot.
- Script app: `templates/script-app/` in
  [OctoScript-App-Design-Flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/tree/main/templates/script-app),
  following its [quickstart](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/QUICKSTART.md).
  It is a small runnable app (a notes list kept in storage) with manifest,
  listing and icon.

For an existing app repository, merge the template files deliberately; do not
overwrite its instructions or metadata. Only `bundle/` is submitted. Keep
`AGENTS.md`, design prompts, source tools, logs, keys and test state outside it.

## 2. Define and build the experience

Describe the app's purpose, screens, actions, data sources, state and error
handling first.

### A card app

Use the [image-to-card flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/image-to-card/FLOW.md)
to create and review the native card output. App-specific service logic and
data binding still require implementation; a screenshot or an atlas is not a
running app. Use [L0](https://github.com/OctoSense-org/OctoSense-System-Apps/blob/main/apps/appcard/a2app-l0/framework/l0.md)
for data/state/event semantics.

```text
my-app/
  AGENTS.md
  README.md
  bundle/
    manifest.json
    listing.json
    page.card
    page.data.json       # optional; omit if the card needs no data
    kit/                 # all files required by this card's kit
    assets/              # local runtime artwork and your icon
    screenshots/
      01-main.png        # actual capture, added after native validation
```

### A script app

Follow the [script-app flow](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/script-app/FLOW.md);
the calls a program may make (`fs`, `net`, `host.request`, widget handles)
are in the [script API](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/docs/SCRIPT-API.md).
The [system apps](https://github.com/OctoSense-org/OctoSense-System-Apps) are
complete worked examples of the same format.

```text
my-app/
  AGENTS.md
  README.md
  bundle/
    manifest.json
    listing.json
    main.splash          # the program; its presence makes this a script app
    assets/              # local artwork and your icon
    screenshots/
      01-main.png
```

Name the bundle's own artwork through `{{assets}}`, for example
`Image{src: http_resource("{{assets}}/assets/logo.png")}`. The host replaces
`{{assets}}` with the loopback origin that serves this bundle and nothing
else; never write that origin, a `file://` path or a `../` path yourself.
An `https://` address in `main.splash` must name a host in the manifest's
`network.hosts`, unless the app is granted `images` or `web`; `http://` is
refused.

A script app never asks for a password, PIN or one-time code. A field with
`is_password: true` or a password/one-time-code content type is refused by the
gate and takes no input at runtime. Sign-in belongs to a host service, which
collects the secret on its own sheet (see
[host services](PUBLISHING.md#host-services-and-sheets)).

### Both kinds

Keep every asset reference inside the bundle. Check exported kit and data
files for stale author-machine paths and development-server URLs. A flow that
relies on an external Python or browser controller must be adapted to the
contained runtime before it is a Hub app.

## 3. Complete identity, permissions and artwork

Edit `bundle/manifest.json`: choose a stable app id, release version and name.
Ids under `os.` are reserved for system apps and are refused. Add only the
capabilities the implemented app needs; `net` also needs exact hosts. The
[manifest reference](PUBLISHING.md#the-manifest) lists every field and
capability.

Edit **all** placeholder values in `bundle/listing.json`: description, category,
publisher/support/privacy information, tested platforms, release notes and
license. Replace the example icon following [ICONS.md](ICONS.md). The icon's
declared path must match the exported asset. Declare only platforms you tested.
The templates' `example.com` URLs are placeholders, not your privacy policy.

## 4. Run the unsigned development bundle and capture it

Stamp after every change to the bundle, then run it in the reference host:

```sh
"$HUB_BIN" stamp "$APP_REPO/bundle"
cd "$HUB_REPO"
MAKEPAD_REMOTE=8151 "$CARD_HOST_BIN" --bundle "$APP_REPO/bundle" \
  --app-data "$APP_REPO/.local-state" --allow-unsigned &
```

Launch from the Hub workspace so the host's resources resolve. Keep the bundle
unsigned for this check: `card-host` verifies no publisher keys and refuses
signed manifests even with `--allow-unsigned`. Use an unsigned development
copy when revisiting an already signed release. All `card-host` flags and the
remote routes are in the [development guide](DEVELOPMENT.md#running-a-bundle-locally-card-host).

Inspect the host log for `admitted` **and** for the app evaluating without
errors. Test the actual interactions and app state through the remote routes
(`/snap`, `/click`, `/t`, `/k`) or the
[native instrument guide](https://github.com/OctoSense-org/OctoScript-App-Design-Flow/blob/main/flows/core/NATIVE-INSTRUMENT.md).

Capture a frame and quit:

```sh
mkdir -p "$APP_REPO/bundle/screenshots"
curl --fail --silent --show-error "127.0.0.1:8151/g?raw=1" \
  -o "$APP_REPO/bundle/screenshots/01-main.png"
curl --fail --silent --show-error "127.0.0.1:8151/quit"
```

(`/g` without `raw=1` answers `{"png": "<path>", …}`; copying that file works
too.) Inspect the captured PNG before using it; do not publish an error
frame. Use the actual app content at the host's current dimensions instead of
assuming a fixed artboard size. Review the icon at small sizes and, during
integration, in the target shell's launcher and Hub surfaces.

## 5. Check the final bytes

Adding a screenshot changes the bundle digest. Restamp, then run the gate and
produce a review packet **outside** the bundle:

```sh
mkdir -p "$APP_REPO/build"
"$HUB_BIN" stamp "$APP_REPO/bundle"
"$HUB_BIN" check "$APP_REPO/bundle" --allow-unsigned
"$HUB_BIN" test "$APP_REPO/bundle" --allow-unsigned --json
"$HUB_BIN" scan "$APP_REPO/bundle" --packet "$APP_REPO/build/review.json"
```

The script-app template, with a real screenshot, passes as:

```text
my-notes 0.1.0 — PASSED
  [warning] publisher-signature: unsigned: accountability rests on the hub alone
  grants: capabilities {"storage"}, hosts {}, storage 16777216 bytes, agent none
```

An unsigned warning is expected for this development check. A gate pass means
the bounded structural rules and artwork decoding passed. `hub test` also
loads the app under its resolved policy and exercises startup/shutdown in the
native host. Neither approves placeholders, checks privacy-policy contents or
replaces visual and interaction review. The untouched Card starter fails
because its declared screenshot is absent.


Confirm the granted permissions match the app's behaviour and record native
checks separately. Keep the review packet out of the bundle: including it
changes the digest and can introduce development-only content.

## 6. Sign and submit

Follow [Signing](PUBLISHING.md#signing) and [Submitting](PUBLISHING.md#submitting).
Stamp before signing, verify with the publisher's public key, and change the
version for each update. Any edit after signing requires restamping and
signing again.

Submit the app's source reference and signed bundle, not changes to the
production catalog or the Hub's signing keys. Keep source, authoring
instructions and tests in your app repository.
