//! What a publisher submits, and what the hub publishes.
//!
//! An index entry is one app version: the manifest that governs it, the hash
//! of the bundle it describes, who published it, where the source lives, and
//! whether it is still offered. A catalog is the signed list of entries a
//! device reads.
//!
//! The manifest is embedded rather than referenced so that what a reviewer
//! read, what the hub signed and what the device enforces are the same bytes.
use octosense_app_policy::{AppManifest, Listing};
use serde::{Deserialize, Serialize};

/// Whether this version is still offered, and why not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state", content = "reason")]
pub enum Status {
    /// Offered for install.
    Offered,
    /// No longer offered and, on a device, no longer runnable. The reason is
    /// shown to the person, so it is written for them.
    Withdrawn(String),
}

impl Status {
    pub fn is_offered(&self) -> bool {
        matches!(self, Status::Offered)
    }
}

/// One app version in the index.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The manifest exactly as the publisher signed it.
    pub manifest: AppManifest,
    /// The listing as reviewed: what the store shows about the app. The
    /// permissions the store shows still come from the manifest.
    #[serde(default)]
    pub listing: Option<Listing>,
    /// Where the hub's own copy of the bundle lives, relative to the catalog.
    pub artifact: String,
    /// The publisher's key identity, as the hub knows it. Update continuity
    /// is checked against this.
    pub publisher: String,
    /// The publisher's public key, hex. Distributed IN the catalog because
    /// the catalog is signed: a device can then check the publisher's
    /// signature itself, and an update signed by a different key is visible
    /// rather than silent. Empty when the app was admitted unsigned.
    #[serde(default)]
    pub publisher_key: String,
    /// Where the source lives, for transparency. Never fetched at install.
    pub source: Source,
    pub status: Status,
    /// When the hub admitted it, as an ISO 8601 date.
    pub admitted: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repository: String,
    pub commit: String,
}

impl Entry {
    pub fn app_id(&self) -> &str {
        &self.manifest.id
    }

    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    /// What the person is told this app may do, in plain words, derived from
    /// the manifest rather than from anything the app says about itself.
    pub fn permissions_summary(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for capability in &self.manifest.capabilities {
            lines.push(match capability.as_str() {
                "storage" => "Keep its own data on this device".to_string(),
                "net" if self.manifest.network.hosts.is_empty() => "Reach the network: nothing listed".to_string(),
                "net" => format!("Reach only: {}", self.manifest.network.hosts.join(", ")),
                "prompt" => "Ask you questions".to_string(),
                "ledger.read" => "Read your shared data".to_string(),
                "location" => "Use your location".to_string(),
                "camera" => "Use the camera".to_string(),
                "clipboard" => "Use the clipboard".to_string(),
                "images" => "Show pictures from any website".to_string(),
                "web" => "Open web pages in a browser view".to_string(),
                "microphone" => "Use the microphone".to_string(),
                "library" => "Save to your photo library, where other apps can see it".to_string(),
                "mail" => "Read and send mail from accounts you sign in to on the device".to_string(),
                other => format!("Use {other}"),
            });
        }
        if let Some(agent) = &self.manifest.agent {
            let tools = if agent.tools.is_empty() { "no tools".to_string() } else { agent.tools.join(", ") };
            lines.push(format!("Run an assistant for this app ({tools}), inside this app's own data only"));
        }
        if lines.is_empty() {
            lines.push("Draw its screens, and nothing else".to_string());
        }
        lines
    }
}

/// The signed list a device reads. `signature` covers [`Catalog::signing_bytes`].
#[derive(Clone, Debug)]
pub struct Catalog {
    pub schema: u32,
    /// Increases with every publish; a device refuses to go backwards, so a
    /// replayed older catalog cannot un-withdraw an app.
    pub sequence: u64,
    /// When this catalog was signed, ISO 8601. The device's freshness window
    /// is measured from here.
    pub published: String,
    pub entries: Vec<Entry>,
    /// Hex ed25519 signature by the hub's working key.
    pub signature: Option<String>,
    /// The working key that signed it, and the anchor's certificate for it.
    pub key: Option<WorkingKey>,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogV1 {
    pub schema: u32,
    /// Increases with every publish; a device refuses to go backwards, so a
    /// replayed older catalog cannot un-withdraw an app.
    pub sequence: u64,
    /// When this catalog was signed, ISO 8601. The device's freshness window
    /// is measured from here.
    pub published: String,
    pub entries: Vec<Entry>,
    /// Hex ed25519 signature by the hub's working key.
    #[serde(default)]
    pub signature: Option<String>,
    /// The working key that signed it, and the anchor's certificate for it.
    #[serde(default)]
    pub key: Option<WorkingKey>,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogV2 {
    pub schema: u32,
    /// Increases with every publish; a device refuses to go backwards, so a
    /// replayed older catalog cannot un-withdraw an app.
    pub sequence: u64,
    /// When this catalog was signed, ISO 8601. The device's freshness window
    /// is measured from here.
    pub published: String,
    pub entries: Vec<Entry>,
    /// Hex ed25519 signature by the hub's working key.
    #[serde(default)]
    pub signature: Option<String>,
    /// The working key that signed it, and the anchor's certificate for it.
    #[serde(default)]
    pub key: Option<WorkingKey>,
}

impl<'de> Deserialize<'de> for Catalog {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = octosense_app_policy::wire::value(deserializer)?;
        let catalog = match value.get("schema").and_then(serde_json::Value::as_u64) {
            Some(1) => { let w: CatalogV1 = serde_json::from_value(value).map_err(serde::de::Error::custom)?; Self { schema: w.schema, sequence: w.sequence, published: w.published, entries: w.entries, signature: w.signature, key: w.key } },
            Some(2) => { let w: CatalogV2 = serde_json::from_value(value).map_err(serde::de::Error::custom)?; Self { schema: w.schema, sequence: w.sequence, published: w.published, entries: w.entries, signature: w.signature, key: w.key } },
            schema => return Err(serde::de::Error::custom(format!("unsupported catalog schema {schema:?}"))),
        };
        catalog.validate_schema().map_err(serde::de::Error::custom)?;
        Ok(catalog)
    }
}
impl Serialize for Catalog {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate_schema().map_err(serde::ser::Error::custom)?;
        match self.schema {
            1 => CatalogV1 { schema: self.schema, sequence: self.sequence, published: self.published.clone(), entries: self.entries.clone(), signature: self.signature.clone(), key: self.key.clone() }.serialize(serializer),
            2 => CatalogV2 { schema: self.schema, sequence: self.sequence, published: self.published.clone(), entries: self.entries.clone(), signature: self.signature.clone(), key: self.key.clone() }.serialize(serializer),
            _ => Err(serde::ser::Error::custom("unsupported catalog schema")),
        }
    }
}

