//! Read-only distribution monitoring. Can run without any signing credentials.
use crate::{Catalog, Entry, Pack, Remote, verify_catalog};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthLevel { Healthy, Warning, Critical, Expired }
#[derive(Debug, Serialize)]
pub struct Health {
    pub schema: u32,
    pub sequence: u64,
    pub published: String,
    pub age_days: u64,
    pub level: HealthLevel,
}

pub fn health(catalog: &Catalog, anchor: &str, now: &str) -> Result<Health, String> {
    verify_catalog(catalog, anchor)?;
    if !matches!(catalog.schema, 1 | 2) { return Err("unsupported catalog schema".into()); }
    crate::release::date(now)?;
    crate::release::valid_date_at(&catalog.published, now)?;
    let age_days = crate::days_between(&catalog.published, now).ok_or("invalid catalog date")?;
    let level = if age_days > crate::CATALOG_FRESHNESS_DAYS { HealthLevel::Expired }
        else if age_days >= 12 { HealthLevel::Critical }
        else if age_days >= 7 { HealthLevel::Warning } else { HealthLevel::Healthy };
    Ok(Health { schema: 1, sequence: catalog.sequence, published: catalog.published.clone(), age_days, level })
}

pub trait DistributionSource {
    fn catalog(&self) -> Result<String, String>;
    fn pack(&self, artifact: &str) -> Result<Pack, String>;
}
impl DistributionSource for Remote {
    fn catalog(&self) -> Result<String, String> { Remote::catalog(self) }
    fn pack(&self, artifact: &str) -> Result<Pack, String> { Remote::pack(self, artifact) }
}

#[derive(Debug, Serialize)]
pub struct Probe {
    pub schema: u32,
    pub passed: bool,
    pub health: Health,
    pub offered_artifacts: usize,
    pub checked_artifacts: usize,
    pub sampled: bool,
    pub findings: Vec<crate::Finding>,
}

/// Checks a signed catalog and a bounded sample of its newest offered packs.
/// `minimum_sequence` comes from trusted local state/an independent monitor.
pub fn probe(source: &impl DistributionSource, anchor: &str, minimum_sequence: u64, now: &str, max_artifacts: usize) -> Result<Probe, String> {
    if !(1..=10).contains(&max_artifacts) { return Err("probe sample size must be 1 to 10".into()); }
    let text = source.catalog()?;
    if text.len() > 64 * 1024 * 1024 { return Err("catalog exceeds probe size limit".into()); }
    let catalog: Catalog = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let health = health(&catalog, anchor, now)?;
    if catalog.sequence < minimum_sequence { return Err(format!("public catalog sequence {} is below expected {minimum_sequence}", catalog.sequence)); }
    crate::publishers::CatalogPublishers::from_catalog(&catalog)?;
    let offered: Vec<&Entry> = catalog.entries.iter().filter(|entry| entry.status.is_offered()).collect();
    let mut nonce = [0u8; 16];
    rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut nonce).map_err(|e| e.to_string())?;
    let scratch = std::env::temp_dir().join(format!("hub-distribution-probe-{}", hex::encode(nonce)));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
    builder.create(&scratch).map_err(|e| e.to_string())?;
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    let scratch = Scratch(scratch);
    let mut findings = vec![];
    let mut checked_artifacts = 0;
    for entry in offered.iter().rev().take(max_artifacts) {
        let checked = (|| {
            crate::admission::safe_relative(&entry.artifact)?;
            // The remote adapter interpolates a relative URL; refuse URL query,
            // fragment and percent-encoded navigation syntax in legacy paths.
            if entry.artifact.contains(['?', '#', '%']) { return Err("artifact path contains URL syntax".into()); }
            let pack = source.pack(&entry.artifact)?;
            crate::release::verify_pack(&pack, entry, &scratch.0)
        })();
        checked_artifacts += 1;
        if let Err(error) = checked { findings.push(crate::Finding::at("artifact-probe-failed", &entry.artifact, error)); }
    }
    Ok(Probe { schema: 1, passed: health.level == HealthLevel::Healthy && findings.is_empty(), health,
        offered_artifacts: offered.len(), checked_artifacts, sampled: offered.len() > checked_artifacts, findings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HubKey, test_bundle::Fixture};
    struct Source { catalog: String, pack: String }
    impl DistributionSource for Source {
        fn catalog(&self) -> Result<String, String> { Ok(self.catalog.clone()) }
        fn pack(&self, _: &str) -> Result<Pack, String> { serde_json::from_str(&self.pack).map_err(|e| e.to_string()) }
    }
    #[test]
    fn renewal_failure_warns_and_escalates_before_install_expiry() {
        let anchor = HubKey::generate();
        let working = HubKey::generate();
        let mut catalog = Catalog::new(1, "2026-09-01", vec![]);
        working.sign_catalog(&mut catalog, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
        for (now, expected) in [("2026-09-07", HealthLevel::Healthy), ("2026-09-08", HealthLevel::Warning),
            ("2026-09-13", HealthLevel::Critical), ("2026-09-15", HealthLevel::Critical), ("2026-09-16", HealthLevel::Expired)] {
            assert_eq!(health(&catalog, &anchor.public_hex(), now).unwrap().level, expected);
        }
        assert!(health(&catalog, &anchor.public_hex(), "2026-08-31").is_err());
        catalog.published = "2026-02-30".into();
        working.sign_catalog(&mut catalog, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
        assert!(health(&catalog, &anchor.public_hex(), "2026-09-25").is_err());
    }
    #[test]
    fn public_probes_detect_replay_bad_signatures_and_changed_pack_bytes() {
        let f = Fixture::new();
        let anchor = HubKey::generate();
        let working = HubKey::generate();
        let mut catalog = Catalog::new(10, "2026-09-25", vec![f.entry()]);
        working.sign_catalog(&mut catalog, &anchor.certify(&working.public_hex()).unwrap()).unwrap();
        let mut source = Source { catalog: serde_json::to_string(&catalog).unwrap(), pack: serde_json::to_string(&crate::pack_dir(&f.bundle).unwrap()).unwrap() };
        let valid = probe(&source, &anchor.public_hex(), 10, "2026-09-25", 10).unwrap();
        assert!(valid.passed);
        assert_eq!(valid.checked_artifacts, 1);
        assert!(!valid.sampled);
        assert!(probe(&source, &anchor.public_hex(), 11, "2026-09-25", 10).is_err());
        assert!(probe(&source, &HubKey::generate().public_hex(), 10, "2026-09-25", 10).is_err());
        std::fs::write(f.bundle.join("page.card"), "changed bytes").unwrap();
        source.pack = serde_json::to_string(&crate::pack_dir(&f.bundle).unwrap()).unwrap();
        let invalid = probe(&source, &anchor.public_hex(), 10, "2026-09-25", 10).unwrap();
        assert!(!invalid.passed);
        assert_eq!(invalid.findings[0].check, "artifact-probe-failed");
    }
}
