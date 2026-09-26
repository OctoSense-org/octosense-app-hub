//! Run one card bundle under its own policy.
//!
//! ```sh
//! card-host --bundle <dir> [--app-data <dir>] [--allow-unsigned] [--stamp] [--system]
//!           [--static <prefix>=<dir>]...
//! ```
//!
//! The bundle is a directory holding `manifest.json`, `page.card`,
//! `page.data.json` and a `kit/` directory. `--stamp` rewrites the manifest's
//! digest to match the directory, which is what a build step does before
//! signing; without it a bundle whose bytes changed is refused. `--system`
//! admits the bundle as a system app is admitted (by digest, under
//! `HostLimits::system`), for developing one; an empty digest in its
//! manifest is filled in memory, as the build fills it in the packed copy.
//! `--static photos=<dir>` serves `<dir>`'s files at `photos/...` from memory,
//! the way a shell serves a system app's compiled-in artwork.
//!
//! The order is the one ADR 0002 fixes: admit, resolve, apply, then evaluate.
//! Nothing here may widen what the manifest asked for, and the two settings
//! the runtime cannot enforce yet are printed rather than assumed.
use makepad_widgets::*;
use octosense_app_policy::{admit_and_resolve_dir, AppPolicy, HostLimits, RefuseAllSignatures};
use std::path::PathBuf;

app_main!(App, font_set: International);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    startup() do #(App::script_component(vm)) {
        ui: Root {
            main_window := Window {
                window.title: "Card host"
                window.inner_size: vec2(412, 892)
                pass +: { clear_color: #fff }
                body +: {
                    padding: 0 margin: 0 spacing: 0 flow: Overlay
                    card := Splash { width: Fill height: Fill }
                    sheet := Splash { visible: false width: Fill height: Fill }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    mounted: bool,
    #[rust]
    assets: Option<octosense_app_policy::AssetServer>,
    #[rust]
    session: Option<octosense_app_validator::CardSession>,
    #[rust]
    app_id: String,
    #[rust]
    host_dir: PathBuf,
}

struct Args {
    bundle: PathBuf,
    app_data: PathBuf,
    allow_unsigned: bool,
    stamp: bool,
    system: bool,
    statics: Vec<(String, PathBuf)>,
}

fn args() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let value = |name: &str| argv.windows(2).find(|w| w[0] == name).map(|w| w[1].clone());
    Args {
        bundle: value("--bundle").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")),
        app_data: value("--app-data")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("octosense-card-apps")),
        allow_unsigned: argv.iter().any(|a| a == "--allow-unsigned"),
        stamp: argv.iter().any(|a| a == "--stamp"),
        system: argv.iter().any(|a| a == "--system"),
        statics: argv
            .windows(2)
            .filter(|w| w[0] == "--static")
            .filter_map(|w| w[1].split_once('=').map(|(p, d)| (p.trim_matches('/').to_string(), PathBuf::from(d))))
            .collect(),
    }
}

/// Read `--static` directories into memory for the life of the process.
fn load_statics(mounts: &[(String, PathBuf)]) -> octosense_app_policy::StaticAssets {
    let mut out: Vec<(&'static str, &'static [u8])> = Vec::new();
    for (prefix, dir) in mounts {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
            out.push((Box::leak(name.into_boxed_str()), Box::leak(bytes.into_boxed_slice())));
        }
    }
    Box::leak(out.into_boxed_slice())
}

/// Admit the bundle and resolve what it gets. Refusals are fatal: a card that
/// cannot be admitted must not be drawn, not even partially.
fn policy_for(args: &Args) -> Result<AppPolicy, String> {
    let manifest_path = args.bundle.join(octosense_app_policy::MANIFEST_FILE);
    let mut manifest_json = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let digest = octosense_app_policy::digest_dir(&args.bundle)?;

    if args.stamp {
        let mut value: serde_json::Value = serde_json::from_str(&manifest_json).map_err(|e| e.to_string())?;
        value["integrity"]["bundle_blake3"] = serde_json::Value::String(digest.clone());
        manifest_json = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
        std::fs::write(&manifest_path, format!("{manifest_json}\n")).map_err(|e| e.to_string())?;
        log!("card-host: stamped {} with digest {}", manifest_path.display(), digest);
    }

    // A system app's source manifest leaves its digest empty: the build
    // stamps it into the packed copy. Developing one, stamp it in memory.
    if args.system && !args.stamp {
        let mut value: serde_json::Value = serde_json::from_str(&manifest_json).map_err(|e| e.to_string())?;
        if value["integrity"]["bundle_blake3"].as_str().unwrap_or("").is_empty() {
            value["integrity"]["bundle_blake3"] = serde_json::Value::String(digest.clone());
            manifest_json = serde_json::to_string(&value).map_err(|e| e.to_string())?;
        }
    }

    let limits = if args.system {
        HostLimits::system()
    } else {
        HostLimits { require_signature: !args.allow_unsigned, ..HostLimits::default() }
    };
    // The digest is computed from the directory, so the manifest's claim is
    // checked against what is actually there.
    admit_and_resolve_dir(&manifest_json, &digest, &limits, &RefuseAllSignatures)
}

