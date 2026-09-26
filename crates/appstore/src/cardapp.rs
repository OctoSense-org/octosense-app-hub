//! An installed card app as an app of its own.
//!
//! The store installs; this runs. One module, `card`, opened with the id of
//! an installed app, becomes that app's window-manager client: its own tile,
//! its own entry in recents, its own close. It re-checks the app against the
//! last verified catalog before drawing anything, so a withdrawal reaches an
//! app that was installed long ago, with no store screen involved.
//!
//! Everything about containment is the same as the store's own runner was:
//! the policy resolved from the manifest is applied to the isolate, the
//! artwork is served from a loopback origin that serves only this bundle,
//! and that origin's port is the one loopback entry the isolate may reach.
use makepad_app_module::{
    makepad_ai_services::wire::{ServiceCall, ServiceManifest, ToolResult},
    AppModule, ExecOutcome, InstanceHandles, InstanceParts, OpenArgKind, OpenSchema, ServiceExecutor, ValidatedOpen,
};
use makepad_widgets::*;
use makepad_widgets::makepad_platform::thread::{SignalToUI, ThreadOptions};
use octosense_app_hub::{PreparedLaunch, Store};
use octosense_app_policy::HostLimits;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};

type CatalogGuard = Box<dyn Fn(u64) -> Result<(), String>>;

