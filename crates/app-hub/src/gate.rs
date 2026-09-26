//! Stage one of admission: the checks a model never makes (ADR 0003 §5).
//!
//! Everything here is a fact about the bundle, decided by code, reported the
//! same way whether it runs on a developer's machine before submitting or in
//! the hub's job after. A finding is either a refusal or a warning; an agent
//! scan runs afterwards on what passes, and never overrides a refusal.
use crate::index::{Catalog, Entry};
use octosense_app_policy::{ policy, AppManifest, AppPolicy, HostLimits, Listing, SignatureVerifier, LISTING_FILE};
use std::path::Path;

/// Everything a bundle may hold besides its manifest, by extension. A bundle
/// is cards, data and artwork; anything else is a refusal, so a publisher
/// cannot smuggle a payload the checks do not understand.
const ALLOWED_EXTENSIONS: &[&str] = &["card", "json", "l0", "octoscript", "splash", "svg", "png", "jpg", "jpeg", "webp", "ttf", "otf", "txt", "md"];

/// Ceiling for a whole bundle. Cards are text and artwork; a bundle bigger
/// than this is either shipping something it should not, or should be split.
pub const MAX_BUNDLE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// The bundle is not admitted.
    Refusal,
    /// Admitted, but the publisher and a reviewer should see it.
    Warning,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub check: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Finding {
    fn refuse(check: &'static str, detail: impl Into<String>) -> Self {
        Finding { severity: Severity::Refusal, check, detail: detail.into(), path: None }
    }
    fn warn(check: &'static str, detail: impl Into<String>) -> Self {
        Finding { severity: Severity::Warning, check, detail: detail.into(), path: None }
    }
    pub(crate) fn at(check: &'static str, path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { severity: Severity::Refusal, check, path: Some(path.into()), detail: detail.into() }
    }
}

#[derive(Debug)]
pub struct GateReport {
    pub app_id: String,
    pub version: String,
    pub digest: String,
    pub findings: Vec<Finding>,
    /// What the app would actually get, when the gate passed.
    pub policy: Option<AppPolicy>,
    pub resources: Vec<crate::admission::ResourceReference>,
    pub compatibility: octosense_app_policy::compatibility::CompatibilityResult,
    // Bind entry creation to the complete manifest checked by this report.
    admitted_manifest: Vec<u8>,
}

impl GateReport {
    pub fn manifest_digest(&self) -> String {
        blake3::hash(&self.admitted_manifest).to_hex().to_string()
    }
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": 1, "stage": "structural", "passed": self.passed(),
            "app_id": self.app_id, "version": self.version, "digest": self.digest,
            "findings": self.findings, "resources": self.resources, "compatibility": self.compatibility,
        })
    }

    pub fn passed(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Refusal)
    }

    /// One line per finding, for a pull-request comment or a terminal.
    pub fn render(&self) -> String {
        let mut out = format!("{} {} — {}\n", self.app_id, self.version, if self.passed() { "PASSED" } else { "REFUSED" });
        for finding in &self.findings {
            let mark = match finding.severity {
                Severity::Refusal => "refused",
                Severity::Warning => "warning",
            };
            let path = finding.path.as_ref().map(|p| format!(" ({p})")).unwrap_or_default();
            out.push_str(&format!("  [{mark}] {}{path}: {}\n", finding.check, finding.detail));
        }
        if let Some(policy) = &self.policy {
            out.push_str(&format!(
                "  grants: capabilities {:?}, hosts {:?}, storage {} bytes, agent {}\n",
                policy.capabilities,
                policy.hosts,
                policy.storage_bytes,
                policy.agent.as_ref().map(|a| a.profile.as_kernel_mode()).unwrap_or("none")
            ));
        }
        out
    }
}

