//! What an installed app declares about itself.
//!
//! The manifest is the ONLY thing an app may say about its own limits, and
//! saying it is not the same as getting it: every field is a request that
//! [`crate::policy`] resolves against the host's ceilings. Unknown fields are
//! refused rather than ignored, so a manifest written for a newer host does
//! not silently run with less containment than it asked for.
use serde::{Deserialize, Serialize};

/// Legacy schema used by existing templates. The versioned reader supports 1 and 2.
pub const SCHEMA: u32 = 1;

/// Every capability an app may request. The list is closed on purpose — a
/// capability that is not here cannot be granted, so adding one is a change
/// to this file and to the service that enforces it, together.
pub const KNOWN_CAPABILITIES: &[&str] = &[
    // Read and write inside the app's own storage jail.
    "storage",
    // Make requests, but only to the hosts in `network.hosts`.
    "net",
    // Raise a prompt the person answers (a permission ask, a confirmation).
    "prompt",
    // Read the shared ledger. Writing is always the app's own rows.
    "ledger.read",
    // Location, camera and clipboard reach the person's world; each is a
    // separate consent, never implied by another.
    "location",
    "camera",
    "clipboard",
    // Show pictures from any public https host, not just `network.hosts`:
    // a feed reader's thumbnails come from wherever its stories link.
    "images",
    // Open any public https page in the system WebView, which gets no way
    // back into the app: a reader for the stories it lists.
    "web",
    // Record sound with a camera video.
    "microphone",
    // Offer what it captures to the system photo library, where other apps
    // can see it; without this, captures stay in the app's own storage.
    "library",
    // Read and send mail through the host's mail service, from accounts the
    // person signs in to on the host's own sheet. The app never holds the
    // password or the connection.
    "mail",
];

/// The permission profiles an app's agent session may ask for. Full access is
/// absent by construction: it is an operator setting for machines they own,
/// and no manifest may name it (ADR 0002, "the rule at the boundary").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileMode {
    /// No writes at all; every write asks.
    ReadOnly,
    /// Read and write inside the workspace; anything else asks.
    WorkspaceWrite,
    /// Read and write inside the workspace; anything else is refused
    /// outright rather than asked. The right default for an unattended app.
    WorkspaceWriteNeverAsk,
}

impl ProfileMode {
    /// The string the kernel's permission profile uses.
    pub fn as_kernel_mode(self) -> &'static str {
        match self {
            ProfileMode::ReadOnly => "read-only",
            ProfileMode::WorkspaceWrite => "workspace-write",
            ProfileMode::WorkspaceWriteNeverAsk => "workspace-write-never",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AppManifest {
    /// Version of the signed wire representation.
    pub schema: u32,
    /// Stable identity. Also the name of the app's storage jail, so it is
    /// constrained to the characters a path component may hold.
    pub id: String,
    /// Opaque for v1; SemVer display label for v2. Both are immutable identities.
    pub version: String,
    /// What a person calls it.
    pub name: String,
    pub integrity: Integrity,
    pub capabilities: Vec<String>,
    pub network: Network,
    pub storage: Storage,
    pub compute: Compute,
    /// Absent means the app gets no agent at all, which is the default.
    pub agent: Option<AgentSpec>,
    contract: Option<crate::compatibility::ManifestContract>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestV1 {
    /// Version of the signed wire representation.
    pub schema: u32,
    /// Stable identity. Also the name of the app's storage jail, so it is
    /// constrained to the characters a path component may hold.
    pub id: String,
    /// Opaque for v1; SemVer display label for v2. Both are immutable identities.
    pub version: String,
    /// What a person calls it.
    pub name: String,
    pub integrity: Integrity,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub storage: Storage,
    #[serde(default)]
    pub compute: Compute,
    /// Absent means the app gets no agent at all, which is the default.
    #[serde(default)]
    pub agent: Option<AgentSpec>,
}


#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ManifestV2 {
    /// Version of the signed wire representation.
    pub schema: u32,
    /// Stable identity. Also the name of the app's storage jail, so it is
    /// constrained to the characters a path component may hold.
    pub id: String,
    /// Opaque for v1; SemVer display label for v2. Both are immutable identities.
    pub version: String,
    /// What a person calls it.
    pub name: String,
    pub integrity: Integrity,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub storage: Storage,
    #[serde(default)]
    pub compute: Compute,
    /// Absent means the app gets no agent at all, which is the default.
    #[serde(default)]
    pub agent: Option<AgentSpec>,
    release_number: u64,
    runtime: crate::compatibility::RuntimeRequirements,
    requires: Vec<String>,
    entrypoints: crate::compatibility::Entrypoints,
    data_schema: u64,
}
// Freeze the original representation: v2 fields never enter v1 signing bytes.
impl<'de> Deserialize<'de> for AppManifest {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = crate::wire::value(deserializer)?;
        let manifest = match value.get("schema").and_then(serde_json::Value::as_u64) {
            Some(1) => { let w: ManifestV1 = serde_json::from_value(value).map_err(serde::de::Error::custom)?; Self { schema: w.schema, id: w.id, version: w.version, name: w.name, integrity: w.integrity, capabilities: w.capabilities, network: w.network, storage: w.storage, compute: w.compute, agent: w.agent, contract: None } },
            Some(2) => { let w: ManifestV2 = serde_json::from_value(value).map_err(serde::de::Error::custom)?; Self { schema: w.schema, id: w.id, version: w.version, name: w.name, integrity: w.integrity, capabilities: w.capabilities, network: w.network, storage: w.storage, compute: w.compute, agent: w.agent, contract: Some(crate::compatibility::ManifestContract { release_number: w.release_number, runtime: w.runtime, requires: w.requires, entrypoints: w.entrypoints, data_schema: w.data_schema }) } },
            schema => return Err(serde::de::Error::custom(format!("unsupported manifest schema {}", schema.map(|s| s.to_string()).unwrap_or_else(|| "(missing or invalid)".into())))),
        };
        manifest.validate_schema().map_err(serde::de::Error::custom)?;
        Ok(manifest)
    }
}
impl Serialize for AppManifest {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate_schema().map_err(serde::ser::Error::custom)?;
        match &self.contract {
            None => ManifestV1 { schema: self.schema, id: self.id.clone(), version: self.version.clone(), name: self.name.clone(), integrity: self.integrity.clone(), capabilities: self.capabilities.clone(), network: self.network.clone(), storage: self.storage.clone(), compute: self.compute.clone(), agent: self.agent.clone() }.serialize(serializer),
            Some(c) => ManifestV2 { schema: self.schema, id: self.id.clone(), version: self.version.clone(), name: self.name.clone(), integrity: self.integrity.clone(), capabilities: self.capabilities.clone(), network: self.network.clone(), storage: self.storage.clone(), compute: self.compute.clone(), agent: self.agent.clone(), release_number: c.release_number, runtime: c.runtime.clone(), requires: c.requires.clone(), entrypoints: c.entrypoints.clone(), data_schema: c.data_schema }.serialize(serializer),
        }
    }
}

