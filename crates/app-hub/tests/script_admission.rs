mod common;
use common::Fixture;
use std::fs;

#[test]
fn signed_script_bundle_passes_without_a_card() {
    let mut fixture = Fixture::new();
    fs::remove_file(fixture.bundle.join("page.card")).unwrap();
    fs::remove_file(fixture.bundle.join("page.data.json")).unwrap();
    fs::remove_dir_all(fixture.bundle.join("kit")).unwrap();
    fs::write(fixture.bundle.join("main.splash"), "Label{text: \"Hello\"}").unwrap();
    fixture.sign();

    let report = fixture.report(None);
    assert!(report.passed(), "{}", report.render());
}

#[test]
fn v2_entrypoint_must_match_the_bundle_program() {
    let mut fixture = Fixture::new();
    let mut value = serde_json::to_value(&fixture.manifest).unwrap();
    value["schema"] = serde_json::json!(2);
    value["release_number"] = serde_json::json!(1);
    value["runtime"] = serde_json::json!({"api":"1","min_build":1,"platforms":[std::env::consts::OS]});
    value["requires"] = serde_json::json!(["script.ui@1"]);
    value["entrypoints"] = serde_json::json!({"ui":"main.splash"});
    value["data_schema"] = serde_json::json!(1);
    fixture.manifest = octosense_app_policy::AppManifest::parse(&value.to_string()).unwrap();
    fixture.sign();
    let report = fixture.report(None);
    assert!(!report.passed(), "a card cannot satisfy a signed script-app entrypoint");
}
