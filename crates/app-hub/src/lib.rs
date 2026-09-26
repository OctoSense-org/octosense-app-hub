//! The OctoSense app hub (ADR 0003).
//!
//! Three sides of one boundary:
//!
//! - [`gate`] decides whether a bundle is admitted. It runs identically on a
//!   developer's machine before submitting and in the hub's job after, so the
//!   report a publisher sees is the report the hub acts on.
//! - [`index`] is what gets published: an entry per app version, and the
//!   signed catalog a device reads.
//! - [`client`] is the device's side: verify, list, search, install, remove,
//!   and refuse to run what has been withdrawn.
//!
//! What an app may DO once installed is not here — that is ADR 0002 and the
//! `octosense-app-policy` crate. The hub decides what is offered; the policy
//! and the runtime decide what is allowed.
pub mod client;
pub mod admission;
pub mod gate;
pub mod index;
mod launch;
pub mod pack;
pub mod publishers;
pub mod remote;
pub mod scan;
pub mod signing;
pub mod runtime;
pub mod release;
pub mod approval;
pub mod operations;
mod process;

pub use client::{days_between, AppAvailability, Availability, Listing, PreparedLaunch, Store, CATALOG_FRESHNESS_DAYS};
pub use pack::{pack_dir, unpack, Pack};
pub use remote::{today, Remote};
pub use scan::{packet, scan, Packet, Route, Verdict};
pub use gate::{check_bundle, entry_for, Finding, GateReport, Severity};
pub use index::{CatalogFormat, Catalog, Entry, Source, Status, WorkingKey, CATALOG_SCHEMA};
pub use signing::{sign_manifest, verify_catalog, HubKey, PublisherKeys};

#[cfg(test)]
extern crate self as octosense_app_hub;
#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod test_bundle;
