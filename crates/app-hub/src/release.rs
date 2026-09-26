//! Operator release transactions. The state directory is private durable
//! signing history, backed up separately from the public catalog/artifacts.
use crate::{Catalog, CatalogFormat, verify_catalog, signing::CatalogSigner, publishers::PublisherRegistry};
use serde::{Deserialize, Serialize};
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}};

/// Only release operations can construct a transaction for the signer.
/// Renewal preserves releases exactly; publication must add validated evidence.
pub struct ValidatedCatalog { catalog: Catalog }
impl ValidatedCatalog { pub fn catalog(&self) -> &Catalog { &self.catalog } }

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    request_hash: String,
    previous_hash: String,
    previous_sequence: u64,
    catalog: Catalog,
}

const MAX_RECORD_BYTES: usize = 64 * 1024 * 1024;
// Leave room for signature, key certificate and transaction metadata in records.
const MAX_CATALOG_BYTES: usize = MAX_RECORD_BYTES - 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    idempotency_hash: String,
    request_hash: String,
    previous_hash: String,
    previous_sequence: u64,
    catalog: Catalog,
}

enum Change<'a> {
    Renew,
    Recover,
    Publish(&'a crate::approval::ApprovedRelease),
    Withdraw { app: &'a str, version: &'a str, reason: &'a str },
}
impl Change<'_> {
    fn identity(&self) -> serde_json::Value {
        match self {
            Self::Renew => serde_json::json!({"operation":"renew"}),
            Self::Recover => serde_json::json!({"operation":"recover"}),
            Self::Publish(release) => serde_json::json!({"operation":"publish","approval":release.id}),
            Self::Withdraw { app, version, reason } => serde_json::json!({"operation":"withdraw","app":app,"version":version,"reason":reason}),
        }
    }
    fn candidate(&self, current: &Catalog, now: &str, registry: &crate::publishers::CatalogPublishers) -> Result<Catalog, String> {
        let mut next = current.clone();
        match self {
            Self::Renew | Self::Recover => { if current.published.is_empty() { return Err("renewal requires an existing signed catalog".into()); } }
            Self::Publish(release) => {
                if current.entries.iter().any(|entry| entry.app_id() == release.entry.app_id() && entry.version() == release.entry.version()) {
                    return Err("this app version is already in the catalog".into());
                }
                if let Some(contract) = release.entry.manifest.contract() {
                    let highest = current.entries.iter()
                        .filter(|entry| entry.app_id() == release.entry.app_id())
                        .filter_map(|entry| entry.manifest.contract().map(|contract| contract.release_number))
                        .max().unwrap_or(0);
                    if contract.release_number <= highest {
                        return Err(format!("release_number {} must be greater than the existing release {}", contract.release_number, highest));
                    }
                }
                crate::publishers::verify_continuity(&release.entry.manifest, registry)?;
                let mut entry = release.entry.clone();
                entry.admitted = now.into();
                next.entries.push(entry);
            }
            Self::Withdraw { app, version, reason } => {
                if reason.trim().is_empty() || reason.len() > 4096 { return Err("withdrawal needs a reason of 1 to 4096 bytes".into()); }
                let entry = next.entries.iter_mut().find(|entry| entry.app_id() == *app && entry.version() == *version)
                    .ok_or("withdrawal target is not in the catalog")?;
                entry.status = crate::Status::Withdrawn((*reason).into());
            }
        }
        next.sequence = next.sequence.checked_add(1).ok_or("catalog sequence exhausted")?;
        next.published = now.into();
        next.signature = None;
        next.key = None;
        Ok(next)
    }
    fn stage(&self, store: &ReleaseStore, hook: &impl Fn(&str) -> Result<(), String>) -> Result<(), String> {
        if let Self::Publish(release) = self { release.stage(store.catalog_path.parent().unwrap(), &store.state, hook)?; }
        Ok(())
    }
}

pub struct ReleaseStore {
    catalog_path: PathBuf,
    state: PathBuf,
    ownership: PathBuf,
    anchor: String,
    catalog: Catalog,
    _domain_lock: File,
    _lock: File,
}

impl ReleaseStore {
    /// Locks the catalog's canonical sibling lock for the entire transaction.
    /// All writers must use this boundary; manual catalog edits are unsupported.
    pub fn open(catalog_path: &Path, state: &Path, anchor: &str) -> Result<Self, String> {
        Self::open_format(catalog_path, state, anchor, CatalogFormat::V1)
    }

