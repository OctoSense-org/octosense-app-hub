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