/// Run the deterministic gate over a bundle directory.
///
/// `previous` is the catalog the hub already published, used for the identity
/// rules: a version may not be republished, and once a publisher key is on
/// record every later version must carry it. The caller must authenticate
/// this catalog before passing it as trusted history.
pub fn check_bundle(
    bundle: &Path,
    limits: &HostLimits,
    verifier: &dyn SignatureVerifier,
    previous: Option<&Catalog>,
) -> Result<GateReport, String> {
    let manifest_path = bundle.join(octosense_app_policy::MANIFEST_FILE);
    let files = crate::admission::inventory(bundle)?;
    let manifest_json = crate::admission::read_text(&manifest_path, crate::admission::MAX_MANIFEST_BYTES)?;
    let manifest = AppManifest::parse(&manifest_json)?;
    let digest = octosense_app_policy::bundle::digest_dir_limited(bundle, MAX_BUNDLE_BYTES, crate::admission::MAX_ENTRIES, crate::admission::MAX_DEPTH)?;
    let (mut findings, resources) = crate::admission::validate(bundle, &files);

    // ---- integrity ------------------------------------------------------
    if digest.to_ascii_lowercase() != manifest.integrity.bundle_blake3.to_ascii_lowercase() {
        findings.push(Finding::refuse(
            "digest",
            format!("the bundle hashes to {digest}, the manifest claims {}", manifest.integrity.bundle_blake3),
        ));
    }
    match &manifest.integrity.signature {
        Some(signature) => {
            if let Err(e) = verifier.verify(&signature.key_id, &signature.value, &manifest.signing_bytes()?) {
                findings.push(Finding::refuse("publisher-signature", e));
            }
        }
        None if limits.require_signature => {
            findings.push(Finding::refuse("publisher-signature", "this hub requires a signed manifest"))
        }
        None => findings.push(Finding::warn("publisher-signature", "unsigned: accountability rests on the hub alone")),
    }

    // ---- identity ------------------------------------------------------
    // Ids under `os.` are the system apps' (appstore::system): every device
    // refuses to install one from a store, so the hub refuses to offer one.
    if manifest.id.starts_with("os.") {
        findings.push(Finding::refuse(
            "identity",
            format!("{} is under os., which is reserved for system apps that ship with the device", manifest.id),
        ));
    }

    if let Some(contract) = manifest.contract() {
        let entry = if files.iter().any(|file| file.path == Path::new(octosense_app_policy::SCRIPT_ENTRY)) {
            octosense_app_policy::SCRIPT_ENTRY
        } else {
            "page.card"
        };
        if contract.entrypoints.ui != entry {
            findings.push(Finding::refuse("entrypoint", format!(
                "manifest names {}, but this bundle runs {entry}", contract.entrypoints.ui
            )));
        }
        if let Some(logic) = &contract.entrypoints.logic {
            if !files.iter().any(|file| file.path == Path::new(logic)) {
                findings.push(Finding::refuse("entrypoint", format!("missing declared logic entrypoint {logic}")));
            }
        }
    }

    // ---- contents -------------------------------------------------------
    let mut total = 0u64;
    for file in files.iter().filter(|f| f.path != Path::new(octosense_app_policy::MANIFEST_FILE)).map(|f| &f.path) {
        let path = bundle.join(&file);
        let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        total += size;
        let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
            findings.push(Finding::refuse(
                "contents",
                format!("{} has extension {extension:?}, which a bundle may not hold", file.display()),
            ));
        }
    }
    if total > MAX_BUNDLE_BYTES {
        findings.push(Finding::refuse("size", format!("the bundle is {total} bytes, over the {MAX_BUNDLE_BYTES} ceiling")));
    }

    // ---- assets are local ----------------------------------------------
    // ADR 0002's prototype found a card with no network grant fetching nine
    // images over HTTP, because the resource loader is not the network module
    // the grant gates. Until that is closed in the runtime, the gate is what
    // keeps a bundle from reaching outside itself.
    for reference in external_references(bundle, &files, &manifest)? {
        findings.push(Finding::refuse(
            "assets",
            format!("{reference} points outside the bundle; ship the asset with the app"),
        ));
    }

    // ---- secrets are the host's ----------------------------------------
    // A contained app never collects a password, a PIN or a code: the host's
    // sheet does, for the service that needs it (appstore::services). The
    // runtime makes such a field inert; the gate refuses the bundle, so a
    // publisher learns it before a person meets a dead field.
    for field in secret_fields(bundle, &files)? {
        findings.push(Finding::refuse(
            "secrets",
            format!("{field}: apps may not ask for passwords or codes; a host service collects them on its own sheet"),
        ));
    }

    // ---- the listing ----------------------------------------------------
    // A store shows nothing it has not reviewed: the listing ships in the
    // bundle, under the same digest, and its assets must be there too.
    match std::fs::read_to_string(bundle.join(LISTING_FILE)) {
        Err(_) => findings.push(Finding::refuse("listing", format!("no {LISTING_FILE}: a store needs a description, a category, screenshots, a publisher and the platforms it runs on"))),
        Ok(text) => match Listing::parse(&text) {
            Err(e) => findings.push(Finding::refuse("listing", e)),
            Ok(listing) => {
                // A screenshot is the one listing claim a reviewer can check
                // against the rendered card, and the icon is what the
                // launcher shows once installed: both are required, as on
                // every store people know.
                if listing.screenshots.is_empty() {
                    findings.push(Finding::refuse("listing", "no screenshots: at least one PNG in the bundle, named in the listing, is required"));
                }
                if listing.icon.is_none() {
                    findings.push(Finding::refuse("listing", "no icon: a square PNG or SVG in the bundle, named in the listing, is required"));
                }
                for asset in listing.screenshots.iter().chain(listing.icon.iter()) {
                    if !bundle.join(asset).is_file() {
                        findings.push(Finding::refuse("listing", format!("{asset} is named by the listing but is not in the bundle")));
                    }
                }
            }
        },
    }

    // ---- what it would get ----------------------------------------------
    let policy = match policy::resolve(&manifest, limits) {
        Ok(policy) => Some(policy),
        Err(e) => {
            findings.push(Finding::refuse("policy", e));
            None
        }
    };

    // ---- identity against what is already published ----------------------
    if let Some(catalog) = previous {
        for entry in &catalog.entries {
            if entry.app_id() == manifest.id && entry.version() == manifest.version {
                findings.push(Finding::refuse(
                    "version",
                    format!("version {} of {} is already published; publish a new version", manifest.version, manifest.id),
                ));
            }
        }
        let continuity = crate::publishers::CatalogPublishers::from_catalog(catalog)
            .and_then(|registry| crate::publishers::verify_continuity(&manifest, &registry));
        if let Err(e) = continuity {
            findings.push(Finding::refuse("continuity", e));
        }
    }

    let compatibility = octosense_app_policy::compatibility::evaluate(&manifest, &octosense_app_policy::compatibility::RuntimeDescriptor::current());
    for reason in &compatibility.reasons { findings.push(Finding::refuse("compatibility", reason.message.clone())); }
    let admitted_manifest = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    Ok(GateReport { app_id: manifest.id, version: manifest.version, digest, findings, policy, resources, compatibility, admitted_manifest })
}