    pub fn open_format(catalog_path: &Path, state: &Path, anchor: &str, format: CatalogFormat) -> Result<Self, String> {
        let parent = catalog_path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let parent = parent.canonicalize().map_err(|e| e.to_string())?;
        let catalog_path = parent.join(catalog_path.file_name().ok_or("catalog filename is missing")?);
        if format == CatalogFormat::V1 && catalog_path.file_name().is_none_or(|name| name != "catalog.json") {
            return Err("v1 catalog must be at <public root>/catalog.json for shared publisher ownership".into());
        }
        let public_root = if format == CatalogFormat::V2 {
            if parent.file_name().is_none_or(|name| name != "v2")
                || catalog_path.file_name().is_none_or(|name| name != "catalog.json") {
                return Err("v2 catalog must be at <public root>/v2/catalog.json".into());
            }
            parent.parent().ok_or("v2 catalog has no public root")?
        } else { parent.as_path() };
        // All schema writers take the same lock before reading either catalog
        // or their shared publisher reservations.
        let domain_lock = lock_file(&public_root.join(".release-domain.lock"))?;
        if fs::symlink_metadata(&catalog_path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err("catalog pointer may not be a symlink".into());
        }
        let lock_path = catalog_path.with_file_name(format!("{}.release.lock", catalog_path.file_name().unwrap().to_string_lossy()));
        let lock = lock_file(&lock_path)?;
        let catalog: Catalog = if catalog_path.exists() {
            let catalog = read_json(&catalog_path)?;
            verify_catalog(&catalog, anchor)?;
            catalog
        } else {
            let mut catalog = Catalog::new(0, "", vec![]);
            catalog.schema = format.schema();
            catalog
        };
        if catalog.schema != format.schema() { return Err("catalog schema does not match the selected operator format".into()); }
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
        builder.create(state).map_err(|e| e.to_string())?;
        if fs::symlink_metadata(state).map_err(|e| e.to_string())?.file_type().is_symlink() { return Err("release state may not be a symlink".into()); }
        let state = state.canonicalize().map_err(|e| e.to_string())?;
        if state.starts_with(public_root) { return Err("release state must be outside the public distribution root".into()); }
        let private_root = state.parent().ok_or("release state has no private parent")?;
        let root_hash = blake3::hash(public_root.to_string_lossy().as_bytes()).to_hex().to_string();
        let ownership = private_root.join(format!(".app-hub-owners-{root_hash}"));
        let ownership_binding = blake3::hash(ownership.to_string_lossy().as_bytes()).to_hex().to_string();
        let marker = public_root.join(".release-ownership.json");
        if fs::symlink_metadata(&marker).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err("release ownership marker may not be a symlink".into());
        }
        if marker.exists() {
            let bound: String = read_json(&marker)?;
            if bound != ownership_binding {
                return Err("v1 and v2 release state must share one private parent directory".into());
            }
        } else {
            immutable_json(&marker, &ownership_binding)?;
        }
        sync_parent(&marker)?;
        if fs::symlink_metadata(&ownership).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err("publisher ownership directory may not be a symlink".into());
        }
        builder.create(&ownership).map_err(|e| e.to_string())?;
        sync_parent(&ownership)?;
        if state.join("head.json").exists() {
            let head: Record = read_json(&state.join("head.json"))?;
            if head.catalog.schema != format.schema() {
                return Err("release state belongs to another catalog schema".into());
            }
        }
        fs::create_dir_all(state.join("requests")).map_err(|e| e.to_string())?;
        sync_parent(&state)?;
        sync_parent(&state.join("requests"))?;
        let store = Self { catalog_path, state, ownership, anchor: anchor.into(), catalog, _domain_lock: domain_lock, _lock: lock };
        store.ownership_registry()?;
        Ok(store)
    }

    fn ownership_registry(&self) -> Result<crate::publishers::CatalogPublishers, String> {
        let sibling = if self.catalog.schema == 2 {
            self.catalog_path.parent().unwrap().parent().unwrap().join("catalog.json")
        } else { self.catalog_path.parent().unwrap().join("v2/catalog.json") };
        let mut catalogs = Vec::new();
        if sibling.exists() {
            if fs::symlink_metadata(&sibling).map_err(|e| e.to_string())?.file_type().is_symlink() {
                return Err("sibling catalog pointer may not be a symlink".into());
            }
            let catalog: Catalog = read_json(&sibling)?;
            verify_catalog(&catalog, &self.anchor)?;
            if catalog.schema == self.catalog.schema { return Err("sibling catalog schema is wrong".into()); }
            catalogs.push(catalog);
        }
        let mut proofs = Vec::new();
        for item in fs::read_dir(&self.ownership).map_err(|e| e.to_string())? {
            let path = item.map_err(|e| e.to_string())?.path();
            let name = path.file_name().unwrap().to_string_lossy();
            if name.len() != 69 || !name.ends_with(".json") || !name[..64].bytes().all(|b| b.is_ascii_hexdigit()) { continue; }
            if fs::symlink_metadata(&path).map_err(|e| e.to_string())?.file_type().is_symlink() {
                return Err("publisher ownership record may not be a symlink".into());
            }
            let catalog: Catalog = read_json(&path)?;
            verify_catalog(&catalog, &self.anchor)?;
            if hash(&catalog)? != name[..64] { return Err("publisher ownership proof has the wrong digest".into()); }
            proofs.push(catalog);
        }
        // A previous hard-link can be visible even when its directory fsync
        // failed. Make every validated link durable before trusting it.
        File::open(&self.ownership).and_then(|file| file.sync_all()).map_err(|e| e.to_string())?;
        let mut proven_apps: std::collections::HashSet<String> = proofs.iter()
            .flat_map(|catalog| catalog.entries.iter().map(|entry| entry.app_id().to_string())).collect();
        let mut all = vec![&self.catalog];
        all.extend(catalogs.iter());
        all.extend(proofs.iter());
        let registry = crate::publishers::CatalogPublishers::from_catalogs(&all)?;
        // Seed authenticated v1/v2 history before accepting a new publisher.
        for catalog in std::iter::once(&self.catalog).chain(catalogs.iter()) {
            if catalog.published.is_empty() { continue; }
            if catalog.entries.iter().any(|entry| !proven_apps.contains(entry.app_id())) {
                let path = self.proof_path(catalog)?;
                if !path.exists() { immutable_json(&path, catalog)?; }
                sync_parent(&path)?;
                proven_apps.extend(catalog.entries.iter().map(|entry| entry.app_id().to_string()));
            }
        }
        Ok(registry)
    }

    fn proof_path(&self, catalog: &Catalog) -> Result<PathBuf, String> {
        Ok(self.ownership.join(format!("{}.json", hash(catalog)?)))
    }

    fn reserve_owner(&self, catalog: &Catalog, app_id: &str, signer: &impl CatalogSigner) -> Result<(), String> {
        if catalog.signature.is_some() { verify_catalog(catalog, &self.anchor)?; }
        if self.ownership_registry()?.app_owner(app_id).is_some() { return Ok(()); }
        let entry = catalog.entries.iter().find(|entry| entry.app_id() == app_id)
            .ok_or("ownership reservation has no matching app")?;
        let mut proof = Catalog::new(1, &catalog.published, vec![entry.clone()]);
        proof.schema = catalog.schema;
        let signed = signer.sign(&ValidatedCatalog { catalog: proof.clone() })?;
        if signed.signing_bytes()? != proof.signing_bytes()? {
            return Err("signer changed the validated publisher reservation".into());
        }
        proof = signed;
        verify_catalog(&proof, &self.anchor)?;
        let path = self.proof_path(&proof)?;
        if !path.exists() { immutable_json(&path, &proof)?; }
        sync_parent(&path)?;
        Ok(())
    }

    pub fn catalog(&self) -> &Catalog { &self.catalog }

    /// `now` is supplied by the trusted operator clock, never by an app.
    pub fn renew(self, expected: u64, idempotency_key: &str, now: &str, signer: &impl CatalogSigner) -> Result<Catalog, String> {
        self.renew_with_hook(expected, idempotency_key, now, signer, |_| Ok(()))
    }

    /// Scheduled renewals can select the current generation under the lock,
    /// while a retry retains its original expected sequence.
    pub fn renew_current(self, idempotency_key: &str, now: &str, signer: &impl CatalogSigner) -> Result<Catalog, String> {
        let key = blake3::hash(idempotency_key.as_bytes()).to_hex().to_string();
        let receipt = self.state.join("requests").join(format!("{key}.json"));
        let expected = if receipt.exists() {
            let record: Record = read_json(&receipt)?;
            verify_catalog(&record.catalog, &self.anchor)?;
            record.previous_sequence
        } else if self.state.join("pending.json").exists() {
            let pending: Intent = read_json(&self.state.join("pending.json"))?;
            if pending.idempotency_hash == key { pending.previous_sequence } else { self.catalog.sequence }
        } else { self.catalog.sequence };
        self.renew(expected, idempotency_key, now, signer)
    }

    /// Recover a restored/missing public pointer from independently retained
    /// signed history, then publish a new generation above that history.
    /// It never trusts backup entries over a newer durable withdrawal.
    pub fn recover(self, expected: u64, idempotency_key: &str, now: &str, signer: &impl CatalogSigner) -> Result<Catalog, String> {
        self.recover_with_hook(expected, idempotency_key, now, signer, |_| Ok(()))
    }
    fn recover_with_hook(mut self, expected: u64, idempotency_key: &str, now: &str, signer: &impl CatalogSigner,
        hook: impl Fn(&str) -> Result<(), String>) -> Result<Catalog, String> {
        date(now)?;
        validate_request_key(idempotency_key)?;
        let head: Record = read_json(&self.state.join("head.json"))?;
        verify_catalog(&head.catalog, &self.anchor)?;
        valid_date_at(&head.catalog.published, now)?;
        crate::publishers::CatalogPublishers::from_catalog(&head.catalog)?;
        if self.catalog.sequence > head.catalog.sequence {
            return Err("public catalog is newer than retained release state; recover the latest state before proceeding".into());
        }
        let receipt_path = self.state.join("requests").join(format!("{}.json", blake3::hash(idempotency_key.as_bytes()).to_hex()));
        let receipt: Option<Record> = if receipt_path.exists() { Some(read_json(&receipt_path)?) } else { None };
        if let Some(receipt) = &receipt {
            verify_catalog(&receipt.catalog, &self.anchor)?;
            valid_date_at(&receipt.catalog.published, now)?;
            if receipt.request_hash != request_hash(&self.anchor, expected, &Change::Recover)? {
                return Err("idempotency key was already used for another request".into());
            }
        }
        if expected != head.catalog.sequence {
            let receipt = receipt.as_ref().ok_or("recovery expected sequence does not match durable history")?;
            if receipt.previous_sequence != expected || hash(&receipt.catalog)? != hash(&head.catalog)? {
                return Err("historical recovery request does not identify the current head; use a new recovery request at its sequence".into());
            }
        }
        if self.state.join("pending.json").exists() {
            let pending: Intent = read_json(&self.state.join("pending.json"))?;
            if pending.catalog.sequence > head.catalog.sequence
                && (pending.idempotency_hash != blake3::hash(idempotency_key.as_bytes()).to_hex().as_str()
                    || pending.request_hash != request_hash(&self.anchor, expected, &Change::Recover)?) {
                return Err("finish the original prepared request before recovering another generation".into());
            }
        }
        if self.catalog.sequence == head.catalog.sequence && hash(&self.catalog)? != hash(&head.catalog)? {
            return Err("conflicting signed catalogs at one sequence require operator investigation".into());
        }
        // Prove every retained artifact is present before exposing the repaired
        // pointer. No native code or reviewer command runs in recovery.
        for entry in &head.catalog.entries {
            crate::admission::safe_relative(&entry.artifact)?;
            let artifact = self.catalog_path.parent().unwrap().join(&entry.artifact);
            let pack_path = self.catalog_path.parent().unwrap().join(format!("{}.pack.json", entry.artifact));
            verify_artifact(&artifact, entry)?;
            let pack: crate::Pack = read_json(&pack_path)?;
            verify_pack(&pack, entry, &self.state)?;
            hook("before-recovered-artifact-sync")?;
            crate::approval::sync_bundle(&artifact)?;
            sync_parent(&artifact)?;
            File::open(&pack_path).and_then(|file| file.sync_all()).map_err(|e| e.to_string())?;
            sync_parent(&pack_path)?;
        }
        if self.catalog.sequence < head.catalog.sequence {
            atomic_json(&self.catalog_path, &head.catalog)?;
            self.catalog = head.catalog;
        }
        hook("after-pointer-repair")?;
        self.transact(expected, idempotency_key, now, signer, Change::Recover, hook)
    }

    pub fn publish(self, expected: u64, idempotency_key: &str, now: &str, signer: &impl CatalogSigner,
        release: &crate::approval::ApprovedRelease) -> Result<Catalog, String> {
        self.transact(expected, idempotency_key, now, signer, Change::Publish(release), |_| Ok(()))
    }

    pub fn withdraw(self, expected: u64, idempotency_key: &str, now: &str, signer: &impl CatalogSigner,
        app: &str, version: &str, reason: &str) -> Result<Catalog, String> {
        self.transact(expected, idempotency_key, now, signer, Change::Withdraw { app, version, reason }, |_| Ok(()))
    }

    fn renew_with_hook(self, expected: u64, idempotency_key: &str, now: &str, signer: &impl CatalogSigner,
        hook: impl Fn(&str) -> Result<(), String>) -> Result<Catalog, String> {
        self.transact(expected, idempotency_key, now, signer, Change::Renew, hook)
    }

    fn transact(mut self, expected: u64, idempotency_key: &str, now: &str, signer: &impl CatalogSigner,
        change: Change<'_>, hook: impl Fn(&str) -> Result<(), String>) -> Result<Catalog, String> {
        date(now)?;
        if !self.catalog.published.is_empty() { valid_date_at(&self.catalog.published, now)?; }
        validate_request_key(idempotency_key)?;
        let request_hash = request_hash(&self.anchor, expected, &change)?;
        let idempotency_hash = blake3::hash(idempotency_key.as_bytes()).to_hex().to_string();
        let receipt_path = self.state.join("requests").join(format!("{idempotency_hash}.json"));
        let head_path = self.state.join("head.json");
        let intent_path = self.state.join("pending.json");
        let registry = self.ownership_registry()?;
        let current_hash = hash(&self.catalog)?;
        let head: Option<Record> = if head_path.exists() { Some(read_json(&head_path)?) } else { None };
        if let Some(head) = &head { verify_catalog(&head.catalog, &self.anchor)?; }
        let mut pending: Option<Intent> = if intent_path.exists() { Some(read_json(&intent_path)?) } else { None };
        if let Some(intent) = &pending {
            // A crash after the pointer swap only left housekeeping. Confirm
            // both the visible and durable generation before clearing it.
            if intent.catalog.sequence == self.catalog.sequence && intent.catalog.signing_bytes()? == self.catalog.signing_bytes()? {
                self.check_head(head.as_ref(), &current_hash)?;
                clear_pending(&intent_path)?;
                pending = None;
            }
        }
        if let Some(intent) = &pending {
            valid_date_at(&intent.catalog.published, now)?;
            if intent.idempotency_hash != idempotency_hash || intent.request_hash != request_hash
                || intent.previous_hash != current_hash || intent.previous_sequence != expected {
                return Err("a prepared generation reserves this sequence; retry its original request before another release".into());
            }
            let intended = change.candidate(&self.catalog, &intent.catalog.published, &registry)?;
            if intended.signing_bytes()? != intent.catalog.signing_bytes()? { return Err("prepared transaction does not match this release change".into()); }
        }
        if receipt_path.exists() {
            let record: Record = read_json(&receipt_path)?;
            verify_catalog(&record.catalog, &self.anchor)?;
            valid_date_at(&record.catalog.published, now)?;
            if record.request_hash != request_hash { return Err("idempotency key was already used for another request".into()); }
            if record.catalog.sequence <= self.catalog.sequence {
                self.check_head(head.as_ref(), &current_hash)?;
                if record.catalog.sequence == self.catalog.sequence && hash(&record.catalog)? != current_hash {
                    return Err("request receipt conflicts with this catalog generation".into());
                }
                return Ok(record.catalog);
            }
            let intent = pending.as_ref().ok_or("prepared receipt has no reserved generation; reconciliation required")?;
            if record.previous_sequence != self.catalog.sequence || record.previous_hash != current_hash
                || record.catalog.signing_bytes()? != intent.catalog.signing_bytes()?
                || head.as_ref().is_some_and(|h| h.catalog.sequence > record.catalog.sequence)
                || head.as_ref().is_some_and(|h| h.catalog.sequence == record.catalog.sequence && hash(&h.catalog).ok() != hash(&record.catalog).ok()) {
                return Err("release history requires reconciliation above its recorded sequence".into());
            }
            if head.as_ref().is_some_and(|h| h.catalog.sequence < record.catalog.sequence) {
                self.check_head(head.as_ref(), &current_hash)?;
            }
            if let Change::Publish(release) = &change {
                self.reserve_owner(&record.catalog, release.entry.app_id(), signer)?;
            }
            change.stage(&self, &hook)?;
            return self.install(&record, &head_path, &hook);
        }
        self.check_head(head.as_ref(), &current_hash)?;
        if expected != self.catalog.sequence { return Err(format!("stale expected sequence {expected}; current sequence is {}", self.catalog.sequence)); }
        // Check identity/continuity before exposing artifacts, and finish
        // staging before reserving a sequence. Refusals cannot publish bytes
        // or block unrelated renewals/withdrawals.
        let new_intent = pending.is_none();
        let next = if let Some(intent) = pending { intent.catalog } else { change.candidate(&self.catalog, now, &registry)? };
        encoded(&next, MAX_CATALOG_BYTES)?;
        change.stage(&self, &hook)?;
        if let Change::Publish(release) = &change {
            // Bind a new app ID before reserving this schema's sequence. A
            // sibling writer cannot invalidate an unsigned pending intent.
            self.reserve_owner(&next, release.entry.app_id(), signer)?;
        }
        if new_intent {
            let intent = Intent { idempotency_hash, request_hash: request_hash.clone(), previous_hash: current_hash.clone(),
                previous_sequence: self.catalog.sequence, catalog: next.clone() };
            hook("before-intent")?;
            immutable_json(&intent_path, &intent)?;
        }
        hook("before-signing")?;
        let signed = signer.sign(&ValidatedCatalog { catalog: next.clone() })?;
        verify_catalog(&signed, &self.anchor)?;
        if signed.signing_bytes()? != next.signing_bytes()? { return Err("signer changed the validated transaction".into()); }
        let record = Record { request_hash, previous_hash: current_hash, previous_sequence: self.catalog.sequence, catalog: signed };
        hook("before-receipt")?;
        immutable_json(&receipt_path, &record)?;
        self.install(&record, &head_path, &hook)
    }

    fn check_head(&self, head: Option<&Record>, current_hash: &str) -> Result<(), String> {
        if let Some(head) = head {
            if head.catalog.sequence != self.catalog.sequence || hash(&head.catalog)? != current_hash {
                return Err("catalog differs from durable release history; reconcile before publishing".into());
            }
        }
        Ok(())
    }

    fn install(&mut self, record: &Record, head_path: &Path, hook: &impl Fn(&str) -> Result<(), String>) -> Result<Catalog, String> {
        hook("before-head")?;
        atomic_json(head_path, record)?;
        hook("before-catalog")?;
        atomic_json(&self.catalog_path, &record.catalog)?;
        self.catalog = record.catalog.clone();
        hook("after-catalog")?;
        clear_pending(&self.state.join("pending.json"))?;
        Ok(self.catalog.clone())
    }
}

