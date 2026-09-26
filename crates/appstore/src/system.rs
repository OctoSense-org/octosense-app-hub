//! System apps: script apps that ship inside the build.
//!
//! News, Photos and Maps are part of the phone, not store downloads, but
//! they need not be native code. A system app is an ordinary bundle
//! (`manifest.json` and a `main.splash` program, with its artwork) packed
//! into one file and compiled into the shell. It runs exactly as an installed
//! app does: in its own isolate, under the policy its manifest resolves to,
//! with a jail and a host list. Only admission differs. The pack arrived
//! with the signed build, so no catalog vouches for it and no signature is
//! asked for; its bytes must still hash to what its manifest claims, and its
//! ceilings are [`HostLimits::system`], not the build's own reach.
//!
//! Ids under `os.` are reserved for system apps, so a store download can
//! never take over a system app's jail.
use octosense_app_policy::{AppPolicy, HostLimits, RefuseAllSignatures};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The id prefix only system apps may use.
pub const SYSTEM_ID_PREFIX: &str = "os.";

/// A system app as the shell registers it: its id, the bundle packed as one
/// JSON file (`octosense_app_hub::pack::pack_system_app` in the shell's
/// build script, then `include_str!`), and artwork too large to pack,
/// compiled in and served from memory at bundle paths (`photos/01.png`).
#[derive(Clone, Copy, Debug)]
pub struct SystemApp {
    pub id: &'static str,
    pub name: &'static str,
    pub pack: &'static str,
    pub assets: octosense_app_policy::StaticAssets,
}

static SYSTEM_APPS: Mutex<Vec<SystemApp>> = Mutex::new(Vec::new());

/// Make a system app openable through the `card` module. Registering the
/// same id again replaces it.
pub fn register_system_app(app: SystemApp) {
    assert!(app.id.starts_with(SYSTEM_ID_PREFIX), "a system app's id starts with {SYSTEM_ID_PREFIX}");
    let mut apps = SYSTEM_APPS.lock().unwrap();
    apps.retain(|a| a.id != app.id);
    apps.push(app);
}

pub fn system_app(id: &str) -> Option<SystemApp> {
    SYSTEM_APPS.lock().unwrap().iter().find(|a| a.id == id).copied()
}

pub fn system_apps() -> Vec<SystemApp> {
    SYSTEM_APPS.lock().unwrap().clone()
}

/// Unpack a system app's bundle (once per build of it) outside every jail,
/// and admit it: the unpacked bytes must hash to the manifest's digest, the
/// manifest must be the app it was registered as, and the policy resolves
/// under system ceilings. Returns the bundle directory and the policy.
pub fn prepare(app_data_root: &Path, app: &SystemApp) -> Result<(PathBuf, AppPolicy), String> {
    // The pack's own hash names the directory, so a new build of the app
    // unpacks beside the old one instead of over a bundle something reads.
    let pack_hash = octosense_app_policy::bundle_digest(app.pack.as_bytes());
    // `.system` can never be an app id, so it is no app's jail.
    let dir = app_data_root.join(".system").join(app.id).join(&pack_hash[..16]);
    if !dir.join(octosense_app_policy::MANIFEST_FILE).is_file() {
        let pack: octosense_app_hub::pack::Pack =
            serde_json::from_str(app.pack).map_err(|e| format!("{}: not a pack: {e}", app.id))?;
        let staging = dir.with_extension("staging");
        let _ = std::fs::remove_dir_all(&staging);
        octosense_app_hub::pack::unpack(&pack, &staging)?;
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(&staging, &dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let manifest = std::fs::read_to_string(dir.join(octosense_app_policy::MANIFEST_FILE))
        .map_err(|e| format!("{}: no manifest: {e}", app.id))?;
    let digest = octosense_app_policy::digest_dir(&dir)?;
    let policy = octosense_app_policy::admit_and_resolve_dir(&manifest, &digest, &HostLimits::system(), &RefuseAllSignatures)?;
    if policy.app_id != app.id {
        return Err(format!("the pack registered as {} holds {}", app.id, policy.app_id));
    }
    Ok((dir, policy))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_of(name: &str, files: &[(&str, &str)]) -> String {
        let dir = std::env::temp_dir().join(format!("appstore-system-src-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        let digest = octosense_app_policy::digest_dir(&dir).unwrap();
        let manifest = format!(
            r#"{{"schema":1,"id":"os.demo","version":"1","name":"Demo","integrity":{{"bundle_blake3":"{digest}"}},
                "capabilities":["storage","net","images"],"network":{{"hosts":["example.org"]}}}}"#
        );
        std::fs::write(dir.join("manifest.json"), manifest).unwrap();
        serde_json::to_string(&octosense_app_hub::pack::pack_dir(&dir).unwrap()).unwrap()
    }

    #[test]
    fn a_system_app_unpacks_outside_every_jail_and_runs_under_its_manifest() {
        let pack: &'static str = Box::leak(pack_of("runs", &[("main.splash", "Label{text: \"hi\"}")]).into_boxed_str());
        let root = std::env::temp_dir().join(format!("appstore-system-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let app = SystemApp { id: "os.demo", name: "Demo", pack, assets: &[] };
        let (dir, policy) = prepare(&root, &app).unwrap();
        assert!(dir.starts_with(root.join(".system")));
        assert!(!dir.starts_with(policy.jail_root(&root)), "the app cannot rewrite its own program");
        assert!(policy.allows("images") && policy.allows_host("example.org"));
        assert!(policy.instruction_budget > HostLimits::default().max_instruction_budget);
        // A second open reuses the unpacked bundle, and still checks it.
        std::fs::write(dir.join("main.splash"), "tampered").unwrap();
        assert!(prepare(&root, &app).is_err(), "a changed byte is refused");
    }

    #[test]
    fn a_pack_must_hold_the_app_it_was_registered_as() {
        let pack: &'static str = Box::leak(pack_of("mismatch", &[("main.splash", "x")]).into_boxed_str());
        let root = std::env::temp_dir().join(format!("appstore-system-root-mismatch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let err = prepare(&root, &SystemApp { id: "os.other", name: "Other", pack, assets: &[] }).unwrap_err();
        assert!(err.contains("holds os.demo"), "{err}");
    }
}
