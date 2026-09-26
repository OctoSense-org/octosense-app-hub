//! Shared preparation used by installed Card hosts and the validator worker.
//! Run validation through the Hub's bounded process interface for untrusted input.
use std::path::Path;
use std::io::{Read, Write};
use octosense_app_hub::admission::{read_text, MAX_TEXT_BYTES};

pub fn card_source(bundle: &Path, asset_origin: &str) -> Result<String, String> {
    Ok(CardSession::open(bundle, asset_origin)?.source)
}

/// One mounted Card and its verified, host-owned event channel. Script and
/// native-kit bundles keep their existing source path; portable L0 controls
/// use the shared state machine and can only change state through this session.
pub struct CardSession {
    pub source: String,
    runtime: Option<octosense_app_runtime::CardRuntime>,
    channel: Option<String>,
    card: String,
    data: serde_json::Value,
    kit: std::path::PathBuf,
    bundle: std::path::PathBuf,
    asset_origin: String,
    state_path: Option<std::path::PathBuf>,
    state_limit: u64,
}

impl CardSession {
    pub fn open(bundle: &Path, asset_origin: &str) -> Result<Self, String> {
        Self::open_with_state(bundle, asset_origin, None)
    }

    /// State lives outside the app's writable jail; only the host may supply
    /// this path after resolving the bundle's storage capability.
    pub fn open_with_state(bundle: &Path, asset_origin: &str, storage: Option<(&Path, u64)>) -> Result<Self, String> {
    if bundle.join(octosense_app_policy::SCRIPT_ENTRY).is_file() {
        let source = read_text(&bundle.join(octosense_app_policy::SCRIPT_ENTRY), MAX_TEXT_BYTES)?;
        return Ok(Self { source: source.replace(octosense_app_policy::ASSETS_PLACEHOLDER, asset_origin.trim_end_matches('/')),
            runtime: None, channel: None, card: String::new(), data: serde_json::Value::Null,
            kit: bundle.join("kit"), bundle: bundle.into(), asset_origin: asset_origin.into(), state_path: None, state_limit: 0 });
    }
    let card = read_text(&bundle.join("page.card"), 256 * 1024)?;
    let data_path = bundle.join("page.data.json");
    let text = if data_path.exists() { read_text(&data_path, MAX_TEXT_BYTES)? } else { "{}".into() };
    let mut data: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("page.data.json: {e}"))?;
    octosense_app_policy::rewrite_assets(&mut data, asset_origin);
    let prepared = octoscript_makepad::l0::prepare(&card, &data, &bundle.join("kit"))?;
    validate_resources(&prepared.tree, bundle, asset_origin)?;
    let runtime = if prepared.native_components { None } else if let Some((path, limit)) = storage.filter(|(path, _)| path.is_file()) {
        Some(octosense_app_runtime::CardRuntime::from_snapshot(&card, data.clone(), &read_snapshot(path, limit)?)?)
    } else { Some(octosense_app_runtime::CardRuntime::new(&card, data.clone())?) };
    if let Some(runtime) = &runtime { runtime.snapshot_bytes()?; }
    let channel = if runtime.is_some() {
        let mut nonce = [0u8; 16];
        rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut nonce).map_err(|e| format!("Card event channel: {e}"))?;
        Some(format!("octosense-card-runtime:{}", nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()))
    } else { None };
    let ui = if let (Some(channel), Some(runtime)) = (&channel, &runtime) {
        let restored = octoscript_makepad::l0::prepare_with_state(&card, &data, &bundle.join("kit"), runtime.store())?;
        validate_resources(&restored.tree, bundle, asset_origin)?;
        octoscript_makepad::to_makepad_l0_ui_with_events(&restored.tree, channel)
    } else { octoscript_makepad::design::to_makepad_ui(&prepared.tree)? };
    Ok(Self { source: format!("width:Fill height:Fill flow:Overlay {ui}"), runtime, channel,
        card, data, kit: bundle.join("kit"), bundle: bundle.into(), asset_origin: asset_origin.into(),
        state_path: storage.map(|(path, _)| path.to_path_buf()), state_limit: storage.map(|(_, limit)| limit).unwrap_or(0) })
    }

    pub fn needs_event_channel(&self) -> bool { self.runtime.is_some() }
    pub fn event_channel(&self) -> Option<&str> { self.channel.as_deref() }

    /// Return updated Splash source only for a declared event on this mounted
    /// generation. Other notifications are left to the host's normal handler.
    pub fn dispatch_notify(&mut self, event_id: &str, payload: &str) -> Result<Option<String>, String> {
        let Some(runtime) = &mut self.runtime else { return Ok(None) };
        if Some(event_id) != self.channel.as_deref() { return Ok(None); }
        if payload.len() > 65_536 { return Err("Card event payload exceeds 64 KiB".into()); }
        let message: serde_json::Value = serde_json::from_str(payload).map_err(|e| format!("Card event payload: {e}"))?;
        let target = message.get("target").and_then(|v| v.as_str()).and_then(|v| v.strip_prefix("l0:"))
            .ok_or("Card event target is missing")?;
        let target: serde_json::Value = serde_json::from_str(target).map_err(|e| format!("Card event target: {e}"))?;
        let key = target.get("k").and_then(|v| v.as_str()).ok_or("Card event key is missing")?;
        let event = target.get("e").and_then(|v| v.as_str()).ok_or("Card event name is missing")?;
        let value = target.get("v").cloned();
        let before = runtime.snapshot_bytes()?;
        let outcome = runtime.dispatch_native(octosense_app_runtime::NativeEvent::new(runtime.generation(), key, event, value))?;
        if !outcome.applied { return Ok(None); }
        let updated = (|| {
            let prepared = octoscript_makepad::l0::prepare_with_state(&self.card, &self.data, &self.kit, runtime.store())?;
            validate_resources(&prepared.tree, &self.bundle, &self.asset_origin)?;
            let ui = octoscript_makepad::to_makepad_l0_ui_with_events(&prepared.tree, event_id);
            if let Some(path) = &self.state_path {
                let bytes = runtime.snapshot_bytes()?;
                if bytes.len() as u64 > self.state_limit { return Err("Card state exceeds the app's storage quota".into()); }
                write_snapshot(path, &bytes)?;
            }
            Ok::<_, String>(format!("width:Fill height:Fill flow:Overlay {ui}"))
        })();
        let source = match updated {
            Ok(source) => source,
            Err(e) => { runtime.restore_snapshot(&before)?; return Err(e); }
        };
        self.source = source.clone();
        Ok(Some(source))
    }
}