fn validate_request_key(key: &str) -> Result<(), String> {
    if key.is_empty() || key.len() > 256 { return Err("idempotency key must be 1 to 256 bytes".into()); }
    Ok(())
}

fn request_hash(anchor: &str, expected: u64, change: &Change<'_>) -> Result<String, String> {
    Ok(blake3::hash(&serde_json::to_vec(&serde_json::json!({"change":change.identity(),"anchor":anchor,"expected":expected})).map_err(|e| e.to_string())?).to_hex().to_string())
}

pub(crate) fn verify_artifact(path: &Path, entry: &crate::Entry) -> Result<(), String> {
    let (payload, manifest) = crate::runtime::identity(path)?;
    let expected_manifest = blake3::hash(&serde_json::to_vec(&entry.manifest).map_err(|e| e.to_string())?).to_hex().to_string();
    if payload != entry.manifest.integrity.bundle_blake3 || manifest != expected_manifest {
        return Err(format!("artifact differs from signed release {} {}", entry.app_id(), entry.version()));
    }
    Ok(())
}

pub(crate) fn verify_pack(pack: &crate::Pack, entry: &crate::Entry, scratch_parent: &Path) -> Result<(), String> {
    let mut nonce = [0u8; 16];
    rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut nonce).map_err(|e| e.to_string())?;
    let path = scratch_parent.join(format!("verify-{}", hex::encode(nonce)));
    struct Scratch(PathBuf);
    impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    let path = Scratch(path);
    crate::unpack(pack, &path.0)?;
    verify_artifact(&path.0, entry)
}