/// Anything in the bundle's text that reaches outside it: an absolute URL, or
/// a path that climbs out. Cards name their artwork in text, so this is a
/// textual check by necessity; it is a gate, not the runtime's enforcement.
fn external_references(root: &Path, files: &[crate::admission::BundleFile], manifest: &AppManifest) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    for file in files.iter().map(|f| &f.path) {
        // The two metadata files carry URLs on purpose (a support page, a
        // privacy policy); nothing loads them. Their own asset paths are
        // checked by the listing rules.
        if file == Path::new(LISTING_FILE) || file == Path::new(octosense_app_policy::MANIFEST_FILE) {
            continue;
        }
        let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(extension.as_str(), "card" | "json" | "l0" | "octoscript" | "splash" | "txt" | "md") {
            continue;
        }
        let text = match crate::admission::read_text(&root.join(&file), crate::admission::MAX_TEXT_BYTES) {
            Ok(text) => text,
            Err(_) => continue, // not valid text: the extension check already covers it
        };
        if extension == "splash" {
            found.extend(script_references(&file, &text, manifest));
            continue;
        }
        if !matches!(extension.as_str(), "card" | "json" | "l0" | "octoscript" | "txt" | "md") {
            continue;
        }
        for needle in ["http://", "https://", "file://", "../"] {
            if let Some(at) = text.find(needle) {
                let snippet: String = text[at..].chars().take(60).collect();
                found.push(format!("{} contains {}", file.display(), snippet.replace('\n', " ")));
                break;
            }
        }
    }
    Ok(found)
}