/// What the bundle must hash to. The digest covers the bundle bytes as they
/// were signed; a signature, when we have one, signs this manifest.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Integrity {
    /// Lowercase hex blake3 of the bundle.
    pub bundle_blake3: String,
    /// Detached signature over the canonical manifest bytes, if the host
    /// requires signing. Verified by a [`crate::verify::SignatureVerifier`].
    #[serde(default)]
    pub signature: Option<Signature>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Signature {
    /// Which key signed it, as the host knows the key.
    pub key_id: String,
    /// Lowercase hex signature bytes.
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Network {
    /// The hosts this app may reach, exactly. No wildcards, no schemes, no
    /// paths: a host and nothing else, and the request is HTTPS by the time
    /// the service makes it.
    #[serde(default)]
    pub hosts: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Storage {
    /// Whole-jail ceiling the app asks for. Clamped to the host's maximum.
    #[serde(default)]
    pub max_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Compute {
    /// Script instructions the app may run per session, cumulative — not the
    /// per-evaluation cap, which only stops one runaway expression.
    #[serde(default)]
    pub instruction_budget: Option<u64>,
    /// Ceiling for the isolate's heap.
    #[serde(default)]
    pub memory_bytes: Option<u64>,
}

/// The agent session an app asks for. Everything here is bounded by the same
/// manifest: its workspace is the app's jail, its network is the app's hosts.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSpec {
    pub profile: ProfileMode,
    /// The tools the session may call. Closed list, resolved against the
    /// host's own allowlist; shell and arbitrary file tools are never in it.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Model turns per request before the session stops and reports.
    #[serde(default)]
    pub max_iterations: Option<u32>,
    /// Tokens the session may spend per request.
    #[serde(default)]
    pub token_budget: Option<u64>,
}

impl AppManifest {
    pub fn contract(&self) -> Option<&crate::compatibility::ManifestContract> { self.contract.as_ref() }

    pub fn validate_schema(&self) -> Result<(), String> {
        match (self.schema, &self.contract) {
            (1, None) => Ok(()),
            (2, Some(contract)) => contract.validate(&self.version),
            _ => Err(format!("unsupported or inconsistent manifest schema {}", self.schema)),
        }
    }

    /// Parse a manifest, refusing unknown fields and a foreign schema.
    pub fn parse(json: &str) -> Result<Self, String> {
        let manifest: AppManifest = serde_json::from_str(json).map_err(|e| format!("manifest is not valid: {e}"))?;
        Ok(manifest)
    }

    /// The bytes a signature covers: the manifest without its own signature,
    /// serialised canonically, so the same manifest always signs the same way.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bare = self.clone();
        bare.integrity.signature = None;
        let value = serde_json::to_value(&bare).map_err(|e| e.to_string())?;
        Ok(canonical(&value).into_bytes())
    }
}

/// Canonical JSON: object keys sorted, no insignificant whitespace. Enough
/// for a stable signing input; it is not a general JCS implementation.
fn canonical(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let body: Vec<String> = keys
                .iter()
                .map(|k| format!("{}:{}", serde_json::Value::String((*k).clone()), canonical(&map[*k])))
                .collect();
            format!("{{{}}}", body.join(","))
        }
        serde_json::Value::Array(items) => {
            let body: Vec<String> = items.iter().map(canonical).collect();
            format!("[{}]", body.join(","))
        }
        other => other.to_string(),
    }
}
