use octosense_app_hub::*;
use octosense_app_policy::{AppManifest, SignatureVerifier};
use serde_json::{json, Value};

fn legacy() -> Catalog { serde_json::from_str(include_str!("fixtures/wire/v1-catalog.json")).unwrap() }
fn v2_value() -> Value {
    let mut value = serde_json::to_value(&legacy().entries[0].manifest).unwrap();
    value["schema"] = json!(2);
    value["version"] = json!("1.2.0");
    value["release_number"] = json!(12);
    value["runtime"] = json!({"api":"1","min_build":1,"platforms":["macos","android"]});
    value["requires"] = json!(["card.ui@1"]);
    value["entrypoints"] = json!({"ui":"page.card","logic":null});
    value["data_schema"] = json!(1);
    value
}

#[test]
fn v1_golden_catalog_and_manifest_signatures_still_verify() {
    let catalog = legacy();
    assert_eq!(catalog.signing_bytes().unwrap(), include_bytes!("fixtures/wire/v1-catalog.canonical.json"));
    assert_eq!(catalog.entries[0].manifest.signing_bytes().unwrap(), include_bytes!("fixtures/wire/v1-manifest.canonical.json"));
    verify_catalog(&catalog, &HubKey::from_bytes(&[1; 32]).public_hex()).unwrap();
    let manifest = &catalog.entries[0].manifest;
    let signature = manifest.integrity.signature.as_ref().unwrap();
    PublisherKeys::new().with("golden-author", &HubKey::from_bytes(&[3; 32]).public_hex())
        .verify(&signature.key_id, &signature.value, &manifest.signing_bytes().unwrap()).unwrap();
}

#[test]
fn unsupported_or_mixed_wire_schemas_are_rejected() {
    let mut value = serde_json::to_value(legacy()).unwrap();
    value["schema"] = json!(99);
    assert!(serde_json::from_value::<Catalog>(value).is_err(), "unknown catalog semantics must not deserialize");
    let mut value = serde_json::to_value(legacy()).unwrap();
    value["entries"][0]["manifest"]["schema"] = json!(99);
    assert!(serde_json::from_value::<Catalog>(value).is_err());
    let mut value = serde_json::to_value(legacy()).unwrap();
    value["entries"][0]["manifest"] = v2_value();
    assert!(serde_json::from_value::<Catalog>(value).is_err(), "v1 may not carry v2 semantics");
    let mut value = serde_json::to_value(&legacy().entries[0].manifest).unwrap();
    value["runtime"] = json!({"api":"1"});
    assert!(AppManifest::parse(&value.to_string()).is_err());
}

#[test]
fn v2_contract_is_signed_and_round_trips_without_changing_v1() {
    let publisher = HubKey::from_bytes(&[3; 32]);
    let mut manifest = AppManifest::parse(&v2_value().to_string()).expect("explicit v2 reader");
    sign_manifest(&publisher, &mut manifest, "golden-author").unwrap();
    let encoded = serde_json::to_string(&manifest).unwrap();
    let reparsed = AppManifest::parse(&encoded).unwrap();
    assert_eq!(manifest.signing_bytes().unwrap(), reparsed.signing_bytes().unwrap());
    let mut changed = serde_json::to_value(&manifest).unwrap();
    changed["runtime"]["min_build"] = json!(2);
    let changed = AppManifest::parse(&changed.to_string()).unwrap();
    let signature = changed.integrity.signature.as_ref().unwrap();
    assert!(PublisherKeys::new().with("golden-author", &publisher.public_hex())
        .verify(&signature.key_id, &signature.value, &changed.signing_bytes().unwrap()).is_err());
    let mut catalog = legacy();
    catalog.schema = 2;
    catalog.entries[0].manifest = manifest;
    let working = HubKey::from_bytes(&[2; 32]);
    working.sign_catalog(&mut catalog, &HubKey::from_bytes(&[1; 32]).certify(&working.public_hex()).unwrap()).unwrap();
    let roundtrip: Catalog = serde_json::from_str(&serde_json::to_string(&catalog).unwrap()).unwrap();
    verify_catalog(&roundtrip, &HubKey::from_bytes(&[1; 32]).public_hex()).unwrap();
}

