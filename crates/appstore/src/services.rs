//! Host services: what a contained app asks the shell to do for it.
//!
//! An app's script calls `host.request("mail.list", {…}, fn(r){…})`. The
//! isolate refuses the call unless the app's policy grants the family (`mail`);
//! what is granted is queued, and the Card runner hands it here. A service
//! registered for the family does the work, in Rust, with whatever it holds
//! that the app must not: a socket, a credential, a device. The app gets data
//! back, never the means.
//!
//! Some work needs the person, not the app: typing a password, approving an
//! account. A service raises a **sheet** for that: a host-owned surface the
//! runner draws over the app, in an isolate of its own under no app's policy.
//! Its script calls the same `host.request`, and those calls arrive marked
//! `from_sheet`, so a service accepts a password only from its own sheet and
//! never from the app.
//!
//! **Secrets are the host's.** A contained app never collects a password,
//! a PIN or a code: its password fields take no input (the runtime refuses
//! them in a policed isolate), and a service method that takes one lives
//! under `<family>.sheet.`, which [`dispatch`] accepts only from the sheet,
//! before any service sees the call. An app cannot open a sheet either:
//! only a service can, through [`ServiceHost`].
//!
//! Answers can come later, from a worker thread: [`Replier::send`] queues the
//! result and wakes the UI, and the runner delivers it to the isolate that
//! asked on its next event.
use makepad_widgets::makepad_platform::SignalToUI;
use makepad_widgets::{Cx, SplashRef};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

/// One request from an app (or from a service's sheet over it).
#[derive(Clone, Debug)]
pub struct ServiceCall {
    /// The app the request is for: its manifest id.
    pub app_id: String,
    /// The full service name, `family.method`.
    pub service: String,
    pub args: Value,
    /// It came from the service's own sheet, which the person typed into,
    /// not from the app.
    pub from_sheet: bool,
    /// A directory only the host can reach, for the service's own state
    /// (accounts, secrets, caches): outside every app's jail.
    pub host_dir: PathBuf,
}

impl ServiceCall {
    /// The part of the service name after the family: `list` in `mail.list`.
    pub fn method(&self) -> &str {
        self.service.split_once('.').map(|(_, m)| m).unwrap_or("")
    }
}

/// Where one request's answer goes. Cheap to move to a worker thread.
#[derive(Clone, Debug)]
pub struct Replier {
    heap_key: usize,
    req_id: u64,
}

static REPLIES: Mutex<Vec<(usize, u64, Result<String, String>)>> = Mutex::new(Vec::new());

impl Replier {
    pub fn send(self, result: Result<Value, String>) {
        let result = result.map(|value| value.to_string());
        REPLIES.lock().unwrap().push((self.heap_key, self.req_id, result));
        SignalToUI::set_ui_signal();
    }
}

/// What a service may ask of the runner that called it.
pub trait ServiceHost {
    /// Show a sheet over the app: `body` is a Splash program, run in a
    /// host-owned isolate. Replaces any sheet already up.
    fn open_sheet(&mut self, body: String);
    fn close_sheet(&mut self);
}

pub trait HostService: Send {
    /// The capability family this service answers: `mail`.
    fn family(&self) -> &'static str;
    fn call(&mut self, call: ServiceCall, reply: Replier, host: &mut dyn ServiceHost);
}

static SERVICES: Mutex<Vec<Box<dyn HostService>>> = Mutex::new(Vec::new());

/// Offer a service to every app granted its family. A second registration
/// for a family replaces the first.
pub fn register_host_service(service: Box<dyn HostService>) {
    let mut services = SERVICES.lock().unwrap();
    let family = service.family();
    services.retain(|s| s.family() != family);
    services.push(service);
}

pub fn has_service(family: &str) -> bool {
    SERVICES.lock().unwrap().iter().any(|s| s.family() == family)
}

/// Hand one request to the service for its family. No service: the app hears
/// so, rather than waiting forever.
pub fn dispatch(call: ServiceCall, heap_key: usize, req_id: u64, host: &mut dyn ServiceHost) {
    let reply = Replier { heap_key, req_id };
    let family = call.service.split('.').next().unwrap_or("").to_string();
    // What the person types on a sheet reaches only the sheet's methods,
    // whatever the service does: an app calling one is refused here.
    if call.method().starts_with("sheet.") && !call.from_sheet {
        reply.send(Err(format!("{} is for the host's sheet, not an app", call.service)));
        return;
    }
    let mut services = SERVICES.lock().unwrap();
    match services.iter_mut().find(|s| s.family() == family) {
        Some(service) => service.call(call, reply, host),
        None => {
            drop(services);
            reply.send(Err(format!("no service answers {family:?} on this device")));
        }
    }
}

/// The answers ready for these isolates, taken off the queue.
pub fn take_replies_for(heap_keys: &[usize]) -> Vec<(usize, u64, Result<String, String>)> {
    let mut replies = REPLIES.lock().unwrap();
    let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *replies).into_iter().partition(|(heap, _, _)| heap_keys.contains(heap));
    *replies = rest;
    mine
}

/// The sheet changes a service asked for during one dispatch, applied after.
#[derive(Default)]
struct SheetOps {
    change: Option<Option<String>>,
}

impl ServiceHost for SheetOps {
    fn open_sheet(&mut self, body: String) {
        self.change = Some(Some(body));
    }
    fn close_sheet(&mut self) {
        self.change = Some(None);
    }
}

/// Sheet closes asked for from a worker thread, which has no ServiceHost at
/// hand, by app: applied by the next pump for that app. Every running app
/// pumps (a home screen shows several), so one app must not take another's.
static PENDING_CLOSE: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Close the sheet over `app_id`, from a service's worker (a sign-in that
/// finished).
pub fn close_sheet_later(app_id: &str) {
    PENDING_CLOSE.lock().unwrap().push(app_id.to_string());
    SignalToUI::set_ui_signal();
}