fn clear_pending(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|e| e.to_string())?;
    sync_parent(path)
}
pub(crate) fn valid_date_at(value: &str, now: &str) -> Result<(), String> {
    date(value)?;
    if value > now { return Err("prepared catalog date is in the future; check the operator clock".into()); }
    Ok(())
}

fn hash(catalog: &Catalog) -> Result<String, String> {
    Ok(blake3::hash(&serde_json::to_vec(catalog).map_err(|e| e.to_string())?).to_hex().to_string())
}

fn lock_file(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)] {
        use std::{os::unix::fs::OpenOptionsExt, os::fd::AsRawFd};
        options.custom_flags(libc::O_NOFOLLOW).mode(0o600);
        let lock = options.open(path).map_err(|e| e.to_string())?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(lock)
    }
    #[cfg(not(unix))] {
        let _ = path;
        Err("release transactions require a supported Unix operator runner".into())
    }
}

/// Strict Gregorian YYYY-MM-DD; device freshness remains wire-compatible.
pub(crate) fn date(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-'
        || bytes.iter().enumerate().any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit()) {
        return Err("date must be YYYY-MM-DD".into());
    }
    let year: u32 = value[..4].parse().map_err(|_| "invalid year")?;
    let month: usize = value[5..7].parse().map_err(|_| "invalid month")?;
    let day: u32 = value[8..].parse().map_err(|_| "invalid day")?;
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [31, if leap {29} else {28}, 31,30,31,30,31,31,30,31,30,31];
    if year == 0 || !(1..=12).contains(&month) || day == 0 || day > days[month - 1] { return Err("invalid calendar date".into()); }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let mut bytes = Vec::new();
    File::open(path).map_err(|e| e.to_string())?.take(MAX_RECORD_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_RECORD_BYTES { return Err("release record exceeds size limit".into()); }
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn encoded(value: &impl Serialize, limit: usize) -> Result<Vec<u8>, String> {
    struct Bounded { bytes: Vec<u8>, limit: usize }
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other("release record exceeds size limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
    }
    let mut writer = Bounded { bytes: Vec::new(), limit };
    serde_json::to_writer(&mut writer, value).map_err(|e| e.to_string())?;
    Ok(writer.bytes)
}

fn create_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = encoded(value, MAX_RECORD_BYTES)?;
    let mut file = OpenOptions::new().create_new(true).write(true).open(path).map_err(|e| e.to_string())?;
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    sync_parent(path)
}