#[test]
fn v2_requires_valid_versions_and_complete_contracts() {
    assert!(AppManifest::parse(&v2_value().to_string()).is_ok());
    for (field, bad) in [("version",json!("1.02.0")),("release_number",json!(0)),("runtime",json!({"api":"1","min_build":0,"platforms":[]})),("entrypoints",json!({"ui":"../page.card"})),("requires",json!(["unversioned"]))] {
        let mut value = v2_value(); value[field] = bad;
        assert!(AppManifest::parse(&value.to_string()).is_err(), "invalid {field} must fail");
    }
    for field in ["release_number","runtime","requires","entrypoints","data_schema"] {
        let mut value = v2_value(); value.as_object_mut().unwrap().remove(field);
        assert!(AppManifest::parse(&value.to_string()).is_err(), "v2 {field} is required");
    }
}

#[test]
fn v2_golden_signing_bytes_are_frozen() {
    let catalog: Catalog = serde_json::from_str(include_str!("fixtures/wire/v2-catalog.json")).unwrap();
    assert_eq!(catalog.signing_bytes().unwrap(), include_bytes!("fixtures/wire/v2-catalog.canonical.json"));
    assert_eq!(catalog.entries[0].manifest.signing_bytes().unwrap(), include_bytes!("fixtures/wire/v2-manifest.canonical.json"));
    verify_catalog(&catalog, &HubKey::from_bytes(&[1; 32]).public_hex()).unwrap();
}

