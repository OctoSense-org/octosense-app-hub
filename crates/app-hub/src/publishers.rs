//! Trusted publisher identity, separate from a submission's key declarations.
//!
//! V1 uses the publisher ID as its key ID. It has no authorized rotation
//! records: conflicting historical bindings require operator reconciliation.
use crate::{Catalog, PublisherKeys};
use octosense_app_policy::{AppManifest, SignatureVerifier};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublisherBinding {
    pub publisher_id: String,
    pub key_id: String,
    pub public_key_hex: String,
}

/// Read-only identity lookup. Future authenticated registry/rotation records
/// can implement this without granting submissions permission to change keys.
pub trait PublisherRegistry {
    fn binding(&self, key_id: &str) -> Option<&PublisherBinding>;
    fn app_owner(&self, app_id: &str) -> Option<&str>;
}

#[derive(Default)]
pub struct CatalogPublishers {
    bindings: BTreeMap<String, PublisherBinding>,
    owners: BTreeMap<String, String>,
}

impl CatalogPublishers {
    /// `catalog` must already be authenticated by the caller. Check *all*
    /// entries, including withdrawals, rather than letting order pick a key.
    pub fn from_catalog(catalog: &Catalog) -> Result<Self, String> {
        Self::from_catalogs(&[catalog])
    }

    pub fn from_catalogs(catalogs: &[&Catalog]) -> Result<Self, String> {
        let mut registry = Self::default();
        for entry in catalogs.iter().flat_map(|catalog| &catalog.entries) {
            let signature = entry.manifest.integrity.signature.as_ref()
                .ok_or("legacy unsigned publisher binding requires operator reconciliation")?;
            if signature.key_id != entry.publisher || entry.publisher.is_empty() {
                return Err("catalog publisher does not match the manifest signature owner".into());
            }
            let binding = PublisherBinding {
                publisher_id: entry.publisher.clone(),
                key_id: signature.key_id.clone(),
                public_key_hex: entry.publisher_key.to_ascii_lowercase(),
            };
            if let Some(previous) = registry.bindings.get(&binding.key_id) {
                if previous != &binding {
                    return Err(format!("conflicting catalog bindings for {:?}; operator reconciliation required", binding.key_id));
                }
            }
            if let Some(owner) = registry.owners.get(entry.app_id()) {
                if owner != &entry.publisher {
                    return Err(format!("conflicting catalog owners for {}; operator reconciliation required", entry.app_id()));
                }
            }
            PublisherKeys::new().with(&binding.key_id, &binding.public_key_hex)
                .verify(&signature.key_id, &signature.value, &entry.manifest.signing_bytes()?)?;
            registry.owners.insert(entry.app_id().to_string(), entry.publisher.clone());
            registry.bindings.insert(binding.key_id.clone(), binding);
        }
        Ok(registry)
    }
}

impl PublisherRegistry for CatalogPublishers {
    fn binding(&self, key_id: &str) -> Option<&PublisherBinding> { self.bindings.get(key_id) }
    fn app_owner(&self, app_id: &str) -> Option<&str> { self.owners.get(app_id).map(String::as_str) }
}

/// Verify continuity using trusted bytes, never a caller's replacement map.
pub fn verify_continuity(manifest: &AppManifest, registry: &dyn PublisherRegistry) -> Result<(), String> {
    let owner = registry.app_owner(&manifest.id);
    let Some(signature) = &manifest.integrity.signature else {
        return if owner.is_some() { Err("an update must carry its registered publisher signature".into()) } else { Ok(()) };
    };
    let binding = registry.binding(&signature.key_id);
    if let Some(owner) = owner {
        if binding.is_none_or(|b| b.publisher_id != owner) {
            return Err(format!("{} belongs to {owner:?}; unknown key/rotation requires registry authorization", manifest.id));
        }
    }
    if let Some(binding) = binding {
        PublisherKeys::new().with(&binding.key_id, &binding.public_key_hex)
            .verify(&signature.key_id, &signature.value, &manifest.signing_bytes()?)
            .map_err(|e| format!("registered publisher key continuity failed: {e}"))?;
    }
    Ok(())
}
