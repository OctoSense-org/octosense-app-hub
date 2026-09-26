# octosense-app-hub-app — the shell integration

Every OctoSense shell (ROM Home, OctoSense-Desktop) links this one crate to
get the App Hub: the native store, the runner for Card and script apps, the
system apps the shell ships, the apps the user installed, and their icons. It
lives here, with the crates it builds on, so that every shell links the same
code at the same pin.

## What a shell gets

| Item | What it is |
| --- | --- |
| `APP_HUB_MODULE` | The native App Hub store as an `AppModule` (id `apphub`): Today, Apps, Search, app details and Library, backed by the signed catalog. |
| `CARD_MODULE` | The Card runner as an `AppModule` (id `card`). It runs a system app (`os.<name>`) or an installed app (`hub:<manifest-id>`) as its own client, mounted below the shell's status area, and draws a host service's sheet (a sign-in) over the app. Registering it registers the system apps first. |
| `system_apps()` | The system apps this build ships (ADR 0004), each `{ id, name, pack, assets }`. The first call runs the generated `register_system_apps()`, which registers every selected pack and its compiled-in asset mounts with the runner. |
| `system_icon(short_id)` | A system app's own launcher art (`icon.svg` or `icon.png` in its bundle), by short id (`camera` for `os.camera`). |
| `installed_apps(root)`, `data_root_if_set()`, `data_root()`, `set_data_root()` | The apps App Hub installed, read fresh from the app-data root, so an install shows up without a restart. |
| `icons` | Installed-app icon lookup (`read_installed_icon`, `installed_icon_path`) and an icon `generation()` that bumps when icons change. |
| `take_completed_installs()`, `AppHubAction` | What the store finished installing, for the shell to refresh its launcher. |
| `APP_ICON_SVG` | App Hub's own store icon, declared in `listing.json` (`assets/icon.svg`). |

## Adopting it in a shell

1. Depend on it at the App Hub pin, and resolve its Makepad and Octoscript
   sources the way the rest of the shell does (the same `[patch]` sections
   this workspace's root `Cargo.toml` carries, pointed at the shell's
   checkouts, including `makepad-wm-theme`):

   ```toml
   [dependencies]
   octosense-app-hub-app = { git = "https://github.com/OctoSense-org/OctoSense-App-Hub", rev = "<pin>" }
   ```

2. Name the shell's system-app selection in `.cargo/config.toml`:

   ```toml
   [env]
   OCTOSENSE_SYSTEM_APPS = { value = "system-apps.json", relative = true }
   ```

   The selection file names the apps and where their bundles are; `source`
   and asset paths are relative to the selection file's directory:

   ```json
   {
     "schema": 1,
     "source": "../.sources/system-apps/apps",
     "apps": ["news", "photos", "maps", "camera", "mail"],
     "assets": { "photos": { "photos": "apps/photos/resources/photos" } }
   }
   ```

   `source` is a checkout of
   [OctoSense-System-Apps](https://github.com/OctoSense-org/OctoSense-System-Apps)
   `apps/`; each `<source>/<name>/bundle` is packed with its digest stamped,
   and each asset directory is compiled in and served at `<prefix>/<file>`.
   The variable is required because this crate usually builds from cargo's
   git checkout, where its own location says nothing about the shell. With
   `relative = true` cargo passes an absolute path; a relative value set some
   other way resolves against the workspace holding the `Cargo.lock` above
   the target directory. **Unset, the build ships no system apps** and prints
   a `cargo:warning`.

3. Link the modules: add `APP_HUB_MODULE` and `CARD_MODULE` to the shell's
   module list, and list each of `system_apps()` and `installed_apps(..)` as a
   launcher row that opens `card` with that app.

4. Register the host services the system apps call through `host.request`
   (`octosense_appstore::services::register_host_service`, for example the
   Mail service's accounts) before the first system app opens.

5. Draw the service sheet: `CARD_MODULE` already wraps the runner in a host
   widget that draws a visible sheet over the app; a shell with its own card
   presentation does the same.

ROM Home is the reference: `home/src/apps.rs` (`system_card_apps`,
`register_host_services`, `card_apps`) and, for the sheet and mounting, the
`HostedHubCard` widget in this crate's `src/card_host.rs` (formerly
`home/apps/app-hub/src/card_host.rs`).

## Run it alone

```sh
cargo run --release -p octosense-app-hub-app --example preview
```

The preview logs built-in launch requests; the shell performs real launches.

## Exercise installation without publishing apps

Generate a fresh local signed catalog in an empty directory:

```sh
cargo run -p octosense-app-hub-app --example fixture -- target/app-hub/local-fixture
```

The generated `environment.json` holds `OCTOSENSE_HUB`,
`OCTOSENSE_HUB_ANCHOR` and `OCTOSENSE_APP_DATA`. Set those when launching the
shell to select the local catalog, its fresh trust anchor and an isolated
installation directory. The signing keys live in memory only. The fixture
includes Trail Notes and Focus Timer as real Card bundles with clearly marked
validation artwork. Never distribute a build configured with a fixture anchor.

## Checks

```sh
cargo test -p octosense-app-hub-app
cargo test -p octosense-app-hub-app --example fixture
```
