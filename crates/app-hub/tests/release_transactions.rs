mod common;
use common::{Fixture, write_catalog};
use octosense_app_hub::*;
use std::{fs, process::Command};

fn renew(f: &Fixture, anchor: &HubKey, working: &HubKey, expected: u64, retry: &str) -> std::process::Output {
    operation(f, anchor, working, expected, retry, "renew")
}
fn operation(f: &Fixture, anchor: &HubKey, working: &HubKey, expected: u64, retry: &str, operation: &str) -> std::process::Output {
    let key = f.root.join("working.key");
    fs::write(&key, hex::encode(working.to_bytes())).unwrap();
    Command::new(env!("CARGO_BIN_EXE_hub")).args([
        "admin", operation, "--catalog", f.root.join("public/catalog.json").to_str().unwrap(),
        "--state-dir", f.root.join("release-state").to_str().unwrap(),
        "--anchor", &anchor.public_hex(), "--key", key.to_str().unwrap(),
        "--anchor-cert", &anchor.certify(&working.public_hex()).unwrap(),
        "--expected-sequence", &expected.to_string(), "--idempotency-key", retry,
    ]).output().unwrap()
}

#[test]
fn renewal_advances_sequence_and_date_without_changing_releases() {
    for populated in [false, true] {
        let f = Fixture::new();
        let anchor = HubKey::generate();
        let working = HubKey::generate();
        let entries = if populated { vec![f.entry()] } else { vec![] };
        let mut before = Catalog::new(7, "2026-09-01", entries);
        working.sign_catalog(&mut before, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
        let path = f.root.join("public/catalog.json");
        write_catalog(&path, &before);
        let output = renew(&f, &anchor, &working, 7, "daily-renewal");
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let after: Catalog = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(after.sequence, 8);
        assert_eq!(after.published, today());
        assert_eq!(serde_json::to_value(&after.entries).unwrap(), serde_json::to_value(before.entries).unwrap());
        verify_catalog(&after, &anchor.public_hex()).unwrap();
        assert!(renew(&f, &anchor, &working, 7, "daily-renewal").status.success(), "retry must return the same result");
        assert!(!renew(&f, &anchor, &working, 7, "different-request").status.success(), "stale compare-and-swap must fail");
        assert!(!renew(&f, &anchor, &working, 8, "daily-renewal").status.success(), "an idempotency key cannot change meaning");
    }
}

#[test]
fn renewal_refuses_bad_future_dates_and_invalid_signatures() {
    for (date, invalid) in [("2026-02-30", false), ("9999-01-01", false), ("2026-09-01", true)] {
        let f = Fixture::new();
        let anchor = HubKey::generate();
        let working = HubKey::generate();
        let mut catalog = Catalog::new(1, date, vec![]);
        working.sign_catalog(&mut catalog, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
        if invalid { catalog.sequence += 1; }
        let path = f.root.join("public/catalog.json");
        write_catalog(&path, &catalog);
        let before = fs::read(&path).unwrap();
        assert!(!renew(&f, &anchor, &working, catalog.sequence, "bad").status.success());
        assert_eq!(fs::read(path).unwrap(), before);
    }
}

#[test]
fn restore_never_replays_an_old_sequence_and_working_keys_can_rotate() {
    let f = Fixture::new();
    let anchor = HubKey::generate();
    let working = HubKey::generate();
    let rotated = HubKey::generate();
    let mut old = Catalog::new(12, "2026-09-01", vec![]);
    working.sign_catalog(&mut old, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
    let path = f.root.join("public/catalog.json");
    write_catalog(&path, &old);
    assert!(renew(&f, &anchor, &rotated, 12, "rotation").status.success());
    let latest: Catalog = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(latest.sequence, 13);
    assert_eq!(latest.key.unwrap().public, rotated.public_hex());
    write_catalog(&path, &old);
    assert!(!renew(&f, &anchor, &working, 12, "restored").status.success(), "restored catalog must reconcile with durable generation history");
    assert_eq!(serde_json::from_slice::<Catalog>(&fs::read(path).unwrap()).unwrap().sequence, 12);
}

#[test]
fn publication_requires_a_recorded_operator_review() {
    let f = Fixture::new();
    let working = HubKey::generate();
    let anchor = HubKey::generate();
    let key = f.root.join("working.key");
    fs::write(&key, hex::encode(working.to_bytes())).unwrap();
    let worker = f.protocol_worker();
    let output = Command::new(env!("CARGO_BIN_EXE_hub")).args([
        "publish", f.bundle.to_str().unwrap(), "--catalog", f.root.join("public/catalog.json").to_str().unwrap(),
        "--state-dir", f.root.join("state").to_str().unwrap(), "--expected-sequence", "0", "--idempotency-key", "release-one",
        "--publisher", "publisher-one", "--publisher-key", &format!("publisher-one={}", f.publisher.public_hex()),
        "--validator", worker.to_str().unwrap(), "--key", key.to_str().unwrap(),
        "--anchor", &anchor.public_hex(), "--anchor-cert", &anchor.certify(&working.public_hex()).unwrap(),
        "--out", f.root.join("public").to_str().unwrap(),
    ]).output().unwrap();
    assert!(!output.status.success(), "passing validators do not replace a review decision");
    assert!(!f.root.join("public/catalog.json").exists());
}

#[test]
fn operator_can_publish_v2_beside_an_unchanged_v1_catalog() {
    let mut f = Fixture::new();
    let mut value = serde_json::to_value(&f.manifest).unwrap();
    value["schema"] = serde_json::json!(2);
    value["release_number"] = serde_json::json!(1);
    value["runtime"] = serde_json::json!({"api":"1","min_build":1,"platforms":[std::env::consts::OS]});
    value["requires"] = serde_json::json!(["card.ui@1"]);
    value["entrypoints"] = serde_json::json!({"ui":"page.card"});
    value["data_schema"] = serde_json::json!(1);
    f.manifest = octosense_app_policy::AppManifest::parse(&value.to_string()).unwrap();
    f.sign();
    let anchor = HubKey::generate();
    let working = HubKey::generate();
    let key = f.root.join("working.key");
    fs::write(&key, hex::encode(working.to_bytes())).unwrap();
    let mut legacy = Catalog::new(50, &today(), vec![]);
    working.sign_catalog(&mut legacy, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
    let legacy_path = f.root.join("public/catalog.json");
    write_catalog(&legacy_path, &legacy);
    fs::create_dir_all(f.root.join("public/v2")).unwrap();
    let v2_path = f.root.join("public/v2/catalog.json");
    let worker = f.protocol_worker();
    let output = Command::new(env!("CARGO_BIN_EXE_hub")).args([
        "admin", "publish", f.bundle.to_str().unwrap(), "--catalog", v2_path.to_str().unwrap(),
        "--catalog-schema", "2", "--state-dir", f.root.join("v2-state").to_str().unwrap(),
        "--expected-sequence", "0", "--idempotency-key", "v2-release-one",
        "--publisher", "publisher-one", "--publisher-key", &format!("publisher-one={}", f.publisher.public_hex()),
        "--reviewed-by", "operator", "--review-id", "decision-one", "--validator", worker.to_str().unwrap(),
        "--key", key.to_str().unwrap(), "--anchor", &anchor.public_hex(),
        "--anchor-cert", &anchor.certify(&working.public_hex()).unwrap(),
    ]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let modern: Catalog = serde_json::from_slice(&fs::read(&v2_path).unwrap()).unwrap();
    assert_eq!((modern.schema, modern.sequence, modern.entries.len()), (2, 1, 1));
    verify_catalog(&modern, &anchor.public_hex()).unwrap();
    let status = Command::new(env!("CARGO_BIN_EXE_hub")).args([
        "admin", "status", "--catalog", v2_path.to_str().unwrap(), "--anchor", &anchor.public_hex(),
    ]).output().unwrap();
    assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
    let renewal = Command::new(env!("CARGO_BIN_EXE_hub")).args([
        "admin", "renew", "--catalog", v2_path.to_str().unwrap(), "--catalog-schema", "2",
        "--state-dir", f.root.join("v2-state").to_str().unwrap(), "--expected-sequence", "1",
        "--idempotency-key", "v2-renewal", "--anchor", &anchor.public_hex(),
        "--key", key.to_str().unwrap(), "--anchor-cert", &anchor.certify(&working.public_hex()).unwrap(),
    ]).output().unwrap();
    assert!(renewal.status.success(), "{}", String::from_utf8_lossy(&renewal.stderr));
    let renewed: Catalog = serde_json::from_slice(&fs::read(&v2_path).unwrap()).unwrap();
    assert_eq!((renewed.schema, renewed.sequence), (2, 2));
    assert!(release::ReleaseStore::open_format(&v2_path, &f.root.join("public/private-state"), &anchor.public_hex(), CatalogFormat::V2).is_err(),
        "v2 state cannot be stored anywhere under the public distribution root");
    assert_eq!(fs::read(&legacy_path).unwrap(), serde_json::to_vec_pretty(&legacy).unwrap());
}

#[test]
fn v2_publication_refuses_a_lower_release_number() {
    let mut f = Fixture::new();
    let anchor = HubKey::generate();
    let working = HubKey::generate();
    let key = f.root.join("working.key");
    fs::write(&key, hex::encode(working.to_bytes())).unwrap();
    let v2_path = f.root.join("public/v2/catalog.json");
    fs::create_dir_all(v2_path.parent().unwrap()).unwrap();
    let publish = |f: &Fixture, expected: u64, request: &str| {
        let worker = f.protocol_worker();
        Command::new(env!("CARGO_BIN_EXE_hub")).args([
            "admin", "publish", f.bundle.to_str().unwrap(), "--catalog", v2_path.to_str().unwrap(),
            "--catalog-schema", "2", "--state-dir", f.root.join("v2-state").to_str().unwrap(),
            "--expected-sequence", &expected.to_string(), "--idempotency-key", request,
            "--publisher", "publisher-one", "--publisher-key", &format!("publisher-one={}", f.publisher.public_hex()),
            "--reviewed-by", "operator", "--review-id", request, "--validator", worker.to_str().unwrap(),
            "--key", key.to_str().unwrap(), "--anchor", &anchor.public_hex(),
            "--anchor-cert", &anchor.certify(&working.public_hex()).unwrap(),
        ]).output().unwrap()
    };
    let mut value = serde_json::to_value(&f.manifest).unwrap();
    value["schema"] = serde_json::json!(2);
    value["release_number"] = serde_json::json!(20);
    value["runtime"] = serde_json::json!({"api":"1","min_build":1,"platforms":[std::env::consts::OS]});
    value["requires"] = serde_json::json!(["card.ui@1"]);
    value["entrypoints"] = serde_json::json!({"ui":"page.card"});
    value["data_schema"] = serde_json::json!(1);
    f.manifest = octosense_app_policy::AppManifest::parse(&value.to_string()).unwrap();
    f.sign();
    let first = publish(&f, 0, "first");
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    value["version"] = serde_json::json!("2.0.0");
    value["release_number"] = serde_json::json!(12);
    f.manifest = octosense_app_policy::AppManifest::parse(&value.to_string()).unwrap();
    f.sign();
    let lower = publish(&f, 1, "lower");
    assert!(!lower.status.success(), "release number 12 must not follow 20");
    let catalog: Catalog = serde_json::from_slice(&fs::read(&v2_path).unwrap()).unwrap();
    assert_eq!((catalog.sequence, catalog.entries.len()), (1, 1));
}

#[test]
fn v2_publication_preserves_v1_publisher_ownership() {
    let mut f = Fixture::new();
    let anchor = HubKey::generate();
    let working = HubKey::generate();
    let key = f.root.join("working.key");
    fs::write(&key, hex::encode(working.to_bytes())).unwrap();
    let legacy_entry = f.entry();
    let mut legacy = Catalog::new(1, &today(), vec![legacy_entry]);
    working.sign_catalog(&mut legacy, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
    write_catalog(&f.root.join("public/catalog.json"), &legacy);
    let mut value = serde_json::to_value(&f.manifest).unwrap();
    value["schema"] = serde_json::json!(2);
    value["version"] = serde_json::json!("2.0.0");
    value["release_number"] = serde_json::json!(2);
    value["runtime"] = serde_json::json!({"api":"1","min_build":1,"platforms":[std::env::consts::OS]});
    value["requires"] = serde_json::json!(["card.ui@1"]);
    value["entrypoints"] = serde_json::json!({"ui":"page.card"});
    value["data_schema"] = serde_json::json!(1);
    f.manifest = octosense_app_policy::AppManifest::parse(&value.to_string()).unwrap();
    f.publisher = HubKey::generate();
    f.sign();
    let v2_path = f.root.join("public/v2/catalog.json");
    fs::create_dir_all(v2_path.parent().unwrap()).unwrap();
    let worker = f.protocol_worker();
    let output = Command::new(env!("CARGO_BIN_EXE_hub")).args([
        "admin", "publish", f.bundle.to_str().unwrap(), "--catalog", v2_path.to_str().unwrap(),
        "--catalog-schema", "2", "--state-dir", f.root.join("v2-state").to_str().unwrap(),
        "--expected-sequence", "0", "--idempotency-key", "hijack",
        "--publisher", "publisher-one", "--publisher-key", &format!("publisher-one={}", f.publisher.public_hex()),
        "--reviewed-by", "operator", "--review-id", "hijack", "--validator", worker.to_str().unwrap(),
        "--key", key.to_str().unwrap(), "--anchor", &anchor.public_hex(),
        "--anchor-cert", &anchor.certify(&working.public_hex()).unwrap(),
    ]).output().unwrap();
    assert!(!output.status.success(), "v2 must preserve v1 publisher key binding");
    assert!(!v2_path.exists());
    assert_eq!(serde_json::from_slice::<Catalog>(&fs::read(f.root.join("public/catalog.json")).unwrap()).unwrap().sequence, 1);
}

#[test]
fn recovery_republishes_durable_history_above_the_high_water_mark() {
    let f = Fixture::new();
    let anchor = HubKey::generate();
    let working = HubKey::generate();
    let mut backup = Catalog::new(12, "2026-09-01", vec![]);
    working.sign_catalog(&mut backup, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
    let path = f.root.join("public/catalog.json");
    write_catalog(&path, &backup);
    assert!(renew(&f, &anchor, &working, 12, "renewal").status.success());
    write_catalog(&path, &backup);
    assert!(!operation(&f, &anchor, &working, 12, "bad-recovery", "recover").status.success());
    let recovered = operation(&f, &anchor, &working, 13, "restore", "recover");
    assert!(recovered.status.success(), "{}", String::from_utf8_lossy(&recovered.stderr));
    let result: Catalog = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(result.sequence, 14);
    verify_catalog(&result, &anchor.public_hex()).unwrap();
    assert!(operation(&f, &anchor, &working, 13, "restore", "recover").status.success());
    assert_eq!(serde_json::from_slice::<Catalog>(&fs::read(&path).unwrap()).unwrap().sequence, 14);
}

#[test]
fn operator_status_reports_authenticated_freshness_as_json() {
    let f = Fixture::new();
    let anchor = HubKey::generate();
    let working = HubKey::generate();
    let mut catalog = Catalog::new(9, &today(), vec![]);
    working.sign_catalog(&mut catalog, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
    let path = f.root.join("public/catalog.json");
    write_catalog(&path, &catalog);
    let output = Command::new(env!("CARGO_BIN_EXE_hub")).args(["admin", "status", "--catalog", path.to_str().unwrap(), "--anchor", &anchor.public_hex()]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["sequence"], 9);
    assert_eq!(status["age_days"], 0);
    assert_eq!(status["level"], "healthy");
}
