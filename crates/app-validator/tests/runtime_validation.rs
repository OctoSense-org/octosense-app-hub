#[path = "../../app-hub/tests/common/mod.rs"]
mod common;
use common::Fixture;
use std::{fs, process::Command};

#[test]
fn valid_template_prepares_with_installed_host_path() {
    let f = Fixture::new();
    let source = octosense_app_validator::card_source(&f.bundle, "http://127.0.0.1:1/").unwrap();
    assert!(source.contains("Focus Timer"));
    let result = Command::new(env!("CARGO_BIN_EXE_app-validator")).arg(&f.bundle).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

#[test]
fn script_bundle_prepares_without_a_card() {
    let mut f = Fixture::new();
    fs::remove_file(f.bundle.join("page.card")).unwrap();
    fs::remove_file(f.bundle.join("page.data.json")).unwrap();
    fs::remove_dir_all(f.bundle.join("kit")).unwrap();
    fs::write(f.bundle.join("main.splash"), "HostedView{width: Fill height: Fill full: View{width: Fill height: Fill Label{text: \"Hello\"}} tile: View{width: Fill height: Fill Label{text: \"Hello\"}}}").unwrap();
    f.sign();
    assert!(f.report(None).passed());
    let source = octosense_app_validator::card_source(&f.bundle, "http://127.0.0.1:1/").unwrap();
    assert!(source.contains("Hello"));
    let result = Command::new(env!("CARGO_BIN_EXE_app-validator")).arg(&f.bundle).output().unwrap();
    assert!(result.status.success(), "{} {}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
}

#[test]
fn unresolved_runtime_component_is_refused() {
    let mut f = Fixture::new();
    let path = f.bundle.join("kit/native/light/kit.json");
    let mut kit: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    kit["components"]["title"]["style"]["t"] = "not-a-native-widget".into();
    fs::write(path, serde_json::to_vec(&kit).unwrap()).unwrap();
    f.sign();
    assert!(f.report(None).passed(), "structural validation does not execute the renderer");
    let result = Command::new(env!("CARGO_BIN_EXE_app-validator")).arg(&f.bundle).output().unwrap();
    assert!(!result.status.success());
    let failure: serde_json::Value = serde_json::from_slice(&result.stdout).expect("worker failures must be JSON");
    assert_eq!(failure["passed"], false);
    assert_eq!(failure["findings"][0]["check"], "runtime-validation-failed");
}

#[test]
fn native_validation_uses_the_manifest_compute_limits() {
    for memory in [false, true] {
        let mut f = Fixture::new();
        if memory { f.manifest.compute.memory_bytes = Some(1); }
        else { f.manifest.compute.instruction_budget = Some(1); }
        f.sign();
        assert!(f.report(None).passed());
        let result = Command::new(env!("CARGO_BIN_EXE_app-validator")).arg(&f.bundle).output().unwrap();
        assert!(!result.status.success(), "a bundle that cannot fit its declared budget must fail native validation");
    }
}

#[test]
fn native_svg_geometry_and_resolved_fonts_are_checked() {
    for bad_font in [false, true] {
        let mut f = Fixture::new();
        if bad_font {
            let path = f.bundle.join("kit/native/light/kit.json");
            let mut kit: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            kit["tokens"]["font"] = serde_json::json!({"value":"missing.ttf"});
            kit["components"]["title"]["style"]["font_src"] = serde_json::json!({"$token":"font"});
            fs::write(path, serde_json::to_vec(&kit).unwrap()).unwrap();
        } else {
            fs::write(f.bundle.join("icon.svg"), r#"<svg width="64" height="64"><unsupported/></svg>"#).unwrap();
        }
        f.sign();
        assert!(f.report(None).passed());
        let result = Command::new(env!("CARGO_BIN_EXE_app-validator")).arg(&f.bundle).output().unwrap();
        assert!(!result.status.success(), "native-only resource checks must refuse this fixture");
    }
}

#[test]
fn runtime_evidence_binds_both_payload_and_complete_manifest() {
    let mut f = Fixture::new();
    let report = f.report(None);
    let evidence = octosense_app_hub::runtime::validate(&f.bundle, &report, std::path::Path::new(env!("CARGO_BIN_EXE_app-validator"))).unwrap();
    assert!(evidence.report().checks.iter().any(|c| c == "native-widget-load"));
    assert_eq!(evidence.validator_digest().len(), 64);
    evidence.verify_bundle(&f.bundle).unwrap();
    f.manifest.capabilities.push("prompt".into());
    f.sign();
    assert!(evidence.verify_bundle(&f.bundle).is_err(), "manifest permissions are bound even though payload digest excludes manifest");
    fs::write(f.bundle.join("extra.txt"), "edited after validation").unwrap();
    assert!(evidence.verify_bundle(&f.bundle).is_err());
}

#[test]
fn large_card_expansion_is_stopped_inside_the_worker() {
    let mut f = Fixture::new();
    let path = f.bundle.join("page.card");
    let original = fs::read_to_string(&path).unwrap();
    let declarations = original.split("view root FixtureSurface").next().unwrap();
    let source = format!("{declarations}\nstate blob {{ shape: text, initial: \"{}\" }}\nview root FixtureSurface(instance: \"page\") {{\n{}\n}}",
        "x".repeat(200_000), format!("FixtureSurface(instance: \"a\") {{ {} }}\nFixtureSurface(instance: \"b\") {{ {} }}", "Fixturetitle(text: blob)\n".repeat(700), "Fixturetitle(text: blob)\n".repeat(700)));
    assert!(source.len() < 256 * 1024);
    fs::write(path, source).unwrap();
    let data_path = f.bundle.join("page.data.json");
    let mut data: serde_json::Value = serde_json::from_slice(&fs::read(&data_path).unwrap()).unwrap();
    data["blob"] = "x".repeat(200_000).into();
    fs::write(data_path, serde_json::to_vec(&data).unwrap()).unwrap();
    f.sign();
    let gate = f.report(None);
    assert!(gate.passed(), "{}", gate.render());
    let error = octosense_app_hub::runtime::validate(&f.bundle, &gate, std::path::Path::new(env!("CARGO_BIN_EXE_app-validator"))).unwrap_err();
    assert!(error.contains("memory allocation") || error.contains("timed out"), "expected worker resource limit, got {error}");
}

#[cfg(unix)]
#[test]
fn wrong_runtime_or_incomplete_checks_cannot_create_evidence() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let (digest, manifest_digest) = octosense_app_hub::runtime::identity(&f.bundle).unwrap();
    let report = f.report(None);
    for (runtime, checks) in [("other-runtime", vec!["structural", "card-preparation", "native-widget-load", "startup-shutdown"]), (octosense_app_hub::runtime::CARD_RUNTIME, vec!["structural"])] {
        let validator = f.root.join("validator");
        let response = serde_json::json!({"schema":1,"check_version":1,"runtime":runtime,
            "target":format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),"digest":digest,"manifest_digest":manifest_digest,"checks":checks});
        fs::write(&validator, format!("#!/bin/sh\nprintf '%s' '{}'\n", response)).unwrap();
        fs::set_permissions(&validator, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(octosense_app_hub::runtime::validate(&f.bundle, &report, &validator).unwrap_err().contains("wrong bundle, runtime or checks"));
    }
}
