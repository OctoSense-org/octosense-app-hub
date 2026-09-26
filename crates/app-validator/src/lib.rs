//! Shared preparation used by installed Card hosts and the validator worker.
//! Run validation through the Hub's bounded process interface for untrusted input.
use std::path::Path;
use octosense_app_hub::admission::{read_text, MAX_TEXT_BYTES};

pub fn card_source(bundle: &Path, asset_origin: &str) -> Result<String, String> {
    if bundle.join(octosense_app_policy::SCRIPT_ENTRY).is_file() {
        let source = read_text(&bundle.join(octosense_app_policy::SCRIPT_ENTRY), MAX_TEXT_BYTES)?;
        return Ok(source.replace(octosense_app_policy::ASSETS_PLACEHOLDER, asset_origin.trim_end_matches('/')));
    }
    let card = read_text(&bundle.join("page.card"), 256 * 1024)?;
    let data_path = bundle.join("page.data.json");
    let text = if data_path.exists() { read_text(&data_path, MAX_TEXT_BYTES)? } else { "{}".into() };
    let mut data: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("page.data.json: {e}"))?;
    octosense_app_policy::rewrite_assets(&mut data, asset_origin);
    let prepared = octoscript_makepad::l0::prepare(&card, &data, &bundle.join("kit"))?;
    // Inspect the resolved renderer nodes, so properties hidden behind Card
    // bindings/tokens receive the same checks as literal asset paths.
    let mut nodes = vec![&prepared.tree];
    while let Some(node) = nodes.pop() {
        if let Some(source) = &node.attrs.src {
            let relative = source.strip_prefix(asset_origin).ok_or_else(|| format!("resource must use this bundle's asset origin: {source}"))?;
            octosense_app_hub::admission::safe_relative(relative)?;
            if !bundle.join(relative).is_file() { return Err(format!("missing resolved resource: {relative}")); }
        }
        if let Some(font) = &node.attrs.font_src {
            if font != "makepad_widgets:resources/Inter.ttf" {
                return Err(format!("font {font} is not supported by the installed Card resource loader; use makepad_widgets:resources/Inter.ttf"));
            }
        }
        nodes.extend(&node.children);
    }
    let ui = octoscript_makepad::design::to_makepad_ui(&prepared.tree)?;
    Ok(format!("width:Fill height:Fill flow:Overlay {ui}"))
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
    let source = card_source(bundle, "http://127.0.0.1:1/")?;
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    cx.with_vm(makepad_widgets::script_mod);
    widget_async::register_splash_isolate_mod(|vm| { octoscript_widgets::design::script_mod(vm); });
    widget_async::register_splash_isolate_mod(|vm| { octoscript_widgets::kit::script_mod(vm); });
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
    octosense_app_policy::splash_adapter::apply(&splash, &mut cx, &policy.isolate_settings(&jail.0));
    splash.set_text(&mut cx, &source);
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