/// A script app fetches what it declares (ADR 0004): an `https://` address
/// may name only a host in the manifest's `network.hosts`, or any public host
/// when the app is granted `images` (pictures) or `web` (a web view). Plain
/// `http://`, `file://` and paths out of the bundle are refused outright. The
/// runtime holds the app to the same list on every request; this refuses the
/// bundle before a person installs it.
fn script_references(file: &Path, text: &str, manifest: &AppManifest) -> Vec<String> {
    let mut found = Vec::new();
    for needle in ["http://", "file://", "../"] {
        if let Some(at) = text.find(needle) {
            let snippet: String = text[at..].chars().take(60).collect();
            found.push(format!("{} contains {}", file.display(), snippet.replace('\n', " ")));
        }
    }
    let any_public = manifest.capabilities.iter().any(|c| c == "images" || c == "web");
    let mut rest = text;
    while let Some(at) = rest.find("https://") {
        rest = &rest[at + "https://".len()..];
        let host: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect::<String>().to_ascii_lowercase();
        // `https://` followed by an interpolation or nothing names no host.
        if host.is_empty() || any_public || manifest.network.hosts.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
            continue;
        }
        found.push(format!("{} reaches {host}, which the manifest does not declare in network.hosts", file.display()));
    }
    found.sort();
    found.dedup();
    found
}

/// Password and one-time-code fields declared in the bundle's scripts.
fn secret_fields(root: &Path, files: &[crate::admission::BundleFile]) -> Result<Vec<String>, String> {
    let mut found = Vec::new();
    for file in files.iter().map(|file| &file.path) {
        let extension = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(extension.as_str(), "card" | "l0" | "octoscript" | "splash") {
            continue;
        }
        let Ok(text) = crate::admission::read_text(&root.join(file), crate::admission::MAX_TEXT_BYTES) else { continue };
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        for needle in ["is_password:true", "TextInputContentType.Password", "TextInputContentType.NewPassword", "TextInputContentType.OneTimeCode"] {
            if compact.contains(needle) {
                found.push(format!("{} declares {needle}", file.display()));
            }
        }
    }
    Ok(found)
}

