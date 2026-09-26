# Review findings

- Hub repository starts clean at e86d43f; separate mobile UI lives in ../OctoSense-mobile/apps/app-hub.
- Hub contains policy, signing, admission, client, CLI, reference hosts and legacy store UI.
- First-app guide limits independently published apps to Card bundles; native features require a shell release.
- Starter contains metadata and icon only; a runnable Card and kit must come from a separate authoring workflow.
- ADR describes implementation and explicitly identifies publisher identity and a hosted service as incomplete; source tracing still needed.
- Existing outer-workspace planning files concern a completed unrelated PR; leave them intact.
- No tracked .github workflows, index entries or artifacts in current Hub tree; catalog is signed sequence 4, dated 2026-09-20, with zero entries.
- CLI publish is an operator command requiring the Hub working key; scan is optional and human approval is a boolean flag. No developer account/submission service is present in the reviewed repository.
- Mobile integration has real UpdateAvailable, consent bound to entry metadata, staging, replacement rollback/recovery, data preservation, cached browse and durable catalog sequence protection. Do not report these as absent.
- Shared Store::may_run refuses an installed version as soon as a newer entry becomes latest, even when the old version is still offered. Mobile Backend::may_open delegates to it.
- Gate verifies file types/listing/digest/policy but never requires page.card or kit, parses the Card, or decodes screenshot artwork.
- Publisher continuity compares the signature key ID string to the prior publisher label, not the prior public key. Needs isolated reproduction.
- Manifest has no runtime/SDK minimum, compatible runtime range or platform requirements; platform list is descriptive listing metadata and is not used by mobile admission.
- Modern mobile Entry drops support URL, privacy policy URL, age rating, license and platform metadata; no remove/uninstall entry point found in its backend/view so far.
- Card runtime loads page.card/data/kit only. Additional allowed .octoscript files are not loaded as an app entry point; Card service executor offers no assistant tools; reference host only prints the requested agent profile.
- Official Apple workflow baseline includes developer roles, build upload, beta testing, submission status and analytics. Use this as coverage, not a requirement to copy its complexity.
- Reproduced all three concerns in an isolated Rust probe under target/store-readiness-probe: a signed bundle without page.card/kit and with fake PNG bytes passes gate; different public key under original publisher ID passes update continuity; an installed v1 still marked Offered cannot open after legitimate v2 is appended.
- 42 Hub/policy tests (including one doctest) and 36 mobile App Hub tests passed, 78 total.
- Live GitHub API confirms zero workflows and no index/artifacts directories; live raw catalog matches local empty sequence 4. Planned OctoSense-org/publish-app endpoint returned HTTP 404 for this session (unavailable to an outside publisher; not proof no private development exists).
- Sibling runtime has a queued host-service bridge, but no consumer of take_splash_host_requests / splash_host_respond exists in the reviewed Hub or mobile shell. Do not equate declared device capabilities with implemented services.
- Current Card pipeline realizes supplied data into a UI tree; no generic application script entrypoint or wired app-specific service executor was found. Native widget interactions and network callback support exist and should not be described as universally nonfunctional.
- Core remove exists and legacy store has a removal flow; current mobile App Hub has no equivalent uninstall action in its view/backend.
- Static catalog hosting is a valid initial data plane. Its 14-day signature freshness needs scheduled renewal; a successful fetch of the same old catalog does not renew it.
- Official Play documents testing tracks and staged release management; these are absent from catalog model (Offered/Withdrawn only).

## Planning decisions

- Create a master roadmap, shared architecture/execution guide, and 26 implementation plans covering every review issue, including optional commerce/native tracks.
- Prioritize correctness and trust before exposing automated publishing. Move compatibility/schema work earlier because SDK, runtime, private testing and release targeting depend on it.
- Recommended control plane: small authenticated Rust service with a transactional registry/submission store and isolated workers; preserve the existing signed static distribution plane. GitHub-only workflow and full portal are alternatives, not separate first implementations.
- All currently nonexistent code/service/workflow paths will be labeled Create. Cross-repository changes are explicitly mapped to Hub, mobile shell, Makepad and Octoscript repositories; publish shared changes before updating consumers' revision pins.
- Plans will distinguish immediate v1-compatible fixes from versioned v2 contracts, and require a migration/compatibility strategy for signed payloads.
- External service providers, deployment region, signing custody and native platform support remain explicit implementation-time decisions with local mock adapters allowing independent work.
- User confirmed first-release scope: free Card apps first; native applications and payments later.
- Wrote 26 individual feature plans with concrete file ownership, behavioral test scenarios, proposed contracts, acceptance criteria and migration/recovery notes.
- Selected shared architecture: signed static distribution plus small authenticated service; alternatives and tradeoffs documented. Private beta and release rollback require separate data/trust semantics, not simply extra catalog status labels.
- Plans 16 (release channels) depend on Plan 17 (data migrations) for safe functional rollback; stable plan numbers are references, not a strict topological execution order.
- Completed the master importance ranking and executable delivery waves. 15 P0 plans qualify an invited free Card pilot; 7 P1 plans complete broader public-beta reliability/integration; 2 P2 growth plans and 2 optional product tracks follow.
- All 29 review issues map to one or more implementation plans; the final validator reports zero coverage, dependency or path errors.

## Release automation references — 2026-09-25

- GitHub primary documentation: scheduled workflows run from the default branch and can be delayed; concurrency limits overlapping runs but does not replace the filesystem transaction lock. Use protected operator runners with no submission checkout; external monitoring is needed to notice a scheduler/runner that never starts. Sources: https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#schedule and https://docs.github.com/en/actions/concepts/workflows-and-actions/concurrency .

## Refreshed repository topology — 2026-09-26

- App Hub upstream now owns the shared `crates/app-hub-app` shell UI, `main.splash` script apps, first-party system-app registration and host-service sheets. OctoSense-mobile is archived; active consumer integration belongs in OctoSense-rom/Home and its System Apps Mail dependency.
- Home currently pins Hub 0d36f50 in two Cargo sections; System Apps Mail service pins the same revision. Pin updates must keep that dependency graph coherent.
- The prior Card-only admission code rejected valid upstream script apps; the validator also lacked the `sys` vocabulary that both runtime hosts registered. Both defects were reproduced with native test fixtures and fixed on the merged branch.
- Plan 05's v2 reader default must remain opt-in until the `v2/catalog.json` endpoint contains every approved installed v1 release needed by upgraded clients. V2 release state and artifacts live in separate domains; a client cannot treat a v1 signature as v2 semantics.
