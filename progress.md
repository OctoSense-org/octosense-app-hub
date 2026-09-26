# Review progress

- Bootstrapped requested Superpowers instructions and loaded planning-with-files.
- Located Hub code, documentation and mobile integration; inspected repository status and recent history.
- Started outside-developer journey review. No application changes made.
- Read CLI publish/gate/policy/client and mobile installation/catalog paths; identified update policy and publisher continuity concerns for reproduction.
- Started locked offline policy and hub tests.
- Consulted official store workflow documentation for comparison.
- Completed 78 existing tests successfully; isolated probe reproduced three release-blocking gaps without changing application source.
- Verified live repository/catalog and publishing workflow availability through read-only requests.
- Completed source review of current mobile UX, runtime host wiring and reference-host limitations; preparing final report.
- Wrote docs/reviews/2026-09-24-store-readiness.md with capability coverage, six launch blockers, concrete evidence, release phases and acceptance criteria.
- Verified all 29 local evidence links resolve. Hub tracked source has no diff; only review documents/planning notes are untracked. Probe remains ignored under target/. Mobile's pre-existing planning changes are unchanged.

## Implementation planning

- User requested a prioritized plan addressing every review issue plus implementation plans for individual features.
- Loaded brainstorming and writing-plans skills; retained planning-with-files workflow.
- Asked optional first-release scope preference while continuing with dependency and source mapping; recommended free Card apps first.
- Planning only: no application implementation, publication or deployment authorized by this follow-up.
- Confirmed the user's free-Card-first scope and wrote all 26 feature plans plus the shared architecture/execution guide.
- Plans explicitly cover signing custody, legacy signature preservation, immutable code/data separation, private artifacts and release recovery.
- Building the master priority table, dependency graph and review-to-plan coverage matrix, then checking file/dependency consistency.
- Finalized planning on 2026-09-25, retaining 2026-09-24 filenames for the review/planning session.
- Created 28 documents: ranked roadmap, shared design/execution guide and 26 feature implementation plans. They contain 122 task slices and 78 acceptance criteria covering all 29 review items.
- Final consistency verifier passed: all review coverage/cross-references, dependency graph, planned/existing file ownership, document links, task counts and code fences; zero errors.
- Confirmed no tracked application-source diff in Hub; preserved the mobile repository's existing planning-file changes. No feature implementation, commits, issue tracker writes or deployment performed.

## 2026-09-25 — Implementation begins

- User authorized implementing each item; free Card apps remain first-release scope.
- Created isolated `feat/app-store-foundations` branch/worktree beside existing repositories to preserve relative Cargo patches.
- Copied review and implementation plans into the worktree; existing shared checkouts preserved.
- Starting plan 01 with regression tests for publisher identity and key continuity.

## Plan 01 verified — 2026-09-25

- Twelve publisher/CLI regressions pass, with seven intended failures observed before the fix. Existing 42 Hub/policy tests remain green.
- Authenticated catalog history now anchors actual publisher keys; unsigned public releases, ambiguous legacy mappings, mismatched owners and reused reports after edits are refused.
- Independent code review found no important issues; corrected a PNG fixture CRC.
- Plan 02 regression suite reproduces all seven launch/manifest defects after correcting a test field name (`version`). No plan 02 production changes yet.

## Plan 02 implementation — 2026-09-25

- Exact installed-release lookup now verifies the complete admitted manifest, canonical bytes and catalog-bound publisher signature. Open and Update are independent in shared and mobile store UI.
- Launch workers create bounded owned snapshots, then recheck current catalog metadata before startup. Snapshots survive concurrent updates and disappear on normal shutdown. Active mobile instances close only for their exact revoked release; pending approvals are invalidated by a catalog refresh.
- Reviewer found a verify/start race and delayed Remove/Update race; both were fixed with owned code snapshots and pending-launch cancellation. Independent follow-up review found no remaining important issues.
- Verification: 68 Hub/policy tests; 41 mobile App Hub tests; one real native shared-store action regression, all passed. Full mobile shell checked successfully against its pinned framework/runtime. New cancellation test failed when its fix was removed, then passed when restored.
- Build preparation initially used the older shared Makepad checkout and hit an existing windows API mismatch. Prepared separate runtime checkouts with tools/setup-native.py; no shared runtime source was changed. Validated Makepad 1d3d383, Octoscript-Makepad c4c9682, Octoscript 68f6a9d.
- Native widget evaluation and shell compilation are host evidence; no Android/device rollout claimed. Coordinated Hub revision-pin verification is the remaining integration checkpoint.