/// Build the index entry for a bundle the gate passed.
#[allow(clippy::too_many_arguments)]
pub fn entry_for(
    bundle: &Path,
    report: &GateReport,
    publisher: &str,
    publisher_key: &str,
    repository: &str,
    commit: &str,
    admitted: &str,
) -> Result<Entry, String> {
    if !report.passed() {
        return Err("cannot create an entry from a refused gate report".into());
    }
    crate::admission::inventory(bundle)?;
    let manifest_json = crate::admission::read_text(&bundle.join(octosense_app_policy::MANIFEST_FILE), crate::admission::MAX_MANIFEST_BYTES)?;
    let manifest = AppManifest::parse(&manifest_json)?;
    if serde_json::to_vec(&manifest).map_err(|e| e.to_string())? != report.admitted_manifest
        || manifest.id != report.app_id || manifest.version != report.version
        || octosense_app_policy::bundle::digest_dir_limited(bundle, MAX_BUNDLE_BYTES, crate::admission::MAX_ENTRIES, crate::admission::MAX_DEPTH)? != report.digest {
        return Err("bundle or manifest changed after the gate; check it again".into());
    }
    let signature = manifest.integrity.signature.as_ref().ok_or("public releases require a signed manifest")?;
    if publisher.is_empty() || publisher != signature.key_id {
        return Err("publisher must match the manifest signature owner".into());
    }
    crate::PublisherKeys::new().with(publisher, publisher_key)
        .verify(publisher, &signature.value, &manifest.signing_bytes()?)?;
    let listing = std::fs::read_to_string(bundle.join(LISTING_FILE)).ok().and_then(|t| Listing::parse(&t).ok());
    Ok(Entry {
        artifact: format!("artifacts/{}-{}.bundle", report.app_id, report.version),
        manifest,
        listing,
        publisher: publisher.to_string(),
        publisher_key: publisher_key.to_string(),
        source: crate::index::Source { repository: repository.to_string(), commit: commit.to_string() },
        status: crate::index::Status::Offered,
        admitted: admitted.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_app_reaches_only_the_hosts_it_declares() {
        let dir = std::env::temp_dir().join(format!("gate-script-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("main.splash"),
            "fn load(){ net.http_request({url: \"https://api.example.com/v1\"}, fn(r){}) }\nlet more = \"https://tracker.example.net/p\"\nImage{src: \"{{assets}}/a.png\"}",
        )
        .unwrap();
        let manifest = |caps: &[&str], hosts: &[&str]| {
            AppManifest::parse(&serde_json::json!({
                "schema": 1, "id": "dev.example.app", "version": "1.0.0", "name": "App",
                "integrity": {"bundle_blake3": ""}, "capabilities": caps, "network": {"hosts": hosts}
            }).to_string()).unwrap()
        };
        let files = crate::admission::inventory(&dir).unwrap();
        let refused = external_references(&dir, &files, &manifest(&["net"], &["api.example.com"])).unwrap();
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(refused[0].contains("tracker.example.net"));
        assert!(external_references(&dir, &files, &manifest(&["net"], &["api.example.com", "tracker.example.net"])).unwrap().is_empty());
        assert!(external_references(&dir, &files, &manifest(&["net", "images"], &["api.example.com"])).unwrap().is_empty(), "images reaches any public host");
        std::fs::write(dir.join("main.splash"), "let x = \"http://api.example.com\"").unwrap();
        assert!(!external_references(&dir, &files, &manifest(&["net", "web"], &["api.example.com"])).unwrap().is_empty(), "plain http is never allowed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_store_bundle_may_not_take_a_system_app_id() {
        let dir = std::env::temp_dir().join(format!("gate-os-id-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.splash"), "Label{text: \"hi\"}").unwrap();
        let write = |id: &str| {
            std::fs::write(
                dir.join(octosense_app_policy::MANIFEST_FILE),
                serde_json::json!({"schema": 1, "id": id, "version": "1.0.0", "name": "App", "integrity": {"bundle_blake3": ""}}).to_string(),
            )
            .unwrap()
        };
        let limits = HostLimits { require_signature: false, ..HostLimits::default() };
        let identity = |dir: &Path| {
            check_bundle(dir, &limits, &octosense_app_policy::RefuseAllSignatures, None)
                .unwrap()
                .findings
                .into_iter()
                .filter(|f| f.check == "identity")
                .count()
        };
        write("os.mail");
        assert_eq!(identity(&dir), 1, "os. ids belong to system apps");
        write("dev.example.mail");
        assert_eq!(identity(&dir), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bundle_that_asks_for_a_password_is_refused() {
        let dir = std::env::temp_dir().join(format!("gate-secrets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cards")).unwrap();
        std::fs::write(dir.join("main.card"), "View{ name := TextInput{empty_text: \"Name\"} }").unwrap();
        std::fs::write(dir.join("cards/login.card"), "View{ pw := TextInput{ is_password : true } }").unwrap();
        std::fs::write(dir.join("cards/otp.card"), "TextInput{content_type: TextInputContentType.OneTimeCode}").unwrap();
        std::fs::write(dir.join("notes.md"), "Set is_password: true in your own app, not here.").unwrap();
        let files = crate::admission::inventory(&dir).unwrap();
        let mut found = secret_fields(&dir, &files).unwrap();
        found.sort();
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].contains("login.card") && found[0].contains("is_password"));
        assert!(found[1].contains("otp.card") && found[1].contains("OneTimeCode"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
