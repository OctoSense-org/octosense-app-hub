use octosense_app_runtime::{CardRuntime, NativeEvent};

const CARD: &str = "state selected { shape: enum[off, on], initial: .off }\n\
event flip { selected: cycle(.off, .on) }\n\
view root Row(on_tap: flip) { TextBody(text: selected) }";

fn contains_text(node: &octoscript_ui_l0::UiNode, text: &str) -> bool {
    node.args.iter().any(|(name, value)| name == "text" && matches!(value, octoscript_ui_l0::NodeValue::Text(value) if value == text))
        || node.children.iter().any(|child| contains_text(child, text))
}

#[test]
fn declared_event_updates_rendered_card_state() {
    let mut runtime = CardRuntime::new(CARD, serde_json::json!({})).unwrap();
    assert!(contains_text(&runtime.render().unwrap(), "off"));
    assert_eq!(runtime.state("@card", "selected"), None);
    let event = NativeEvent::new(runtime.generation(), "root", "flip", None);
    let result = runtime.dispatch_native(event).unwrap();
    assert!(result.applied);
    assert_eq!(runtime.state("@card", "selected").and_then(|v| v.as_str()), Some("on"));
    assert!(contains_text(&runtime.render().unwrap(), "on"));
    assert!(!runtime.dispatch_native(NativeEvent::new(runtime.generation(), "root", "invented", None)).unwrap().applied);
    assert_eq!(runtime.state("@card", "selected").and_then(|v| v.as_str()), Some("on"));
}

#[test]
fn stale_instance_event_cannot_change_a_new_card() {
    let first = CardRuntime::new(CARD, serde_json::json!({})).unwrap();
    let old = NativeEvent::new(first.generation(), "root", "flip", None);
    let mut replacement = CardRuntime::new(CARD, serde_json::json!({})).unwrap();
    assert!(!replacement.dispatch_native(old).unwrap().applied);
    assert_eq!(replacement.state("@card", "selected"), None);
}

#[test]
fn saved_note_survives_process_restart() {
    let mut first = CardRuntime::new(CARD, serde_json::json!({})).unwrap();
    let event = NativeEvent::new(first.generation(), "root", "flip", None);
    assert!(first.dispatch_native(event).unwrap().applied);
    let bytes = first.snapshot_bytes().unwrap();
    let mut restarted = CardRuntime::from_snapshot(CARD, serde_json::json!({}), &bytes).unwrap();
    assert!(contains_text(&restarted.render().unwrap(), "on"));
    assert!(!restarted.dispatch_native(NativeEvent::new(first.generation(), "root", "flip", None)).unwrap().applied);
}

#[test]
fn unsupported_durable_effect_does_not_commit_state() {
    let card = "source movers sys.movers(count: 5, fields: [ticker, name])\n\
        source watch sys.watchlist(fields: [ticker, name])\n\
        event keep { watch: append($value) }\n\
        view root Surface { for m, i in movers key m.ticker {\n\
            Row(on_tap: keep, value: m.ticker) { TextBody(text: m.ticker) }\n\
        }}";
    let mut runtime = CardRuntime::new(card, serde_json::json!({"movers":[{"ticker":"AAA","name":"Alpha"}],"watch":[]})).unwrap_or_else(|e| {
        panic!("{e}: {:?}", octoscript_ui_l0::check_ui_l0(card).diagnostics)
    });
    let before = runtime.snapshot_bytes().unwrap();
    fn tap(node: &octoscript_ui_l0::UiNode) -> Option<(String, serde_json::Value)> {
        let event = node.args.iter().find(|(name, _)| name == "on_tap");
        if matches!(event, Some((_, octoscript_ui_l0::NodeValue::Event(name))) if name == "keep") {
            let value = node.args.iter().find(|(name, _)| name == "value")?.1.clone();
            if let octoscript_ui_l0::NodeValue::Text(value) = value {
                return Some((node.key.clone(), value.into()));
            }
        }
        node.children.iter().find_map(tap)
    }
    let (key, value) = tap(&runtime.render().unwrap()).unwrap();
    let event = NativeEvent::new(runtime.generation(), &key, "keep", Some(value));
    assert!(runtime.dispatch_native(event).is_err());
    assert_eq!(runtime.snapshot_bytes().unwrap(), before);
}