script_mod! {
    use mod.prelude.widgets.*

    mod.widgets.CardAppView = set_type_default() do #(CardAppView::register_widget(vm)) {
        width: Fill height: Fill flow: Overlay
        show_bg: true draw_bg.color: #fff
        body := View { width: Fill height: Fill flow: Down
            notice := Label { width: Fill text: "" draw_text.color: #b00 draw_text.text_style.font_size: 12 margin: 16 }
            card := Splash { width: Fill height: Fill }
        }
        // A host service's sheet over the app (services.rs): its own isolate,
        // under no app's policy, where the person types what the app must
        // never see.
        sheet := Splash { visible: false width: Fill height: Fill }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct CardAppView {
    #[deref]
    view: View,
    #[rust]
    app_id: String,
    #[rust]
    started: bool,
    #[rust]
    asset_server: Option<octosense_app_policy::AssetServer>,
    #[rust]
    pending: Option<Receiver<Result<PreparedLaunch, String>>>,
    #[rust]
    running_release: Option<(String, String)>,
    #[rust]
    catalog_guard: Option<CatalogGuard>,
    #[rust]
    prepared: Option<PreparedLaunch>,
    #[rust]
    host_dir: PathBuf,
}

impl CardAppView {
    pub fn running_release(&self) -> Option<(String, String)> {
        self.running_release.clone()
    }

    /// Reject stale disk state before any installed app code is loaded.
    pub fn set_catalog_guard(&mut self, guard: Box<dyn Fn(u64) -> Result<(), String>>) {
        self.catalog_guard = Some(guard);
    }

    fn start(&mut self, cx: &mut Cx) {
        let root = crate::data_root(cx);
        // `.host` can never be an app id, so the host service keeps its own
        // files separate from each contained app's jail.
        self.host_dir = root.join(".host");
        if let Some(app) = crate::system::system_app(&self.app_id) {
            match crate::system::prepare(&root, &app) {
                Ok((bundle, policy)) => self.mount(cx, &policy, &bundle, app.assets, None),
                Err(e) => self.refuse(cx, &format!("Cannot open {}: {e}", app.name)),
            }
            return;
        }

        let anchor = std::env::var("OCTOSENSE_HUB_ANCHOR").unwrap_or_else(|_| crate::DEFAULT_ANCHOR.to_string());
        let app_id = self.app_id.clone();
        let (tx, rx) = mpsc::channel();
        self.view.label(cx, ids!(notice)).set_text(cx, "Checking installed app…");
        match cx.thread_spawner().spawn_worker(ThreadOptions::default(), move || {
            let result = (|| {
                let mut store = Store::with_runtime(&anchor, &root, HostLimits::default(), octosense_app_policy::compatibility::RuntimeDescriptor::current(), crate::source::configured_format());
                let catalog = std::fs::read_to_string(root.join(store.format().cache_filename())).map_err(|e| format!("no cached catalog: {e}"))?;
                store.accept_catalog(&catalog)?;
                store.prepare_launch(&app_id)
            })();
            let _ = tx.send(result);
            SignalToUI::set_ui_signal();
        }) {
            Ok(handle) => { handle.detach(); self.pending = Some(rx); }
            Err(e) => self.refuse(cx, &format!("Cannot verify the app: {e:?}")),
        }
    }

    fn finish_start(&mut self, cx: &mut Cx, prepared: PreparedLaunch) {
        let root = crate::data_root(cx);
        let anchor = std::env::var("OCTOSENSE_HUB_ANCHOR").unwrap_or_else(|_| crate::DEFAULT_ANCHOR.to_string());
        // Hashing ran on the worker. Recheck authenticated metadata on the UI
        // thread before loading the already-owned code snapshot.
        let current = (|| {
            let mut store = Store::with_runtime(&anchor, &root, HostLimits::default(), octosense_app_policy::compatibility::RuntimeDescriptor::current(), crate::source::configured_format());
            store.accept_catalog(&std::fs::read_to_string(root.join(store.format().cache_filename())).map_err(|e| e.to_string())?)?;
            if let Some(guard) = &self.catalog_guard { guard(store.catalog().unwrap().sequence)?; }
            store.validate_prepared_launch(&prepared)
        })();
        if let Err(e) = current { return self.refuse(cx, &format!("Cannot open: {e}")); }
        let policy = prepared.policy.clone();
        let bundle = prepared.bundle().to_path_buf();
        self.mount(cx, &policy, &bundle, &[] as octosense_app_policy::StaticAssets, Some(prepared));
    }

    fn mount(&mut self, cx: &mut Cx, policy: &octosense_app_policy::AppPolicy, bundle: &std::path::Path,
             statics: octosense_app_policy::StaticAssets, prepared: Option<PreparedLaunch>) {
        let root = crate::data_root(cx);
        let mut settings = policy.isolate_settings(&root);
        if let Err(e) = std::fs::create_dir_all(&settings.jail_root) {
            return self.refuse(cx, &format!("Cannot make the app's storage: {e}"));
        }
        let server = match octosense_app_policy::AssetServer::start_with_static(bundle, statics) {
            Ok(server) => server,
            Err(e) => return self.refuse(cx, &format!("Cannot serve the app's artwork: {e}")),
        };
        settings.hosts.push(server.allowlist_entry());
        let origin = server.origin().to_string();
        self.asset_server = Some(server);
        let splash = self.view.splash(cx, ids!(card));
        let applied = octosense_app_policy::splash_adapter::apply(&splash, cx, &settings);
        log!(
            "card: {} running under {} capability(ies), {} host(s), {} bytes of storage, {} instructions, {} bytes of heap",
            self.app_id, applied.capabilities, applied.hosts, applied.storage_quota, applied.instruction_budget, applied.memory_bytes
        );
        match crate::card_source(bundle, &origin) {
            Ok(source) => splash.set_text(cx, &source),
            Err(e) => return self.refuse(cx, &format!("The app did not open: {e}")),
        }
        self.prepared = prepared;
        self.running_release = Some((policy.app_id.clone(), policy.version.clone()));
        self.view.label(cx, ids!(notice)).set_text(cx, "");
    }

    fn refuse(&mut self, cx: &mut Cx, reason: &str) {
        self.running_release = None;
        self.asset_server = None;
        self.prepared = None;
        error!("card: {reason}");
        self.view.label(cx, ids!(notice)).set_text(cx, reason);
        self.view.redraw(cx);
    }
}

impl Widget for CardAppView {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.started {
            self.started = true;
            self.start(cx);
        }
        if let Some(result) = self.pending.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.pending = None;
            match result {
                Ok(prepared) => self.finish_start(cx, prepared),
                Err(e) => self.refuse(cx, &format!("Cannot open: {e}")),
            }
        }
        let (card, sheet) = (self.view.splash(cx, ids!(card)), self.view.splash(cx, ids!(sheet)));
        let sheet_up = sheet.borrow().map(|s| s.view.visible).unwrap_or(false);
        if sheet_up && event.requires_visibility() {
            sheet.handle_event(cx, event, scope);
        } else {
            self.view.handle_event(cx, event, scope);
        }
        crate::services::pump(cx, &self.app_id, &self.host_dir, &card, &sheet);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

pub struct CardModule;
pub static CARD_MODULE: CardModule = CardModule;

impl AppModule for CardModule {
    fn id(&self) -> &'static str {
        "card"
    }
    fn label(&self) -> &'static str {
        "Card app"
    }
    /// The module itself asks for nothing; each app it runs is bounded by
    /// its own manifest, resolved and enforced per isolate.
    fn capabilities(&self) -> &'static [&'static str] {
        &[]
    }
    fn open_schema(&self) -> OpenSchema {
        OpenSchema::new(1).arg("app", OpenArgKind::Text, true)
    }
    fn register(&self, vm: &mut ScriptVm) {
        octoscript_widgets::design::script_mod(vm);
        octoscript_widgets::kit::script_mod(vm);
        script_mod(vm);
        crate::register_card_vocabulary();
    }
    fn create(&self, vm: &mut ScriptVm, open: ValidatedOpen, _handles: InstanceHandles) -> InstanceParts {
        let value = script_eval!(vm, { use mod.widgets.* CardAppView {} });
        let root = WidgetRef::script_from_value(vm, value);
        if let Some(mut view) = root.borrow_mut::<CardAppView>() {
            view.app_id = open.text("app").unwrap_or_default().to_string();
        }
        let closing = root.clone();
        InstanceParts {
            root,
            executor: Box::new(CardExecutor),
            // The camera and any web views the app opened go when the app
            // does, not whenever its isolate is next collected.
            shutdown: Box::new(move |vm| {
                let cx = vm.cx_mut();
                let splash = closing.splash(cx, ids!(card));
                let heap = splash.borrow_mut().and_then(|mut s| s.isolate_heap_key(cx));
                if let Some(heap) = heap {
                    makepad_widgets::camera_preview::release_isolate_devices(cx, heap);
                }
            }),
        }
    }
}

struct CardExecutor;

impl ServiceExecutor for CardExecutor {
    fn manifest(&self) -> ServiceManifest {
        ServiceManifest::new("card", "Card app", "An installed card app, running in its own contained isolate.")
    }
    fn execute(&mut self, _cx: &mut Cx, call: &ServiceCall) -> ExecOutcome {
        ExecOutcome::Done(ToolResult::unavailable(&call.call_id, "A card app offers no tools to the assistant yet"))
    }
    fn cancel(&mut self, _cx: &mut Cx, _call_id: &str) {}
    fn subscribe(&mut self, _cx: &mut Cx, _sub_id: &str, _topic: &str, _filter: Option<&str>) {}
    fn unsubscribe(&mut self, _cx: &mut Cx, _sub_id: &str) {}
}

#[allow(dead_code)]
fn _unused(_: PathBuf) {}