/// A stable filename independent of app-supplied path spelling.
pub fn state_path(app_data_root: &Path, app_id: &str) -> std::path::PathBuf {
    app_data_root.join(".host/card-state").join(format!("{}.json", blake3::hash(app_id.as_bytes()).to_hex()))
}

fn read_snapshot(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).map_err(|e| e.to_string())?.take(1_048_577)
        .read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 1_048_576 { return Err("Card state snapshot exceeds 1 MiB".into()); }
    if bytes.len() as u64 > limit { return Err("Card state exceeds the app's storage quota".into()); }
    Ok(bytes)
}

fn write_snapshot(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Card state path has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; 16];
    rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut nonce).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("{}.tmp", nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&temp, path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if let Err(e) = std::fs::File::open(parent).and_then(|dir| dir.sync_all()) {
            // The replacement already happened; rolling back memory here
            // would disagree with the file a restart can read.
            eprintln!("Card state directory sync failed after replace: {e}");
        }
        Ok::<_, String>(())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result
}

fn validate_resources(tree: &octoscript_render::UiNode, bundle: &Path, asset_origin: &str) -> Result<(), String> {
    // Inspect the resolved renderer nodes, so properties hidden behind Card
    // bindings/tokens receive the same checks as literal asset paths.
    let mut nodes = vec![tree];
    while let Some(node) = nodes.pop() {
        if let Some(source) = &node.attrs.src {
            let relative = source.strip_prefix(asset_origin).ok_or_else(|| format!("resource must use this bundle's asset origin: {source}"))?;
            octosense_app_hub::admission::safe_relative(relative)?;
            if !bundle.join(relative).is_file() { return Err(format!("missing resolved resource: {relative}")); }
        }
        if let Some(font) = node.attrs.font_src.as_ref().filter(|font| !font.is_empty()) {
            if font != "makepad_widgets:resources/Inter.ttf" {
                return Err(format!("font {font} is not supported by the installed Card resource loader; use makepad_widgets:resources/Inter.ttf"));
            }
        }
        nodes.extend(&node.children);
    }
    Ok(())
}

