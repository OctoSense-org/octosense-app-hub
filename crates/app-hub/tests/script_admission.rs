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