fn heap_of(cx: &mut Cx, splash: &SplashRef, only_visible: bool) -> Option<usize> {
    if only_visible && !splash.borrow().map(|s| s.view.visible).unwrap_or(false) {
        return None;
    }
    let mut s = splash.borrow_mut()?;
    s.isolate_heap_key(cx)
}

fn apply_sheet(cx: &mut Cx, sheet: &SplashRef, change: Option<String>) {
    static OPENED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let up = change.is_some();
    match change {
        Some(body) => {
            // Every sheet starts empty: the same body again would keep the
            // last one's fields, a password typed into a cancelled sign-in
            // among them. The counter makes each opening a new program.
            let n = OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            sheet.set_text(cx, &format!("let sheet_opening = {n}\n{body}"));
        }
        None => sheet.set_text(cx, ""),
    }
    // After the text: a new program, or tearing the old one down, replaces
    // the sheet's view, and the replacement is visible. A sheet left
    // visible and empty would stay modal and take every touch meant for
    // the app.
    if let Some(mut s) = sheet.borrow_mut() {
        s.view.visible = up;
    }
    cx.redraw_all();
}

/// One turn of a host running an app: hand the app's (and its sheet's) host
/// requests to the services, apply the sheets they raise, and deliver the
/// answers that are ready. Call it on every event the host sees.
pub fn pump(cx: &mut Cx, app_id: &str, host_dir: &std::path::Path, card: &SplashRef, sheet: &SplashRef) {
    // An answer runs the app's script, which may re-render a list, and a
    // redraw asked for during a draw is dropped: the new rows would take
    // taps without ever being drawn. Everything queued here signalled the
    // UI when it was queued, so it waits for that event instead.
    if cx.in_draw_event() {
        return;
    }
    let app = heap_of(cx, card, false);
    let sheet_heap = heap_of(cx, sheet, true);
    let heaps: Vec<usize> = app.into_iter().chain(sheet_heap).collect();
    if heaps.is_empty() {
        return;
    }
    for request in makepad_widgets::splash_host::take_splash_host_requests_for(&heaps) {
        let call = ServiceCall {
            app_id: app_id.to_string(),
            service: request.service.clone(),
            args: serde_json::from_str(&request.args_json).unwrap_or(Value::Null),
            from_sheet: Some(request.heap_key) == sheet_heap,
            host_dir: host_dir.to_path_buf(),
        };
        let mut ops = SheetOps::default();
        dispatch(call, request.heap_key, request.req_id, &mut ops);
        if let Some(change) = ops.change {
            apply_sheet(cx, sheet, change);
        }
    }
    for (heap, req_id, result) in take_replies_for(&heaps) {
        let answer = match &result {
            Ok(json) => Ok(json.as_str()),
            Err(error) => Err(error.as_str()),
        };
        makepad_widgets::splash_host::splash_host_respond(cx, heap, req_id, answer);
    }
    let closing = {
        let mut pending = PENDING_CLOSE.lock().unwrap();
        let before = pending.len();
        pending.retain(|app| app != app_id);
        pending.len() != before
    };
    if closing {
        apply_sheet(cx, sheet, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl HostService for Echo {
        fn family(&self) -> &'static str {
            "echo"
        }
        fn call(&mut self, call: ServiceCall, reply: Replier, host: &mut dyn ServiceHost) {
            if call.method() == "sheet" {
                host.open_sheet("Label{text: \"hi\"}".into());
            }
            reply.send(Ok(serde_json::json!({"method": call.method(), "sheet": call.from_sheet})));
        }
    }

    #[derive(Default)]
    struct Host {
        sheet: Option<String>,
    }
    impl ServiceHost for Host {
        fn open_sheet(&mut self, body: String) {
            self.sheet = Some(body);
        }
        fn close_sheet(&mut self) {
            self.sheet = None;
        }
    }

    fn call(service: &str) -> ServiceCall {
        ServiceCall { app_id: "os.demo".into(), service: service.into(), args: Value::Null, from_sheet: false, host_dir: std::env::temp_dir() }
    }

    #[test]
    fn a_request_reaches_its_familys_service_and_the_answer_waits_for_its_isolate() {
        register_host_service(Box::new(Echo));
        let mut host = Host::default();
        dispatch(call("echo.sheet"), 7001, 1, &mut host);
        dispatch(call("nobody.here"), 7002, 2, &mut host);
        assert!(host.sheet.is_some(), "a service can raise a sheet");
        let mine = take_replies_for(&[7001]);
        assert_eq!(mine.len(), 1);
        assert!(mine[0].2.as_ref().unwrap().contains("\"method\":\"sheet\""));
        let other = take_replies_for(&[7002]);
        assert!(other[0].2.as_ref().unwrap_err().contains("no service"), "an unanswered family fails, not hangs");
    }

    #[test]
    fn only_the_sheet_reaches_a_sheet_method() {
        register_host_service(Box::new(Echo));
        let mut host = Host::default();
        dispatch(call("echo.sheet.submit"), 7011, 1, &mut host);
        let refused = take_replies_for(&[7011]);
        assert!(refused[0].2.as_ref().unwrap_err().contains("for the host's sheet"), "an app is refused before the service sees it");
        let mut from_sheet = call("echo.sheet.submit");
        from_sheet.from_sheet = true;
        dispatch(from_sheet, 7012, 1, &mut host);
        assert!(take_replies_for(&[7012])[0].2.as_ref().unwrap().contains("\"sheet\":true"));
    }
}