/// Lower the card to isolate source: realize it, then lower it with the kit
/// that ships in the bundle. Nothing is read from outside the bundle.
/// Give every card isolate the kit vocabulary it needs to draw.
///
/// An isolate starts with the standard widgets only, so a lowered L0 card,
/// which names `DesignSurface`, `KitButton` and friends, does not evaluate
/// without this. Note the shape of the hook: it is PROCESS-WIDE, so every
/// isolate gets the same vocabulary. ADR 0002 phase 1 wants this per app,
/// from the manifest; until the runtime can do that, a host must choose one
/// vocabulary for all its cards and keep it to drawing.
fn register_card_vocabulary() {
    use makepad_widgets::widget_async::register_splash_isolate_mod;
    fn design(vm: &mut ScriptVm) {
        octoscript_widgets::design::script_mod(vm);
    }
    fn kit(vm: &mut ScriptVm) {
        octoscript_widgets::kit::script_mod(vm);
    }
    fn tap(vm: &mut ScriptVm) {
        octoscript_widgets::tap::script_mod(vm);
    }
    register_splash_isolate_mod(design);
    register_splash_isolate_mod(kit);
    register_splash_isolate_mod(tap);
    register_splash_isolate_mod(makepad_widgets::splash::register_agent_module);
}

impl App {
    fn mount(&mut self, cx: &mut Cx) {
        let args = args();
        let policy = match policy_for(&args) {
            Ok(policy) => policy,
            Err(e) => {
                error!("card-host: refused: {e}");
                return;
            }
        };
        log!(
            "card-host: {} {} admitted — capabilities {:?}, hosts {:?}, storage {} bytes, agent {}",
            policy.app_id,
            policy.version,
            policy.capabilities,
            policy.hosts,
            policy.storage_bytes,
            policy.agent.as_ref().map(|a| a.profile.as_kernel_mode()).unwrap_or("none"),
        );

        self.app_id = policy.app_id.clone();
        self.host_dir = args.app_data.join(".host");
        let mut settings = policy.isolate_settings(&args.app_data);
        if let Err(e) = std::fs::create_dir_all(&settings.jail_root) {
            error!("card-host: cannot make the app's jail at {}: {e}", settings.jail_root.display());
            return;
        }
        // The app's artwork is served from a loopback origin of our own,
        // serving only its bundle, and that origin — this port, no other — is
        // the one loopback entry the isolate may reach.
        let server = match octosense_app_policy::AssetServer::start_with_static(&args.bundle, load_statics(&args.statics)) {
            Ok(server) => server,
            Err(e) => {
                error!("card-host: cannot serve the app's artwork: {e}");
                return;
            }
        };
        settings.hosts.push(server.allowlist_entry());
        let origin = server.origin().to_string();
        self.assets = Some(server);
        let state = policy.allows("storage").then(|| octosense_app_validator::state_path(&args.app_data, &policy.app_id));
        let storage = state.as_deref().map(|path| (path, policy.storage_bytes));
        let session = match octosense_app_validator::CardSession::open_with_state(&args.bundle, &origin, storage) {
            Ok(session) => session,
            Err(e) => { error!("card-host: the card did not lower: {e}"); return; }
        };
        if session.needs_event_channel() { settings.capabilities.push("agent.notify".into()); }
        let splash = self.ui.splash(cx, ids!(card));
        let applied = octosense_app_policy::splash_adapter::apply(&splash, cx, &settings);
        log!(
            "card-host: isolate jailed at {} with {} bytes, {} capability(ies), {} host(s), {} instructions, {} bytes of heap, prompts {} — all enforced",
            settings.jail_root.display(),
            applied.storage_quota,
            applied.capabilities,
            applied.hosts,
            applied.instruction_budget,
            applied.memory_bytes,
            settings.host_prompts,
        );

        // The session profile this app's agent would run under. Printed here
        // so the two containers can be compared at a glance; asking the kernel
        // for it is the next phase.
        if let Some(session) = policy.session_profile(&args.app_data) {
            match serde_json::to_string(&session) {
                Ok(json) => log!("card-host: agent session profile {json}"),
                Err(e) => error!("card-host: cannot render the session profile: {e}"),
            }
        }

        splash.set_text(cx, &session.source);
        self.session = Some(session);
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if !self.mounted {
            self.mounted = true;
            register_card_vocabulary();
            self.mount(cx);
        }
        self.ui.handle_event(cx, event, &mut Scope::empty());
        // Host services (a sheet the service raises, answers from its
        // workers), exactly as the Card runner does.
        let (card, sheet) = (self.ui.splash(cx, ids!(card)), self.ui.splash(cx, ids!(sheet)));
        if let Event::Actions(actions) = event {
            for action in actions {
                if let SplashAction::Notify { event_id, payload } = action.cast() {
                    let changed = self.session.as_mut().map(|session| session.dispatch_notify(&event_id, &payload));
                    match changed {
                        Some(Ok(Some(source))) => card.set_text(cx, &source),
                        Some(Err(e)) => {
                            error!("card-host: Card event failed: {e}");
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        octosense_appstore::services::pump(cx, &self.app_id, &self.host_dir, &card, &sheet);
    }
}
