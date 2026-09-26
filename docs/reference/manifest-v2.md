# Manifest and catalog v2

V2 is an explicit release contract for apps and clients. The public default
remains v1: `catalog.json` and the local `catalog.json` cache. A client opts in
with `OCTOSENSE_HUB_SCHEMA=2`, reads `v2/catalog.json`, and keeps its accepted
sequence in `catalog-v2.json`. Catalog bytes cannot switch a v1 reader into
v2. Keep both endpoints and their private release-state directories while
clients migrate.

## Manifest contract

V2 retains the v1 identity, integrity, capability, network, storage, compute
and agent fields and adds these required fields:

```json
{
  "schema": 2,
  "id": "org.example.notes",
  "version": "1.2.0",
  "name": "Notes",
  "integrity": {"bundle_blake3": "<64 lowercase hex digits>"},
  "release_number": 12,
  "runtime": {"api": "1", "min_build": 1, "platforms": ["android", "macos"]},
  "requires": ["card.ui@1"],
  "entrypoints": {"ui": "page.card"},
  "data_schema": 1
}
```

`version` is a SemVer display label; `release_number` is a positive,
monotonically increasing ordering value for an app. Catalog array position and
the SemVer label do not override that number. A client will not automatically
downgrade an installed release. The release operator rejects a number that is
not greater than every published v2 number for that app. The signed entrypoint must match the actual
bundle: `page.card` for a Card app or `main.splash` for a script app. Separate
logic entrypoints are reserved until an implementation advertises
`app.logic@1`. Paths are portable bundle-relative names.

The current baseline advertises runtime API `1`, build `1`, `card.ui@1`,
`script.ui@1`, and the core `storage`, `net` and `prompt` capabilities on the
host OS. A release requiring a newer build, another platform, an unadvertised
feature/capability or an unsupported entrypoint is visible with a concrete
reason and has no install consent. A host may advertise additional features
only when their service adapter is present. V1 apps remain governed by their
original signed manifest and the existing policy resolver.

## Signing and publication

V1 signed fields and their canonical bytes are frozen in
`crates/app-hub/tests/fixtures/wire/`. V2 has separate golden signing fixtures.
Unknown schemas, duplicate JSON fields, and v2 fields smuggled into v1 are
refused. V2 catalogs may retain approved v1 release records so already
installed versions remain launchable, but any retained record's artifact must
also be available under the v2 origin.

An operator stages and reviews a signed bundle as usual, then uses
`hub admin publish ... --catalog <public>/v2/catalog.json --catalog-schema 2
--state-dir <private-v2-state>`. Renewals and withdrawals use the same
`--catalog-schema 2` and private state. Omission selects v1. The v1 and v2
state directories, sequence floors, catalog files and artifact roots must
remain separate, but both private state directories must have the same parent.
For example, use `<private>/v1-state` and `<private>/v2-state`. Both writers
take one lock at `<public>/.release-domain.lock`, and signed publisher
ownership reservations live in that shared private parent. Keep the v1 and
v2 state directories and the `.app-hub-owners-*` directory in the same
backup set. The operator records their shared location at
`<public>/.release-ownership.json` and refuses a different private parent.
Keep the intact signed v1 catalog available when the first v2 transaction
opens; the operator copies its publisher history into one signed ownership
proof. If the v1 pointer is missing or older than retained private history,
recover it first. New app claims use compact one-entry signed proofs, made
durable before either schema writes a pending release intent.
Both catalog endpoints use the standard names shown above. `hub admin status`
can inspect either signed file. The
operator should populate and probe the v2 endpoint, including approved v1
releases needed by installed clients, before enabling the v2 reader in a
client build. A rollout never restores an older signed sequence.

## Validation

Run `cargo test --locked -p octosense-app-hub --test compatibility` and
`cargo test --locked -p octosense-app-hub --test release_transactions` from
the Hub root. Native execution still requires the trusted `app-validator`
worker and a platform build; schema parsing alone is not runtime approval.
