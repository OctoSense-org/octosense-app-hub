//! `hub` — the command the publisher and the hub both run.
//!
//! ```sh
//! hub keygen <path>                       # a signing key (anchor or working)
//! hub certify --anchor <key> --working <key>
//! hub stamp <bundle>                      # write the bundle digest into its manifest
//! hub sign-manifest <bundle> --key <key> --key-id <id>
//! hub check <bundle> [--catalog <file>] [--allow-unsigned] [--publisher-key id=hex]
//! hub admin publish <bundle> --catalog <file> --state-dir <private>
//!             --expected-sequence <n> --idempotency-key <request>
//!             --key <working> --anchor-cert <hex> --reviewed-by <operator>
//!             --review-id <decision> --publisher <id> [--catalog-schema 2]
//! hub verify <catalog> --anchor <hex>
//! ```
//!
//! `check` is the gate: a developer runs it before submitting and sees the
//! same report the hub's job produces. `admin publish` runs the gate again,
//! copies the bundle into the artifact store and signs the catalog.
use octosense_app_hub::*;
use octosense_app_hub::scan::Route;
use octosense_app_policy::{AppManifest, HostLimits};
use std::path::{Path, PathBuf};

fn main() {
    if let Err(e) = run() {
        eprintln!("hub: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut argv: Vec<String> = std::env::args().collect();
    if argv.get(1).is_some_and(|s| s == "admin") && argv.get(2).is_some_and(|s| ["publish", "withdraw", "remove"].contains(&s.as_str())) {
        argv.remove(1);
    }
    let command = argv.get(1).map(String::as_str).unwrap_or("help");
    let flag = |name: &str| argv.windows(2).find(|w| w[0] == format!("--{name}")).map(|w| w[1].clone());
    let has = |name: &str| argv.iter().any(|a| a == &format!("--{name}"));
    let catalog_format = || match flag("catalog-schema").as_deref().unwrap_or("1") {
        "1" => Ok(CatalogFormat::V1),
        "2" => Ok(CatalogFormat::V2),
        _ => Err("--catalog-schema must be 1 or 2".to_string()),
    };
    let positional = argv.get(2).cloned();

    match command {
        "admin" if positional.as_deref() == Some("status") => {
            let result = (|| {
                let catalog = read_catalog(Path::new(&flag("catalog").ok_or("--catalog <file>")?))?;
                operations::health(&catalog, &flag("anchor").ok_or("--anchor <trusted public key>")?, &today())
            })();
            match result {
                Ok(report) => {
                    println!("{}", serde_json::to_string(&report).map_err(|e| e.to_string())?);
                    if report.level == operations::HealthLevel::Healthy { Ok(()) }
                    else { Err(format!("catalog freshness is {:?} at {} days", report.level, report.age_days)) }
                }
                Err(error) => { println!("{}", serde_json::json!({"schema":1,"passed":false,"stage":"distribution","error":error})); Err(error) }
            }
        }
        "admin" if positional.as_deref() == Some("probe") => {
            let result = (|| {
                let base = flag("base").ok_or("--base <https hub origin>")?;
                let uri = base.parse::<ureq::http::Uri>().map_err(|e| format!("invalid probe origin: {e}"))?;
                if uri.authority().is_none_or(|a| a.as_str().contains('@')) || uri.path_and_query().is_some_and(|p| p.query().is_some()) {
                    return Err("probe origin must have a host and no credentials or query".into());
                }
                if uri.scheme_str() != Some("https") && !(has("allow-local-http") && uri.scheme_str() == Some("http")
                    && matches!(uri.host(), Some("127.0.0.1" | "localhost"))) {
                    return Err("public probes require HTTPS; --allow-local-http is for explicit loopback tests".into());
                }
                let anchor = flag("anchor").ok_or("--anchor <trusted public key>")?;
                let minimum = if let Some(path) = flag("expected-catalog") {
                    let local = read_catalog(Path::new(&path))?;
                    verify_catalog(&local, &anchor)?;
                    local.sequence
                } else { flag("minimum-sequence").unwrap_or_else(|| "0".into()).parse::<u64>().map_err(|e| e.to_string())? };
                let sample = flag("max-artifacts").unwrap_or_else(|| "10".into()).parse::<usize>().map_err(|e| e.to_string())?;
                operations::probe(&Remote::new(&base), &anchor, minimum, &today(), sample)
            })();
            match result {
                Ok(report) => {
                    println!("{}", serde_json::to_string(&report).map_err(|e| e.to_string())?);
                    if report.passed { Ok(()) } else { Err("distribution probe requires operator attention".into()) }
                }
                Err(error) => { println!("{}", serde_json::json!({"schema":1,"passed":false,"stage":"distribution","error":error})); Err(error) }
            }
        }
        "admin" if matches!(positional.as_deref(), Some("renew" | "recover")) => {
            let catalog_path = PathBuf::from(flag("catalog").ok_or("--catalog <file>")?);
            let state = PathBuf::from(flag("state-dir").ok_or("--state-dir <private durable directory>")?);
            let expected = flag("expected-sequence").ok_or("--expected-sequence <number or auto for renewal>")?;
            let request = flag("idempotency-key").ok_or("--idempotency-key <unique request>")?;
            let anchor = flag("anchor").ok_or("--anchor <trusted public key>")?;
            let working = load_key(&flag("key").ok_or("--key <working key file>")?)?;
            let certificate = flag("anchor-cert").ok_or("--anchor-cert <certificate>")?;
            let store = release::ReleaseStore::open_format(&catalog_path, &state, &anchor, catalog_format()?)?;
            let signer = signing::LocalCatalogSigner { key: &working, anchor_certificate: &certificate };
            let catalog = if positional.as_deref() == Some("recover") {
                store.recover(expected.parse::<u64>().map_err(|e| e.to_string())?, &request, &today(), &signer)?
            } else if expected == "auto" { store.renew_current(&request, &today(), &signer)? }
            else { store.renew(expected.parse::<u64>().map_err(|e| e.to_string())?, &request, &today(), &signer)? };
            println!("{} catalog sequence {} ({})", positional.unwrap(), catalog.sequence, catalog.published);
            Ok(())
        }
        "admin" => Err("usage: hub admin renew --catalog <file> --state-dir <private directory> --expected-sequence <n> --idempotency-key <request> --anchor <hex> --key <file> --anchor-cert <hex>".into()),
        "keygen" => {
            let path = positional.ok_or("usage: hub keygen <path>")?;
            let key = HubKey::generate();
            std::fs::write(&path, hex::encode(key.to_bytes())).map_err(|e| e.to_string())?;
            println!("{}", key.public_hex());
            Ok(())
        }
        "pubkey" => {
            let key = load_key(&positional.ok_or("usage: hub pubkey <key file>")?)?;
            println!("{}", key.public_hex());
            Ok(())
        }
        "certify" => {
            let anchor = load_key(&flag("anchor").ok_or("--anchor <key file>")?)?;
            let working = load_key(&flag("working").ok_or("--working <key file>")?)?;
            println!("{}", anchor.certify(&working.public_hex())?);
            Ok(())
        }
        "stamp" => {
            let bundle = PathBuf::from(positional.ok_or("usage: hub stamp <bundle>")?);
            let digest = octosense_app_policy::digest_dir(&bundle)?;
            let path = bundle.join(octosense_app_policy::MANIFEST_FILE);
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let mut value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            value["integrity"]["bundle_blake3"] = serde_json::Value::String(digest.clone());
            write_json(&path, &value)?;
            println!("{digest}");
            Ok(())
        }
        "sign-manifest" => {
            let bundle = PathBuf::from(positional.ok_or("usage: hub sign-manifest <bundle> --key <f> --key-id <id>")?);
            let key = load_key(&flag("key").ok_or("--key <key file>")?)?;
            let key_id = flag("key-id").ok_or("--key-id <id>")?;
            let path = bundle.join(octosense_app_policy::MANIFEST_FILE);
            let mut manifest = AppManifest::parse(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)?;
            sign_manifest(&key, &mut manifest, &key_id)?;
            write_json(&path, &serde_json::to_value(&manifest).map_err(|e| e.to_string())?)?;
            println!("signed {} {} with {key_id}", manifest.id, manifest.version);
            Ok(())
        }
        "check" => {
            let bundle = PathBuf::from(positional.ok_or("usage: hub check <bundle>")?);
            let report = match gate_for(&bundle, &argv, has("allow-unsigned"), flag("catalog")) {
                Ok(report) => report,
                Err(error) => {
                    if has("json") { println!("{}", serde_json::json!({"schema":1,"stage":"structural","passed":false,"findings":[{"severity":"refusal","check":"bundle-invalid","detail":error}]})); }
                    return Err(error);
                }
            };
            if has("json") { println!("{}", report.json()); } else { print!("{}", report.render()); }
            if report.passed() {
                Ok(())
            } else {
                Err("the bundle was refused".into())
            }
        }
        "test" => {
            let bundle = PathBuf::from(positional.ok_or("usage: hub test <bundle> [--validator <executable>] [--json]")?);
            let report = match gate_for(&bundle, &argv, has("allow-unsigned"), flag("catalog")) {
                Ok(report) => report,
                Err(error) => {
                    if has("json") { println!("{}", serde_json::json!({"schema":1,"stage":"structural","passed":false,"findings":[{"severity":"refusal","check":"bundle-invalid","detail":error}]})); }
                    return Err(error);
                }
            };
            if !report.passed() {
                if has("json") { println!("{}", report.json()); } else { print!("{}", report.render()); }
                return Err("the bundle was refused".into());
            }
            let evidence = match validator_path(flag("validator")).and_then(|validator| runtime::validate(&bundle, &report, &validator)) {
                Ok(evidence) => evidence,
                Err(error) => {
                    if has("json") { println!("{}", runtime::failure_json(&error)); }
                    return Err(error);
                }
            };
            if has("json") { println!("{}", serde_json::json!({"schema":1,"stage":"runtime","passed":true,"findings":report.findings,"evidence":evidence})); }
            else { print!("{}", report.render()); println!("  runtime: {} — {}", evidence.report().runtime, evidence.report().checks.join(", ")); }
            Ok(())
        }
        "publish" => {
            if has("allow-unsigned") { return Err("public releases require a signed manifest; --allow-unsigned is only for local checks".into()); }
            if has("reviewer") || has("reviewed") {
                return Err("run review separately with hub scan; publication requires --reviewed-by and --review-id, not a caller-supplied command or passed flag".into());
            }
            let bundle = PathBuf::from(positional.ok_or("usage: hub admin publish <bundle> [operator transaction flags]")?);
            let catalog_path = PathBuf::from(flag("catalog").ok_or("--catalog <file>")?);
            let state = PathBuf::from(flag("state-dir").ok_or("--state-dir <private durable directory>")?);
            let expected = flag("expected-sequence").ok_or("--expected-sequence <number>")?.parse::<u64>().map_err(|e| e.to_string())?;
            let request = flag("idempotency-key").ok_or("--idempotency-key <unique request>")?;
            let anchor = flag("anchor").ok_or("--anchor <hex> is required to authenticate catalog history and its signer")?;
            let review = approval::OperatorReview {
                reviewer: flag("reviewed-by").ok_or("--reviewed-by <authenticated operator identity> is required")?,
                review_id: flag("review-id").ok_or("--review-id <review decision record> is required")?,
            };
            // Structural/native work happens before loading a signing key or
            // taking the release lock. Continuity is rechecked under that lock.
            let report = gate_for(&bundle, &argv, false, None)?;
            print!("{}", report.render());
            if !report.passed() { return Err("refusing to publish a bundle the gate refused".into()); }
            let evidence = runtime::validate(&bundle, &report, &validator_path(flag("validator"))?)?;
            let publisher = flag("publisher").ok_or("--publisher <id>")?;
            let publisher_key = argv.windows(2).filter(|w| w[0] == "--publisher-key")
                .filter_map(|w| w[1].split_once('='))
                .find(|(id, _)| *id == publisher).map(|(_, key)| key).unwrap_or("");
            let release = approval::ApprovedRelease::new(evidence, &report, &publisher, publisher_key,
                &flag("repo").unwrap_or_default(), &flag("commit").unwrap_or_default(), review)?;
            if let Some(out) = flag("out") {
                let parent = catalog_path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
                if Path::new(&out).canonicalize().map_err(|e| e.to_string())? != parent.canonicalize().map_err(|e| e.to_string())? {
                    return Err("--out must be the catalog's parent directory so its relative artifact paths are reachable".into());
                }
            }
            let working = load_key(&flag("key").ok_or("--key <working key file>")?)?;
            let certificate = flag("anchor-cert").ok_or("--anchor-cert <certificate>")?;
            let store = release::ReleaseStore::open_format(&catalog_path, &state, &anchor, catalog_format()?)?;
            let catalog = store.publish(expected, &request, &today(), &signing::LocalCatalogSigner { key: &working, anchor_certificate: &certificate }, &release)?;
            println!("published {} {} (catalog sequence {}, approval {})", report.app_id, report.version, catalog.sequence, release.id());
            Ok(())
        }
        "withdraw" => {
            let app = positional.ok_or("usage: hub admin withdraw <app id> [operator transaction flags]")?;
            let version = flag("version").ok_or("--version <v>")?;
            let reason = flag("reason").ok_or("--reason <text shown to the person>")?;
            let catalog_path = PathBuf::from(flag("catalog").ok_or("--catalog <file>")?);
            let state = PathBuf::from(flag("state-dir").ok_or("--state-dir <private durable directory>")?);
            let expected = flag("expected-sequence").ok_or("--expected-sequence <number>")?.parse::<u64>().map_err(|e| e.to_string())?;
            let request = flag("idempotency-key").ok_or("--idempotency-key <unique request>")?;
            let anchor = flag("anchor").ok_or("--anchor <trusted public key>")?;
            let working = load_key(&flag("key").ok_or("--key <working key file>")?)?;
            let certificate = flag("anchor-cert").ok_or("--anchor-cert <certificate>")?;
            let store = release::ReleaseStore::open_format(&catalog_path, &state, &anchor, catalog_format()?)?;
            let catalog = store.withdraw(expected, &request, &today(), &signing::LocalCatalogSigner { key: &working, anchor_certificate: &certificate }, &app, &version, &reason)?;
            println!("withdrew {app} {version}: {reason} (catalog sequence {})", catalog.sequence);
            Ok(())
        }
        "remove" => Err("catalog history and immutable artifacts cannot be removed by a release command; use hub admin withdraw for an exact app/version".into()),
        "scan" => {
            let bundle = PathBuf::from(positional.ok_or("usage: hub scan <bundle> [--reviewer <cmd>] [--packet <out.json>]")?);
            let report = gate_for(&bundle, &argv, true, flag("catalog"))?;
            if !report.passed() {
                print!("{}", report.render());
                return Err("the gate refused this bundle; a scan is not offered".into());
            }
            let packet = packet(&bundle, &report)?;
            if let Some(path) = flag("packet") {
                write_json(Path::new(&path), &serde_json::to_value(&packet).map_err(|e| e.to_string())?)?;
                println!("wrote the review packet to {path}");
            }
            match flag("reviewer") {
                Some(reviewer) => {
                    let verdict = scan(&packet, &reviewer);
                    println!("{}", serde_json::to_string_pretty(&verdict).map_err(|e| e.to_string())?);
                    if verdict.route == Route::Reject {
                        return Err("rejected".into());
                    }
                    Ok(())
                }
                None => {
                    println!("no --reviewer given; the packet holds {} questions for one", packet.questions.len());
                    Ok(())
                }
            }
        }
        "verify" => {
            let catalog = read_catalog(Path::new(&positional.ok_or("usage: hub verify <catalog> --anchor <hex>")?))?;
            let anchor = flag("anchor").ok_or("--anchor <hex public key>")?;
            verify_catalog(&catalog, &anchor)?;
            println!("catalog sequence {} verified, {} entries", catalog.sequence, catalog.entries.len());
            Ok(())
        }
        _ => {
            println!("{}", include_str!("hub-usage.txt"));
            Ok(())
        }
    }
}

fn validator_path(override_path: Option<String>) -> Result<PathBuf, String> {
    if let Some(path) = override_path { return Ok(PathBuf::from(path)); }
    Ok(std::env::current_exe().map_err(|e| e.to_string())?.with_file_name("app-validator"))
}

fn gate_for(bundle: &Path, argv: &[String], allow_unsigned: bool, catalog: Option<String>) -> Result<GateReport, String> {
    let mut keys = PublisherKeys::new();
    for pair in argv.windows(2).filter(|w| w[0] == "--publisher-key").map(|w| w[1].clone()) {
        let (id, public) = pair.split_once('=').ok_or("--publisher-key expects id=hexkey")?;
        keys = keys.with(id, public);
    }
    let limits = HostLimits { require_signature: !allow_unsigned, ..HostLimits::default() };
    let previous = match catalog {
        Some(path) if Path::new(&path).exists() => {
            let catalog = read_catalog(Path::new(&path))?;
            let anchor = argv.windows(2).find(|w| w[0] == "--anchor")
                .map(|w| w[1].as_str()).ok_or("--anchor <hex> is required to authenticate an existing catalog")?;
            verify_catalog(&catalog, anchor)?;
            Some(catalog)
        }
        _ => None,
    };
    check_bundle(bundle, &limits, &keys, previous.as_ref())
}

fn load_key(path: &str) -> Result<HubKey, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let raw = hex::decode(text.trim()).map_err(|e| format!("{path}: not hex: {e}"))?;
    let bytes: [u8; 32] = raw.as_slice().try_into().map_err(|_| format!("{path}: not a 32-byte key"))?;
    Ok(HubKey::from_bytes(&bytes))
}

fn read_catalog(path: &Path) -> Result<Catalog, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, format!("{text}\n")).map_err(|e| format!("{}: {e}", path.display()))
}