- Coordinated integration passed: mobile pins published Hub commit `0269a85b0b2166de3866464716f06dce3e69ab1a`; full shell check and 41 mobile tests passed again using fetched Git dependencies, without a local Hub patch. Framework overrides point to exact pinned revisions in isolated checkouts. Hub feature branch pushed; no main merge or deployment.
- Beginning plan 03: bounded structural admission, shared runtime preparation and isolated validation evidence.

## Plan 03 structural slice — 2026-09-25

- Added bounded inventory, UTF-8/JSON/Card syntax and kit closure checks; full PNG/JPEG/WebP decode, SVG resource checks, square icons, structured diagnostics and typed resource inventories. Public `hub check --json` and library reports match.
- Packing and unpacking now enforce size/entry/depth limits before writes and refuse nonempty/symlink staging. Tests reproduced both oversized unpacking and staging-symlink escapes before fixes.
- Twenty admission tests pass, including seven initial structural failures and reviewer regressions. Retained conservative text URL denial pending plan 07.
- Review found Card/kit expansion was not bounded by input size. Removed realization from the headless Hub process; full preparation must occur in the upcoming constrained runtime worker. Cached token weights, checked every SVG CSS URL, and avoided interpreting kit property declarations as resource paths or theme overlays as native-only Cards.
- Added dependencies from cached pinned releases: existing image 0.25.10 with PNG/JPEG/WebP codecs, roxmltree 0.21.1 and the already-used Octoscript L0 revision. Policy core remains free of graphical dependencies. Image maximum dimensions are strict; decoded output bytes are checked explicitly before allocation because decoder max_alloc is advisory (https://docs.rs/image/0.25.10/image/struct.Limits.html).
- Plan 03 remains in progress: shared native preparation, isolated runtime evidence, publication binding and native end-to-end verification are still required.

## Plan 03 native admission slice — 2026-09-25

- Added the native worker, shared Card preparation, owned validation snapshots, full manifest/payload/runtime binary binding and mandatory publish checks. Reference and installed hosts use the same preparation function.
- Native smoke uses actual Splash + the existing policy adapter, temporary storage, startup and shutdown. Review found fixed evaluator budgets diverged from installed policies; regressions for tiny memory/instruction budgets now pass using the actual host.
- Worker timeout/crash/output/environment checks pass; reviewer subprocesses retain their configured cwd/environment after a compatibility regression was fixed. Worker Rust allocations are capped at 256 MiB before Card expansion, with a real expansion regression.
- JSON refusal regressions failed before fixes; hub test now preserves structural findings and returns a stable runtime failure envelope, also emitted by the worker.
- Current full suite: 101 tests pass (Hub/policy/native validator, including doctest). A native CLI signed-fixture/publication/catalog check and consumer integration are being verified before checkpoint.
- Evidence scope is macOS native widget construction and lifecycle. The static fixture has no declared interactions; GPU rendering, Android and application-state conformance remain separate (plans 06/26). Initial compile needed closures to discard script_mod return values; corrected with no framework edits.
- Native end-to-end CLI journey passed: ephemeral publisher/anchor/working keys, signed real Card fixture, native hub test, publication, catalog signature verification and identical runtime sidecar. No production publication. Independent follow-up review found no important issues.
- Mobile local-consumer integration passed: 41 App Hub tests and the native shared-store action regression, using isolated exact mobile runtime revisions.
- Exact mobile runtime verification also passed all seven native-validator cases from the Hub workspace, plus the full mobile shell check. Cargo cannot test dev-dependencies of a non-member dependency; reran from the owning Hub workspace with runtime overrides, then restored its normal lockfile. No framework source changes.

## Plan 04 begins — 2026-09-25

- Starting monotonic renewal and serialized, durable release transactions. Current CLI rewrites catalogs in place, writes artifacts before optional review, and independently reads/signs catalog generations; these paths must share one transaction boundary.
- Plan 03 Hub commit ab76fcc is pushed on the feature branch. Consumer pin verification is in progress; an offline lookup correctly required fetching the new revision first.
- Consumer integration finished: mobile pins fetched Hub ab76fcc (no local Hub override), full shell check and 41 mobile tests passed. Existing shell cfg warnings remain unrelated. Mobile commit records the new shared validator dependency.
- Three renewal regressions failed on the old CLI, then passed: unchanged entries with advanced date/sequence, strict date/signature refusal, idempotent retry/CAS conflict, durable restore refusal and certified working-key rotation.
- Renewal interruption/concurrency review identified a prepared-receipt gap; added a durable pre-signing intent so another request cannot reuse the reserved sequence. Recovery revalidates its original date after clock correction. Added bounded compact serialization and a real oversized-record regression (observed red with the prior writer, then restored the fix).
- All five renewal transaction unit tests and three CLI tests pass; independent follow-up review has no important findings. This slice is renewal only: legacy publish/withdraw/remove must still migrate to the same transaction boundary before production use.

## Plan 04 publication slice in progress

- Migrated operator publication/withdrawal to the renewal transaction boundary and mandatory trusted-anchor/CAS/idempotency inputs. Public release requires reviewer identity and a review record; caller-supplied reviewer commands/boolean bypasses are refused on publication and remain separate scan operations.
- ApprovedRelease is constructed only from owned runtime evidence plus a matching gate report. Artifacts use a digest of the approved bytes/metadata and are fully staged privately, synchronized and exposed before the catalog swap. Existing destinations are immutable and verified on retry.
- Destructive catalog removal is refused; exact-version withdrawal retains publisher identity/history and existing artifacts. Tests are checking every artifact/catalog fault boundary and concurrent publication/rebase. No production releases have been made.
- Publication review prompted ordering and durability refinements: candidate continuity/duplicate checks now precede artifact exposure; artifact staging precedes sequence reservation; matching existing artifacts/sidecars are re-synced on retry. A duplicate/ownership rejection regression failed before the ordering fix. State must be outside the public catalog directory.
- The real native CLI signed-fixture → publication → catalog verification journey passed with the new recorded-review and transaction inputs; proof sidecars still match the validator's bytes.
- Publication slice verification: 109 Hub/policy tests pass, including 11 release transaction unit tests and 4 CLI release tests. Real native CLI publication/catalog verification passed again after final ordering/private-state fixes. Independent final review has no findings; diff whitespace check passes.

## Plan 04 recovery and monitoring

- Added explicit recovery from authenticated durable history, preserving withdrawals and refusing stale/reused recovery identities before any pointer repair. Restored bundles/packs are synchronized before exposure. Recovery interruption and sync-failure regressions pass.
- Read-only JSON status/public probes verify age, signatures, sequence floors and a bounded pack sample; warnings at 7 days, critical at 12, expiry after 14.
- Daily renewal uses locked automatic sequence selection. Opt-in workflows use a protected preinstalled operator binary and an independent hosted read-only monitor. No workflows activated or signing infrastructure deployed.
- Independent final review found no remaining important issues. Corrected an always-run probe that could have executed after checksum refusal; monitor runs separately from the signer runner.
- Local HTTP fixture required loopback socket permission; reran with approved escalation. Workflow checksum probe revealed macOS sha256sum requires -c with an explicit stdin filename rather than --check; switched to portable -c - (also supported on the Linux runner).
- Native CLI publication and real HTTP healthy/damaged-pack probes passed. Workflow YAML/shell syntax and actual trusted-installation checksum rejection/success passed; GitHub execution remains unexercised.
- Recovery/monitoring checkpoint: 117 Hub/policy tests passed, diff whitespace clean. Operator setup and actual alert delivery remain unactivated. Next: coordinated consumer pin and plan 05.

## Plan 05 wire compatibility slice

- Plan 04 integration passed: fetched Hub 6ae3cca, full mobile shell check and 41 App Hub tests; mobile pin commit 7fc01ba pushed. Existing unrelated shell cfg warnings remain.
- Captured deterministic populated v1 catalog/manifest signatures and canonical bytes before changing wire types. Added explicit versioned wire types and normalized domain models; v1 has no new default fields. Three new cases initially failed for unknown catalog schemas/v2 contracts and now pass.
- V2 requires SemVer display versions, positive numeric release numbers and complete runtime/features/entrypoint/data contracts. Capability resolution still owns grants.
- An existing policy assertion treated schema 2 as permanently unknown; changed its fixture to schema 99 now that 2 has a defined format. Incomplete v2 remains explicitly refused. The temporary golden capture initially used the wrong sign_manifest argument order; corrected before capture.
- Upstream refreshed App Hub with script apps, first-party system apps, host-service sheets and a shared `app-hub-app`; the old mobile repository is archived. Merged the refreshed main branch into the feature worktree as 72b1006. Preserved both earlier Plan 05 worktrees in reversible stashes before merging.
- Reproduced and fixed a script admission regression caused by the older Card-only structural gate. Native validation now handles `main.splash` with bounded text reads, and a real script fixture passes its worker check. A second conditional-root regression exposed missing `sys`/agent vocabulary in the validator isolate; parity registration fixed it. Independent merge review identified that defect and found no further actionable integration problems.
- Ported installed-release lifecycle, independent Open during Update, sequence guards and revocation reporting from the archived mobile Hub UI into the new shared `app-hub-app`, retaining the refreshed system-app and service-sheet behavior. Two update tests were corrected to keep the installed release in the catalog, as required by exact-version launch.
- Restored Plan 05 wire changes on the merged baseline. Separate v1/v2 caches and sequence floors, numeric release selection, actionable compatibility reasons and strict wire parsing pass. Added `main.splash` as an advertised v2 UI entrypoint and bound the signed entrypoint to the actual bundle program. The detail screen shows an unavailable app's specific reason.
- Added operator v2 publication/renewal through an explicit `--catalog-schema 2` at `v2/catalog.json`, with a separate private state directory and unchanged v1 catalog. An integration test failed on the former v1-only writer, then passed after adding the format-aware transaction path. V2 status checks and the private-state boundary are covered. `docs/reference/manifest-v2.md` describes the dual-endpoint rollout.
- Full refreshed Hub workspace suite passed after these changes, including 15 compatibility tests, 42 shared-Hub UI tests, 9 native validator tests and v2 operator publication. Current Home is still pinned to upstream 0d36f50; coordinated active-consumer integration and device validation remain.
- Release-path review found three gaps: a lower v2 release number could follow a higher one; v1 and v2 could claim the same app ID under different publisher keys; repeated listing resolution scaled quadratically with release history. Each was reproduced or traced before correction. V2 now enforces strictly increasing release numbers, and listings deduplicate IDs before resolving history.
- V1/v2 writers now share a public-root lock and a signed publisher-ownership reservation directory in one private state parent. Existing signed catalog history seeds one proof; new app IDs use compact one-entry proofs, avoiding a full catalog copy per app. A new ownership claim is signed and durable before the per-schema pending intent. Interrupted-v2-before-signing and before-receipt tests confirm a competing v1 publisher cannot claim the same ID, and the original request can retry. Existing proof links and the shared-location marker are re-synced on retry. The cross-schema takeover and lower-release regressions passed after initially failing. Back up both state directories and the shared ownership directory together.
- Full refreshed Hub workspace suite passed again with the ownership and release-order fixes. `git diff --check` is clean. A repository-wide `cargo fmt --check` reports extensive pre-existing formatting drift across untouched files, so no broad formatting rewrite was applied. Active Home consumer integration remains.
- Final independent review of cross-schema ownership found no remaining important issue after compact signed proofs, pre-intent reservation and retry fsync. Hub commit `0b033c4` was pushed on `feat/app-store-foundations`; no main merge or production publication.
- Refreshed ROM Home now pins `0b033c4` alongside the Mail host service in an isolated System Apps worktree. Home drains exact revoked `(app_id, version)` releases and closes only matching mounted Card instances. The combined Home library suite passed 310 tests and the locked offline shell check passed, with existing `app-finance`/`app-robrix` feature warnings. ROM commit `6e1c159` and System Apps commit `05c2cf0` were pushed on integration feature branches. No Android/device validation or production activation is claimed.

## Plan 06 event/state foundation in progress

- Inspected the refreshed Octoscript L0 renderer and System Apps AppCard host. The renderer already provides bounded realization, stable instance keys, declared event transitions, payload-origin checks and a report of stale sources/durable writes. The installed Card host currently prepares a static source and has no route from native widget actions back to these transitions.
- Added `octosense-app-runtime`, a shared bounded Card state/event core. Its first test failed with an inert dispatch, then passed when dispatch authenticated the event against the current realized tree and applied the transition to a cloned state only after a complete re-render. A second test rejects a stale generation. The rendered text changes from `off` to `on` under the declared event. This is a synthetic event test; native widget routing, installed/reference host integration, persistence and effects remain.
- The full Hub workspace suite passed with the new crate. `docs/reference/app-runtime.md` records the implemented boundary and remaining integration without advertising an unsupported `app.logic@1` feature.