/// The hub's day-to-day signing key, certified by the offline anchor. A
/// device trusts the anchor only; rotating the working key is then a signed
/// statement it already knows how to check, with no shell release.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkingKey {
    /// Hex ed25519 public key.
    pub public: String,
    /// Hex ed25519 signature by the ANCHOR over the working public key bytes.
    pub anchor_certificate: String,
}

pub const CATALOG_SCHEMA: u32 = 1;

impl Catalog {
    pub fn validate_schema(&self) -> Result<(), String> {
        if !matches!(self.schema, 1 | 2) { return Err(format!("unsupported catalog schema {}", self.schema)); }
        let mut versions = std::collections::BTreeSet::new();
        let mut numbers = std::collections::BTreeSet::new();
        for entry in &self.entries {
            entry.manifest.validate_schema()?;
            if self.schema == 2 && !versions.insert((entry.app_id(), entry.version())) {
                return Err("catalog has duplicate app/version identity".into());
            }
            if let Some(contract) = entry.manifest.contract() {
                if !numbers.insert((entry.app_id(), contract.release_number)) {
                    return Err("catalog has duplicate app/release_number identity".into());
                }
            }
            if self.schema == 1 && entry.manifest.schema != 1 {
                return Err("v1 catalog may only contain v1 manifests".into());
            }
        }
        Ok(())
    }

    pub fn new(sequence: u64, published: &str, entries: Vec<Entry>) -> Self {
        Catalog {
            schema: CATALOG_SCHEMA,
            sequence,
            published: published.to_string(),
            entries,
            signature: None,
            key: None,
        }
    }

    /// The bytes the hub signs: the catalog without its signature or key, in
    /// canonical form, so the same catalog always signs the same way.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, String> {
        let mut bare = self.clone();
        bare.signature = None;
        bare.key = None;
        let value = serde_json::to_value(&bare).map_err(|e| e.to_string())?;
        Ok(canonical(&value).into_bytes())
    }

    /// The offered entry for an app id, if any.
    pub fn offered(&self, app_id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.app_id() == app_id && e.status.is_offered())
    }
}

/// Canonical JSON: sorted keys, no insignificant whitespace.
pub(crate) fn canonical(value: &serde_json::Value) -> String {
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
        serde_json::Value::Array(items) => format!("[{}]", items.iter().map(canonical).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_capability_is_told_in_plain_words() {
        let manifest = AppManifest::parse(
            &serde_json::json!({
                "schema": 1, "id": "dev.example.app", "version": "1", "name": "App",
                "integrity": {"bundle_blake3": ""},
                "capabilities": octosense_app_policy::KNOWN_CAPABILITIES,
                "network": {"hosts": ["api.example.com"]}
            })
            .to_string(),
        )
        .unwrap();
        let entry = Entry {
            artifact: String::new(),
            manifest,
            listing: None,
            publisher: String::new(),
            publisher_key: String::new(),
            source: Source { repository: String::new(), commit: String::new() },
            status: Status::Offered,
            admitted: String::new(),
        };
        let lines = entry.permissions_summary();
        assert_eq!(lines.len(), octosense_app_policy::KNOWN_CAPABILITIES.len());
        for (capability, line) in octosense_app_policy::KNOWN_CAPABILITIES.iter().zip(&lines) {
            assert_ne!(line, &format!("Use {capability}"), "{capability} has no plain-words line");
        }
    }
}

/// A trust/cache domain selected by the host, never inferred from network bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CatalogFormat { V1, V2 }
impl CatalogFormat {
    pub fn schema(self) -> u32 { match self { Self::V1 => 1, Self::V2 => 2 } }
    pub fn relative_catalog(self) -> &'static str { match self { Self::V1 => "catalog.json", Self::V2 => "v2/catalog.json" } }
    pub fn cache_filename(self) -> &'static str { match self { Self::V1 => "catalog.json", Self::V2 => "catalog-v2.json" } }
}