pub fn validate_native(bundle: &Path) -> Result<octosense_app_hub::runtime::RuntimeReport, String> {
    use makepad_widgets::*;
    use octosense_app_hub::{admission, runtime::*};
    let files = admission::inventory(bundle)?;
    let (findings, _) = admission::validate(bundle, &files);
    if !findings.is_empty() {
        return Err(serde_json::to_string(&findings).map_err(|e| e.to_string())?);
    }
    for file in &files {
        if file.path.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("svg")) {
            let text = read_text(&bundle.join(&file.path), MAX_TEXT_BYTES)?;
            let document = makepad_widgets::makepad_draw::svg::parse_svg(&text);
            if document.root.is_empty() || document.compute_bounds().is_none() {
                return Err(format!("{}: no supported drawable SVG geometry", file.path.display()));
            }
        }
    }
    let digest = octosense_app_policy::digest_dir(bundle)?;
    let manifest = octosense_app_policy::AppManifest::parse(&read_text(&bundle.join("manifest.json"), admission::MAX_MANIFEST_BYTES)?)?;
    if digest != manifest.integrity.bundle_blake3 { return Err("bundle digest mismatch".into()); }
    let manifest_digest = blake3::hash(&serde_json::to_vec(&manifest).map_err(|e| e.to_string())?).to_hex().to_string();
    // No asset server or network grant is needed for widget construction.
    // Actual image decoding was checked above; visual/device quality remains
    // a separate review step.
    let session = CardSession::open(bundle, "http://127.0.0.1:1/")?;
    let source = &session.source;
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    cx.with_vm(makepad_widgets::script_mod);
    widget_async::register_splash_isolate_mod(|vm| { octoscript_widgets::design::script_mod(vm); });
    widget_async::register_splash_isolate_mod(|vm| { octoscript_widgets::kit::script_mod(vm); });
    widget_async::register_splash_isolate_mod(|vm| { octoscript_widgets::tap::script_mod(vm); });
    widget_async::register_splash_isolate_mod(makepad_widgets::splash::register_agent_module);
    let root = cx.with_vm(|vm| {
        let value = script_eval!(vm, { use mod.widgets.* Splash {} });
        WidgetRef::script_from_value(vm, value)
    });
    let splash = root.as_splash();
    let policy = octosense_app_policy::policy::resolve(&manifest, &octosense_app_policy::HostLimits {
        require_signature: false, ..Default::default()
    }).map_err(|e| format!("runtime policy: {e:?}"))?;
    struct Jail(std::path::PathBuf);
    impl Drop for Jail { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let jail = std::env::temp_dir().join(format!("octosense-validator-data-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos()));
    std::fs::create_dir(&jail).map_err(|e| e.to_string())?;
    let jail = Jail(jail);
    let mut settings = policy.isolate_settings(&jail.0);
    if session.needs_event_channel() { settings.capabilities.push("agent.notify".into()); }
    octosense_app_policy::splash_adapter::apply(&splash, &mut cx, &settings);
    splash.set_text(&mut cx, source);
    let loaded = {
        let mut inner = splash.borrow_mut().ok_or("native Splash was not constructed")?;
        !inner.view.children.is_empty() && inner.isolate_heap_key(&mut cx).is_some_and(splash_policy::may_run)
    };
    if !loaded { return Err("native widget loading failed under the app's resolved compute limits".into()); }
    root.handle_event(&mut cx, &Event::Startup, &mut Scope::empty());
    root.handle_event(&mut cx, &Event::Shutdown, &mut Scope::empty());
    let running = splash.borrow_mut().and_then(|mut inner| inner.isolate_heap_key(&mut cx)).is_some_and(splash_policy::may_run);
    splash.set_text(&mut cx, "");
    drop(splash);
    drop(root);
    widget_async::gc_dead_splash_isolates(&mut cx);
    if !running { return Err("native lifecycle exhausted the app's resolved compute limits".into()); }
    Ok(RuntimeReport {
        schema: 1, check_version: CHECK_VERSION, runtime: CARD_RUNTIME.into(),
        target: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        digest, manifest_digest,
        checks: vec!["structural".into(), "card-preparation".into(), "native-widget-load".into(), "startup-shutdown".into()],
    })
}
