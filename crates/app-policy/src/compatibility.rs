//! Versioned requirements are requests; only the host advertises availability.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRequirements {
    pub api: String,
    pub min_build: u64,
    pub platforms: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entrypoints {
    pub ui: String,
    #[serde(default)]
    pub logic: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ManifestContract {
    pub release_number: u64,
    pub runtime: RuntimeRequirements,
    pub requires: Vec<String>,
    pub entrypoints: Entrypoints,
    pub data_schema: u64,
}

impl ManifestContract {
    pub(crate) fn validate(&self, version: &str) -> Result<(), String> {
        semver::Version::parse(version).map_err(|e| format!("v2 version must be SemVer: {e}"))?;
        if self.release_number == 0 { return Err("release_number must be positive".into()); }
        if self.runtime.api.is_empty() || !self.runtime.api.bytes().all(|c| c.is_ascii_digit()) || self.runtime.min_build == 0 {
            return Err("runtime requires a numeric API and positive min_build".into());
        }
        if self.runtime.platforms.is_empty() || self.runtime.platforms.iter().any(|p| !matches!(p.as_str(), "android" | "macos" | "ios" | "windows" | "linux" | "web")) {
            return Err("runtime.platforms must name supported platform identifiers".into());
        }
        for feature in &self.requires {
            let Some((name, version)) = feature.split_once('@') else { return Err(format!("required feature {feature:?} must include @version")); };
            if name.is_empty() || !name.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'-')
                || version.parse::<u32>().ok().filter(|n| *n > 0).is_none() {
                return Err(format!("invalid required feature {feature:?}"));
            }
        }
        for path in std::iter::once(&self.entrypoints.ui).chain(self.entrypoints.logic.iter()) {
            if path.is_empty() || path.len() > 256 || !path.is_ascii() || path.contains(['\\', ':', '?', '#', '%'])
                || path.split('/').any(|p| p.is_empty() || p == "." || p == ".." || p.bytes().any(|b| b.is_ascii_control())) {
                return Err("entrypoint must be a portable relative bundle path".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDescriptor {
    pub api: String,
    pub build: u64,
    pub platform: String,
    pub features: std::collections::BTreeSet<String>,
    pub capabilities: std::collections::BTreeSet<String>,
}
impl RuntimeDescriptor {
    /// Release-managed baseline. Additional services are advertised only when
    /// their real host adapter is installed, never from app listing metadata.
    pub fn current() -> Self {
        Self { api: "1".into(), build: 1, platform: std::env::consts::OS.into(),
            features: ["card.ui@1".into(), "script.ui@1".into()].into(),
            capabilities: ["storage".into(), "net".into(), "prompt".into()].into() }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityReason { pub code: String, pub message: String }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilityResult { pub compatible: bool, pub reasons: Vec<CompatibilityReason> }
impl CompatibilityResult {
    pub fn require(&self) -> Result<(), String> {
        if self.compatible { Ok(()) } else { Err(self.reasons.iter().map(|r| r.message.as_str()).collect::<Vec<_>>().join("; ")) }
    }
}
pub fn evaluate(manifest: &crate::AppManifest, runtime: &RuntimeDescriptor) -> CompatibilityResult {
    let mut reasons = Vec::new();
    let mut refuse = |code: &str, message: String| reasons.push(CompatibilityReason { code: code.into(), message });
    if let Err(error) = manifest.validate_schema() { refuse("manifest", error); }
    if let Some(contract) = manifest.contract() {
        let required = &contract.runtime;
        if required.api != runtime.api { refuse("runtime-api", format!("Requires runtime API {}; this host provides {}", required.api, runtime.api)); }
        if required.min_build > runtime.build { refuse("runtime-build", format!("Requires runtime build {} or newer; this host is build {}", required.min_build, runtime.build)); }
        if !required.platforms.contains(&runtime.platform) { refuse("platform", format!("Requires {}; this host is {}", required.platforms.join(" or "), runtime.platform)); }
        for feature in &contract.requires {
            if !runtime.features.contains(feature) { refuse("feature", format!("Requires unavailable runtime feature {feature}")); }
        }
        for capability in &manifest.capabilities {
            if !runtime.capabilities.contains(capability) { refuse("capability", format!("This host does not provide the {capability} capability")); }
        }
        let ui_supported = match contract.entrypoints.ui.as_str() {
            "page.card" => runtime.features.contains("card.ui@1"),
            "main.splash" => runtime.features.contains("script.ui@1"),
            _ => false,
        };
        if !ui_supported || (contract.entrypoints.logic.is_some() && !runtime.features.contains("app.logic@1")) {
            refuse("entrypoint", "This runtime does not provide the requested application entrypoints".into());
        }
        if manifest.agent.is_some() && !runtime.features.contains("agent.session@1") {
            refuse("feature", "This runtime does not provide contained agent sessions".into());
        }
    }
    CompatibilityResult { compatible: reasons.is_empty(), reasons }
}