fn v2_entry(number: u64, version: &str, min_build: u64) -> Entry {
    let mut entry = legacy().entries.remove(0);
    let mut value = v2_value();
    value["release_number"] = json!(number);
    value["version"] = json!(version);
    value["runtime"]["min_build"] = json!(min_build);
    entry.manifest = AppManifest::parse(&value.to_string()).unwrap();
    sign_manifest(&HubKey::from_bytes(&[3; 32]), &mut entry.manifest, "golden-author").unwrap();
    entry
}
fn store_with(entries: Vec<Entry>) -> Store {
    let mut catalog = Catalog::new(1, "2026-09-25", entries);
    catalog.schema = 2;
    let anchor = HubKey::from_bytes(&[1; 32]);
    let working = HubKey::from_bytes(&[2; 32]);
    working.sign_catalog(&mut catalog, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
    let mut runtime = octosense_app_policy::compatibility::RuntimeDescriptor::current();
    runtime.platform = "macos".into();
    let mut store = Store::with_runtime(&anchor.public_hex(), std::path::Path::new("/tmp/hub-compatibility-no-installed"), octosense_app_policy::HostLimits::default(), runtime, CatalogFormat::V2);
    store.accept_catalog(&serde_json::to_string(&catalog).unwrap()).unwrap();
    store
}

#[test]
fn newest_compatible_release_uses_numbers_not_labels_or_catalog_order() {
    let store = store_with(vec![v2_entry(10, "1.10.0", 1), v2_entry(11, "2.0.0", 99), v2_entry(2, "9.0.0", 1)]);
    assert_eq!(store.entry("golden").unwrap().version(), "1.10.0");
    assert_eq!(store.listings()[0].version, "1.10.0");
}

#[test]
fn incompatible_listing_has_reasons_and_is_refused_before_staging() {
    let store = store_with(vec![v2_entry(12, "1.2.0", 99)]);
    let listing = &store.listings()[0];
    assert!(matches!(listing.availability, Availability::Unavailable { .. }));
    assert_eq!(listing.compatibility.reasons[0].code, "runtime-build");
    assert!(store.install_candidate("golden").unwrap_err().contains("99"));
    // This path does not exist: compatibility must refuse before any I/O.
    let error = store.install_staged("golden", std::path::Path::new("/no-such-staging-directory"), &store.publisher_keys(), "2026-09-25").unwrap_err();
    assert!(error.contains("99"), "{error}");
}

#[test]
fn runtime_requirements_cannot_grant_features_capabilities_or_unwired_entrypoints() {
    use octosense_app_policy::compatibility::{RuntimeDescriptor, evaluate};
    let mut runtime = RuntimeDescriptor::current(); runtime.platform = "macos".into();
    for (field, value, code) in [
        ("runtime",json!({"api":"1","min_build":1,"platforms":["android"]}),"platform"),
        ("runtime",json!({"api":"9","min_build":1,"platforms":["macos"]}),"runtime-api"),
        ("requires",json!(["camera.capture@1"]),"feature"),
        ("capabilities",json!(["camera"]),"capability"),
        ("entrypoints",json!({"ui":"page.card","logic":"app.octoscript"}),"entrypoint"),
    ] {
        let mut manifest = v2_value(); manifest[field] = value;
        let result = evaluate(&AppManifest::parse(&manifest.to_string()).unwrap(), &runtime);
        assert!(!result.compatible, "{field}");
        assert!(result.reasons.iter().any(|r| r.code == code), "{result:?}");
    }
}

#[test]
fn current_runtime_accepts_the_existing_script_app_entrypoint() {
    use octosense_app_policy::compatibility::{evaluate, RuntimeDescriptor};
    let mut value = v2_value();
    value["entrypoints"] = json!({"ui":"main.splash"});
    value["requires"] = json!(["script.ui@1"]);
    value["runtime"]["platforms"] = json!([std::env::consts::OS]);
    let manifest = AppManifest::parse(&value.to_string()).unwrap();
    let result = evaluate(&manifest, &RuntimeDescriptor::current());
    assert!(result.compatible, "{result:?}");
}

#[test]
fn current_runtime_advertises_core_isolate_capabilities() {
    use octosense_app_policy::compatibility::{evaluate, RuntimeDescriptor};
    let mut value = v2_value();
    value["capabilities"] = json!(["storage", "net", "prompt"]);
    value["runtime"]["platforms"] = json!([std::env::consts::OS]);
    let manifest = AppManifest::parse(&value.to_string()).unwrap();
    let result = evaluate(&manifest, &RuntimeDescriptor::current());
    assert!(result.compatible, "{result:?}");
}

#[test]
fn catalog_domains_do_not_share_sequence_floors_or_accept_each_others_format() {
    let anchor = HubKey::from_bytes(&[1; 32]).public_hex();
    let mut legacy_store = Store::new(&anchor, std::path::Path::new("/tmp/hub-v1-compatibility"), octosense_app_policy::HostLimits::default());
    legacy_store.accept_catalog(include_str!("fixtures/wire/v1-catalog.json")).unwrap();
    assert!(legacy_store.accept_catalog(include_str!("fixtures/wire/v2-catalog.json")).is_err());
    let modern = store_with(vec![v2_entry(1, "1.0.0", 1)]);
    assert_eq!(modern.catalog().unwrap().sequence, 1, "v1 sequence 23 is unrelated to v2 sequence 1");
    let mut modern = modern;
    assert!(modern.accept_catalog(include_str!("fixtures/wire/v1-catalog.json")).is_err());
    assert_eq!(CatalogFormat::V1.relative_catalog(), "catalog.json");
    assert_eq!(CatalogFormat::V2.relative_catalog(), "v2/catalog.json");
}

#[test]
fn version_dispatch_preserves_actionable_errors_and_rejects_duplicate_fields() {
    let mut value = serde_json::to_value(&legacy().entries[0].manifest).unwrap();
    value["sandbox"] = json!("off");
    let error = AppManifest::parse(&value.to_string()).unwrap_err();
    assert!(error.contains("sandbox"), "{error}");
    let mut value = serde_json::to_value(legacy()).unwrap();
    value["unexpected"] = json!(true);
    let error = serde_json::from_value::<Catalog>(value).unwrap_err().to_string();
    assert!(error.contains("unexpected"), "{error}");
    let text = serde_json::to_string(&legacy().entries[0].manifest).unwrap();
    assert!(AppManifest::parse(&text.replace("\"schema\":1", "\"schema\":1,\"schema\":1")).is_err());
    assert!(AppManifest::parse(&text.replace("\"memory_bytes\":null", "\"memory_bytes\":null,\"memory_bytes\":42")).is_err());
}

mod common;
#[test]
fn installed_v2_is_not_automatically_downgraded_and_legacy_launch_survives_v2_catalog() {
    let f = common::Fixture::new();
    let root = f.root.join("installed");
    let anchor = HubKey::generate(); let working = HubKey::generate();
    let cert = anchor.certify(&working.public_hex()).unwrap();
    let mut runtime = octosense_app_policy::compatibility::RuntimeDescriptor::current(); runtime.platform = "macos".into();
    let mut store = Store::with_runtime(&anchor.public_hex(), &root, octosense_app_policy::HostLimits::default(), runtime, CatalogFormat::V2);
    let old = f.entry();
    let mut next = old.clone();
    let mut value = v2_value(); value["id"] = json!(old.app_id()); value["version"] = json!("2.0.0");
    value["runtime"]["min_build"] = json!(99);
    next.manifest = AppManifest::parse(&value.to_string()).unwrap();
    sign_manifest(&f.publisher, &mut next.manifest, "publisher-one").unwrap();
    let mut catalog = Catalog::new(1, "2026-09-25", vec![old.clone(), next]); catalog.schema = 2;
    working.sign_catalog(&mut catalog, &cert).unwrap(); store.accept_catalog(&serde_json::to_string(&catalog).unwrap()).unwrap();
    store.install_staged(old.app_id(), &f.bundle, &f.keys(), "2026-09-25").unwrap();
    assert!(store.may_run(old.app_id()).is_ok());
    assert!(store.app_availability(old.app_id()).can_open);
    assert!(store.app_availability(old.app_id()).update_version.is_none());
    // Model a newer installed v2 on a subsequently older runtime. It may not
    // silently be replaced by an eligible lower release or a v1 fallback.
    let installed = catalog.entries[1].manifest.clone();
    std::fs::write(store.install_dir(old.app_id()).join("manifest.json"), serde_json::to_vec(&installed).unwrap()).unwrap();
    assert!(store.install_candidate(old.app_id()).unwrap_err().contains("downgrade"));
    assert!(store.app_availability(old.app_id()).update_version.is_none());
}

#[test]
fn v2_duplicate_release_numbers_or_versions_are_ambiguous() {
    let mut catalog = Catalog::new(1, "2026-09-25", vec![v2_entry(2,"1.0.0",1),v2_entry(2,"2.0.0",1)]);
    catalog.schema = 2;
    assert!(catalog.validate_schema().is_err(), "one numeric release may not identify different artifacts");
    catalog.entries = vec![v2_entry(2,"1.0.0",1),v2_entry(3,"1.0.0",1)];
    assert!(catalog.validate_schema().is_err(), "installed version must identify one release");
}

#[test]
fn cli_compatibility_matches_shared_client_reason() {
    let mut f = common::Fixture::new();
    let mut manifest = v2_value(); manifest["id"] = json!(f.manifest.id); manifest["runtime"]["min_build"] = json!(99);
    f.manifest = AppManifest::parse(&manifest.to_string()).unwrap(); f.sign();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_hub")).arg("check").arg(&f.bundle)
        .args(["--json","--publisher-key",&format!("publisher-one={}",f.publisher.public_hex())]).output().unwrap();
    assert!(!output.status.success(), "incompatible runtime must be visible before upload");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["compatibility"]["reasons"][0]["code"], "runtime-build", "{report}");
}
