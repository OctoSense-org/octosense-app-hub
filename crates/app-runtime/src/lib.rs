//! State and event boundary shared by installed, reference and validation hosts.
//! A Card names transitions; only a mounted native control may submit one.
use octoscript_ui_l0::{InstanceStore, RealizeLimits, UiNode};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_CARD_BYTES: usize = 256 * 1024;
const MAX_DATA_BYTES: usize = 1024 * 1024;
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

pub struct NativeEvent {
    generation: u64,
    key: String,
    event: String,
    payload: Option<Value>,
}

impl NativeEvent {
    pub fn new(generation: u64, key: &str, event: &str, payload: Option<Value>) -> Self {
        Self { generation, key: key.into(), event: event.into(), payload }
    }
}

pub struct DispatchResult {
    pub applied: bool,
    pub changed: Vec<String>,
    pub stale: Vec<String>,
    pub writes: Vec<octoscript_ui_l0::CollectionWrite>,
}

impl DispatchResult {
    fn ignored() -> Self { Self { applied: false, changed: Vec::new(), stale: Vec::new(), writes: Vec::new() } }
}

pub struct CardRuntime {
    source: String,
    data: Value,
    state: InstanceStore,
    generation: u64,
}

impl CardRuntime {
    pub fn new(source: &str, data: Value) -> Result<Self, String> {
        if source.len() > MAX_CARD_BYTES { return Err("Card source exceeds the runtime limit".into()); }
        if serde_json::to_vec(&data).map_err(|e| e.to_string())?.len() > MAX_DATA_BYTES {
            return Err("Card data exceeds the runtime limit".into());
        }
        let report = octoscript_ui_l0::check_ui_l0(source);
        if !report.valid { return Err("Card is outside the supported L0 profile".into()); }
        let mut runtime = Self { source: source.into(), data, state: InstanceStore::default(),
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed) };
        runtime.render()?;
        Ok(runtime)
    }

    pub fn generation(&self) -> u64 { self.generation }
    pub fn store(&self) -> &InstanceStore { &self.state }
    pub fn snapshot_bytes(&self) -> Result<Vec<u8>, String> { self.state.snapshot_bytes() }

    pub fn from_snapshot(source: &str, data: Value, bytes: &[u8]) -> Result<Self, String> {
        let mut runtime = Self::new(source, data)?;
        runtime.state = InstanceStore::from_snapshot_bytes(bytes)?;
        runtime.render()?;
        Ok(runtime)
    }

    /// Roll back a staged transition when its durable write failed.
    pub fn restore_snapshot(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.state = InstanceStore::from_snapshot_bytes(bytes)?;
        self.render()?;
        Ok(())
    }
    pub fn state(&self, key: &str, field: &str) -> Option<&Value> { self.state.get(key, field) }

    pub fn render(&mut self) -> Result<UiNode, String> {
        let report = octoscript_ui_l0::realize_with_state(&self.source, &self.data, &self.state, RealizeLimits::default());
        let root = report.complete_root()?.clone();
        for (field, value) in &report.captured { self.state.set_cell("@card", field, value.clone()); }
        self.state.prune(&report.live_keys);
        Ok(root)
    }

    pub fn dispatch_native(&mut self, event: NativeEvent) -> Result<DispatchResult, String> {
        if event.generation != self.generation || event.key.len() > 256 || event.event.len() > 256 {
            return Ok(DispatchResult::ignored());
        }
        if event.payload.as_ref().is_some_and(|payload| {
            serde_json::to_vec(payload).map_or(true, |bytes| bytes.len() > 64 * 1024)
        }) { return Err("native event payload exceeds the runtime limit".into()); }
        let current = octoscript_ui_l0::realize_with_state(&self.source, &self.data, &self.state, RealizeLimits::default());
        let root = current.complete_root()?;
        let Some(origin) = octoscript_ui_l0::event_payload_origin(root, &event.key, &event.event) else {
            return Ok(DispatchResult::ignored());
        };
        let mut next = self.state.clone();
        let outcome = octoscript_ui_l0::dispatch_reporting_with_origin(
            &self.source, &mut next, &event.key, &event.event, event.payload.as_ref(), &self.data, origin);
        if !outcome.applied { return Ok(DispatchResult::ignored()); }
        let rendered = octoscript_ui_l0::realize_with_state(&self.source, &self.data, &next, RealizeLimits::default());
        rendered.complete_root()?;
        for (field, value) in &rendered.captured { next.set_cell("@card", field, value.clone()); }
        next.prune(&rendered.live_keys);
        self.state = next;
        Ok(DispatchResult { applied: true, changed: outcome.changed, stale: outcome.stale, writes: outcome.writes })
    }
}