fn immutable_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut nonce = [0u8; 16];
    rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut nonce).map_err(|e| e.to_string())?;
    let temporary = path.with_file_name(format!(".receipt-{}.tmp", hex::encode(nonce)));
    let result = (|| {
        create_json(&temporary, value)?;
        // Link is create-only and exposes fully synced bytes. A crash while
        // writing a temporary cannot leave a truncated idempotency receipt.
        fs::hard_link(&temporary, path).map_err(|e| e.to_string())?;
        sync_parent(path)
    })();
    let _ = fs::remove_file(temporary);
    result
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut nonce = [0u8; 16];
    rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut nonce).map_err(|e| e.to_string())?;
    let temp = path.with_file_name(format!(".release-{}.tmp", hex::encode(nonce)));
    struct Temporary(PathBuf);
    impl Drop for Temporary { fn drop(&mut self) { let _ = fs::remove_file(&self.0); } }
    let temp = Temporary(temp);
    create_json(&temp.0, value)?;
    fs::rename(&temp.0, path).map_err(|e| e.to_string())?;
    sync_parent(path)
}
pub(crate) fn sync_parent(path: &Path) -> Result<(), String> {
    File::open(path.parent().ok_or("missing parent")?).and_then(|f| f.sync_all()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HubKey, signing::LocalCatalogSigner};
    struct Fixture { root: PathBuf, public: PathBuf, anchor: HubKey, working: HubKey }
    impl Fixture {
        fn new() -> Self {
            let mut nonce = [0u8; 16];
            rand_core::TryRngCore::try_fill_bytes(&mut rand_core::OsRng, &mut nonce).unwrap();
            let root = std::env::temp_dir().join(format!("hub-release-{}", hex::encode(nonce)));
            fs::create_dir(&root).unwrap();
            let public = root.join("public");
            fs::create_dir(&public).unwrap();
            let f = Self { root, public, anchor: HubKey::generate(), working: HubKey::generate() };
            let mut catalog = Catalog::new(7, "2026-09-01", vec![]);
            f.working.sign_catalog(&mut catalog, &f.anchor.certify(&f.working.public_hex()).unwrap()).unwrap();
            create_json(&f.public.join("catalog.json"), &catalog).unwrap();
            f
        }
        fn open(&self) -> ReleaseStore {
            ReleaseStore::open(&self.public.join("catalog.json"), &self.root.join("private-state"), &self.anchor.public_hex()).unwrap()
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

    #[test]
    fn interruption_at_each_renewal_boundary_keeps_a_valid_generation_and_retry() {
        for point in ["before-intent", "before-signing", "before-receipt", "before-head", "before-catalog", "after-catalog"] {
            let f = Fixture::new();
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            let result = f.open().renew_with_hook(7, "operation", "2026-09-25", &signer,
                |stage| if stage == point { Err("injected interruption".into()) } else { Ok(()) });
            assert!(result.is_err());
            let visible: Catalog = read_json(&f.public.join("catalog.json")).unwrap();
            verify_catalog(&visible, &f.anchor.public_hex()).unwrap();
            assert!([7, 8].contains(&visible.sequence));
            let retried = f.open().renew(7, "operation", "2026-09-25", &signer).unwrap();
            assert_eq!(retried.sequence, 8);
            assert_eq!(f.open().catalog().sequence, 8);
        }
    }

    #[test]
    fn concurrent_renewals_cannot_reuse_a_generation() {
        let f = Fixture::new();
        let barrier = std::sync::Barrier::new(4);
        let successes = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..4).map(|i| {
                let f = &f;
                let barrier = &barrier;
                scope.spawn(move || {
                    let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
                    let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
                    barrier.wait();
                    f.open().renew(7, &format!("operation-{i}"), "2026-09-25", &signer).is_ok()
                })
            }).collect();
            handles.into_iter().map(|h| h.join().unwrap() as usize).sum::<usize>()
        });
        assert_eq!(successes, 1);
        assert_eq!(f.open().catalog().sequence, 8);
    }

    #[test]
    fn prepared_generation_blocks_a_different_request_and_future_clock_retry() {
        for different_request in [true, false] {
            let f = Fixture::new();
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            assert!(f.open().renew_with_hook(7, "prepared", "2026-09-25", &signer,
                |stage| if stage == "before-head" { Err("interrupted".into()) } else { Ok(()) }).is_err());
            let (key, now) = if different_request { ("different", "2026-09-26") } else { ("prepared", "2026-09-24") };
            let rotated = HubKey::generate();
            let certificate = f.anchor.certify(&rotated.public_hex()).unwrap();
            assert!(f.open().renew(7, key, now, &LocalCatalogSigner { key: &rotated, anchor_certificate: &certificate }).is_err(),
                "a pending generation must reserve its identity and date");
            assert_eq!(f.open().catalog().sequence, 7);
            assert_eq!(f.open().renew(7, "prepared", "2026-09-25", &signer).unwrap().sequence, 8);
        }
    }

    fn approve(f: &crate::test_bundle::Fixture) -> crate::approval::ApprovedRelease {
        let gate = f.report(None);
        let worker = f.protocol_worker();
        let evidence = crate::runtime::validate(&f.bundle, &gate, &worker).unwrap();
        crate::approval::ApprovedRelease::new(evidence, &gate, "publisher-one", &f.publisher.public_hex(), "", "",
            crate::approval::OperatorReview { reviewer: "test-operator".into(), review_id: "review-one".into() }).unwrap()
    }

    #[test]
    fn crash_at_each_publish_boundary_keeps_a_complete_catalog() {
        for point in ["before-intent", "before-artifact", "after-artifact-rename", "before-pack", "after-pack-link", "before-validation-record", "after-validation-record-link", "before-signing", "before-receipt", "before-head", "before-catalog", "after-catalog"] {
            let f = Fixture::new();
            let bundle = crate::test_bundle::Fixture::new();
            let approved = approve(&bundle);
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            assert!(f.open().transact(7, "publish", "2026-09-25", &signer, Change::Publish(&approved),
                |stage| if stage == point { Err("interrupted".into()) } else { Ok(()) }).is_err());
            let visible = f.open().catalog().clone();
            verify_catalog(&visible, &f.anchor.public_hex()).unwrap();
            assert!([7, 8].contains(&visible.sequence));
            for entry in &visible.entries {
                let artifact = f.public.join(&entry.artifact);
                assert_eq!(crate::runtime::identity(&artifact).unwrap().0, entry.manifest.integrity.bundle_blake3);
                assert!(f.public.join(format!("{}.pack.json", entry.artifact)).is_file());
                assert!(f.public.join(format!("{}.validation.json", entry.artifact)).is_file());
            }
            assert_eq!(f.open().publish(7, "publish", "2026-09-25", &signer, &approved).unwrap().sequence, 8);
        }
    }

    #[test]
    fn concurrent_publishes_require_rebase_and_keep_both_releases() {
        let f = Fixture::new();
        let first = crate::test_bundle::Fixture::new();
        let mut second = crate::test_bundle::Fixture::new();
        second.manifest.id = "second-app".into();
        second.publisher = HubKey::from_bytes(&first.publisher.to_bytes());
        second.sign();
        let approvals = [approve(&first), approve(&second)];
        let barrier = std::sync::Barrier::new(2);
        let results = std::thread::scope(|scope| {
            let handles: Vec<_> = approvals.iter().enumerate().map(|(i, approval)| {
                let f = &f;
                let barrier = &barrier;
                scope.spawn(move || {
                    let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
                    let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
                    barrier.wait();
                    f.open().publish(7, &format!("publish-{i}"), "2026-09-25", &signer, approval).is_ok()
                })
            }).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>()
        });
        assert_eq!(results.iter().filter(|s| **s).count(), 1);
        let i = results.iter().position(|s| !s).unwrap();
        let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
        let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
        let catalog = f.open().publish(8, &format!("publish-{i}"), "2026-09-25", &signer, &approvals[i]).unwrap();
        assert_eq!(catalog.sequence, 9);
        assert_eq!(catalog.entries.len(), 2);
        let withdrawn = f.open().withdraw(9, "withdraw", "2026-09-25", &signer, "example-app", "1.0.0", "test retirement").unwrap();
        assert_eq!(withdrawn.sequence, 10);
        assert_eq!(withdrawn.entries.len(), 2);
        assert!(!withdrawn.entries.iter().find(|e| e.app_id() == "example-app").unwrap().status.is_offered());
        for entry in &withdrawn.entries { assert!(f.public.join(&entry.artifact).is_dir()); }
    }

    #[test]
    fn interrupted_v2_publish_reserves_publisher_before_v1_can_claim_the_id() {
        for interruption in ["before-signing", "before-receipt"] {
            let f = Fixture::new();
            let mut modern = crate::test_bundle::Fixture::new();
            let mut value = serde_json::to_value(&modern.manifest).unwrap();
            value["schema"] = serde_json::json!(2);
            value["release_number"] = serde_json::json!(1);
            value["runtime"] = serde_json::json!({"api":"1","min_build":1,"platforms":[std::env::consts::OS]});
            value["requires"] = serde_json::json!(["card.ui@1"]);
            value["entrypoints"] = serde_json::json!({"ui":"page.card"});
            value["data_schema"] = serde_json::json!(1);
            modern.manifest = octosense_app_policy::AppManifest::parse(&value.to_string()).unwrap();
            modern.sign();
            let approved = approve(&modern);
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            fs::create_dir(f.public.join("v2")).unwrap();
            let modern_path = f.public.join("v2/catalog.json");
            let result = ReleaseStore::open_format(&modern_path, &f.root.join("v2-state"), &f.anchor.public_hex(), CatalogFormat::V2)
                .unwrap().transact(0, "modern", "2026-09-25", &signer, Change::Publish(&approved),
                    |stage| if stage == interruption { Err("interrupted".into()) } else { Ok(()) });
            assert!(result.is_err());
            assert!(!modern_path.exists());
            let old_bundle = crate::test_bundle::Fixture::new();
            let competing = approve(&old_bundle);
            assert!(f.open().publish(7, "competing", "2026-09-25", &signer, &competing).is_err());
            assert_eq!(f.open().catalog().sequence, 7);
            let retried = ReleaseStore::open_format(&modern_path, &f.root.join("v2-state"), &f.anchor.public_hex(), CatalogFormat::V2)
                .unwrap().publish(0, "modern", "2026-09-25", &signer, &approved).unwrap();
            assert_eq!(retried.sequence, 1);
        }
    }

    #[test]
    fn preexisting_artifacts_cannot_be_overwritten_or_signed() {
        let f = Fixture::new();
        let bundle = crate::test_bundle::Fixture::new();
        let approved = approve(&bundle);
        let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
        let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
        // A preexisting destination is immutable, even when it is only a
        // partially written directory from a writer outside the transaction.
        let artifact = f.public.join(&approved.entry.artifact);
        fs::create_dir_all(&artifact).unwrap();
        fs::write(artifact.join("owned.txt"), "must stay untouched").unwrap();
        assert!(f.open().publish(7, "bad-artifact", "2026-09-25", &signer, &approved).is_err());
        assert_eq!(fs::read_to_string(artifact.join("owned.txt")).unwrap(), "must stay untouched");
        assert_eq!(f.open().catalog().sequence, 7);
        assert_eq!(f.open().renew(7, "unrelated-renewal", "2026-09-25", &signer).unwrap().sequence, 8);
    }

    #[test]
    fn retry_cannot_skip_a_failed_existing_artifact_sync() {
        let f = Fixture::new();
        let bundle = crate::test_bundle::Fixture::new();
        let approved = approve(&bundle);
        let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
        let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
        for failure in ["after-validation-record-link", "before-existing-validation-record-sync"] {
            assert!(f.open().transact(7, "retry-sync", "2026-09-25", &signer, Change::Publish(&approved),
                |stage| if stage == failure { Err("sync interrupted".into()) } else { Ok(()) }).is_err());
            assert_eq!(f.open().catalog().sequence, 7);
        }
        assert_eq!(f.open().publish(7, "retry-sync", "2026-09-25", &signer, &approved).unwrap().sequence, 8);
    }

    #[test]
    fn state_inside_the_catalog_directory_is_refused() {
        let f = Fixture::new();
        assert!(ReleaseStore::open(&f.public.join("catalog.json"), &f.public.join("state"), &f.anchor.public_hex()).is_err());
    }

    #[test]
    fn schema_writers_cannot_split_publisher_history_across_private_parents() {
        let f = Fixture::new();
        drop(f.open());
        fs::create_dir(f.public.join("v2")).unwrap();
        let other = f.root.join("other-private");
        let attempt = ReleaseStore::open_format(&f.public.join("v2/catalog.json"), &other.join("state"),
            &f.anchor.public_hex(), CatalogFormat::V2);
        assert!(attempt.is_err());
    }

    #[test]
    fn duplicate_versions_and_conflicting_owners_never_expose_artifacts() {
        for change_owner in [false, true] {
            let f = Fixture::new();
            let mut bundle = crate::test_bundle::Fixture::new();
            let first = approve(&bundle);
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            f.open().publish(7, "first", "2026-09-25", &signer, &first).unwrap();
            if change_owner { bundle.manifest.version = "2.0.0".into(); bundle.publisher = HubKey::generate(); }
            else { fs::write(bundle.bundle.join("additional.txt"), "same version, different bytes").unwrap(); }
            bundle.sign();
            let refused = approve(&bundle);
            assert!(f.open().publish(8, "refused", "2026-09-25", &signer, &refused).is_err());
            assert!(!f.public.join(&refused.entry.artifact).exists(), "rejected admission must precede public artifact exposure");
            assert_eq!(f.open().catalog().sequence, 8);
        }
    }

    #[test]
    fn recovery_keeps_withdrawals_and_requires_complete_artifacts() {
        for damage_pack in [false, true] {
            let f = Fixture::new();
            let bundle = crate::test_bundle::Fixture::new();
            let approved = approve(&bundle);
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            let old = f.open().publish(7, "publish", "2026-09-25", &signer, &approved).unwrap();
            f.open().withdraw(8, "withdraw", "2026-09-25", &signer, "example-app", "1.0.0", "retired").unwrap();
            atomic_json(&f.public.join("catalog.json"), &old).unwrap();
            if damage_pack { fs::write(f.public.join(format!("{}.pack.json", old.entries[0].artifact)), "{}").unwrap(); }
            let result = f.open().recover(9, "restore", "2026-09-25", &signer);
            if damage_pack {
                assert!(result.is_err());
                assert_eq!(f.open().catalog().sequence, 8);
            } else {
                let result = result.unwrap();
                assert_eq!(result.sequence, 10);
                assert!(!result.entries[0].status.is_offered(), "an old backup cannot undo a recorded withdrawal");
            }
        }
    }

    #[test]
    fn recovery_and_scheduled_renewal_retries_keep_the_original_generation() {
        for point in ["after-pointer-repair", "before-intent", "before-signing", "before-receipt", "before-head", "before-catalog", "after-catalog"] {
            let f = Fixture::new();
            let backup = f.open().catalog().clone();
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            assert_eq!(f.open().renew_current("daily", "2026-09-25", &signer).unwrap().sequence, 8);
            assert_eq!(f.open().renew_current("daily", "2026-09-25", &signer).unwrap().sequence, 8);
            atomic_json(&f.public.join("catalog.json"), &backup).unwrap();
            assert!(f.open().recover_with_hook(8, "restore", "2026-09-25", &signer,
                |stage| if stage == point { Err("interruption".into()) } else { Ok(()) }).is_err());
            let result = f.open().recover(8, "restore", "2026-09-25", &signer).unwrap();
            assert_eq!(result.sequence, 9);
        }
    }

    #[test]
    fn recovery_preconditions_do_not_change_the_public_pointer() {
        for request in ["", "daily", "old-recovery"] {
            let f = Fixture::new();
            let backup = f.open().catalog().clone();
            let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
            let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
            f.open().renew_current("daily", "2026-09-25", &signer).unwrap();
            if request == "old-recovery" {
                f.open().recover(8, request, "2026-09-25", &signer).unwrap();
                f.open().renew_current("later", "2026-09-25", &signer).unwrap();
            }
            atomic_json(&f.public.join("catalog.json"), &backup).unwrap();
            assert!(f.open().recover(8, request, "2026-09-25", &signer).is_err());
            assert_eq!(f.open().catalog().sequence, 7, "refused recovery must not repair a pointer as a side effect");
        }
    }

    #[test]
    fn failed_restore_sync_cannot_publish_the_repaired_pointer() {
        let f = Fixture::new();
        let bundle = crate::test_bundle::Fixture::new();
        let approved = approve(&bundle);
        let certificate = f.anchor.certify(&f.working.public_hex()).unwrap();
        let signer = LocalCatalogSigner { key: &f.working, anchor_certificate: &certificate };
        let backup = f.open().publish(7, "publish", "2026-09-25", &signer, &approved).unwrap();
        f.open().renew_current("daily", "2026-09-25", &signer).unwrap();
        atomic_json(&f.public.join("catalog.json"), &backup).unwrap();
        assert!(f.open().recover_with_hook(9, "restore", "2026-09-25", &signer,
            |stage| if stage == "before-recovered-artifact-sync" { Err("sync failure".into()) } else { Ok(()) }).is_err());
        assert_eq!(f.open().catalog().sequence, 8);
        assert_eq!(f.open().recover(9, "restore", "2026-09-25", &signer).unwrap().sequence, 10);
    }

    #[test]
    fn oversized_serialized_records_are_refused_before_creating_a_file() {
        let f = Fixture::new();
        let path = f.root.join("oversized.json");
        assert!(create_json(&path, &"x".repeat(MAX_RECORD_BYTES)).is_err());
        assert!(!path.exists(), "a refused serialization must not leave a partial record");
    }

    #[test]
    fn signer_cannot_replace_the_validated_catalog() {
        let f = Fixture::new();
        struct WrongSigner<'a>(&'a Fixture);
        impl CatalogSigner for WrongSigner<'_> {
            fn sign(&self, transaction: &ValidatedCatalog) -> Result<Catalog, String> {
                let mut result = transaction.catalog().clone();
                result.sequence += 99;
                self.0.working.sign_catalog(&mut result, &self.0.anchor.certify(&self.0.working.public_hex())?)?;
                Ok(result)
            }
        }
        assert!(f.open().renew(7, "bad-signer", "2026-09-25", &WrongSigner(&f)).unwrap_err().contains("signer changed"));
        assert_eq!(f.open().catalog().sequence, 7);
    }
}
