# 07. Host services, capability matrix and structured network policy Implementation Plan

> **For implementers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make declared app capabilities correspond to real, tested host services with predictable refusals.

**Architecture:** Drain the existing Splash host request bridge through an authenticated per-instance broker. Keep platform adapters in the shell/framework and policy/serialization in the shared runtime; only advertise services whose adapter has passed conformance.

**Tech Stack:** Rust, serde/JSON, the existing Hub policy/client, Makepad/Octoscript where applicable; additional service/storage adapters follow the [shared design](2026-09-24-app-store-design.md).

**Status:** In progress. The existing Splash request bridge is drained by installed/reference hosts; bounded pending replies, timeouts and close cancellation are being added. Storage/network/device conformance and a versioned support matrix remain. **Priority:** P0. **Phase:** B — Application platform. **Relative size:** L (complexity, not a delivery-date estimate).

**Prerequisites:** [05 — Versioned runtime contracts and release compatibility](2026-09-24-store-05-runtime-compatibility.md); [06 — Card application lifecycle, state and effects](2026-09-24-store-06-card-application-runtime.md)

**Review coverage:** R07, R09 in the [roadmap coverage matrix](2026-09-24-app-store-roadmap.md#review-coverage).

Read the shared design first for repository aliases, wire-compatibility rules, isolated development, meaningful test requirements and coordinated revision-pin updates. File paths below are exact relative to their named repository. “Create” means new code; “Modify” may refer to a file introduced by a prerequisite plan.

## Files

| Action | Path |
| --- | --- |
| Create | [H/crates/app-runtime/src/services.rs](../../crates/app-runtime/src/services.rs) |
| Create | [H/crates/app-runtime/tests/service_conformance.rs](../../crates/app-runtime/tests/service_conformance.rs) |
| Modify | [H/crates/app-policy/src/policy.rs](../../crates/app-policy/src/policy.rs) |
| Modify | [H/crates/app-hub/src/gate.rs](../../crates/app-hub/src/gate.rs) |
| Modify | [F/widgets/src/splash_host.rs](../../../makepad/widgets/src/splash_host.rs) |
| Modify | [M/src/main.rs](../../../OctoSense-mobile/src/main.rs) |
| Create | [M/src/card_services.rs](../../../OctoSense-mobile/src/card_services.rs) |
| Modify | [M/src/android_integration.rs](../../../OctoSense-mobile/src/android_integration.rs) |
| Create | [H/docs/reference/capabilities.md](../../docs/reference/capabilities.md) |
| Modify | [H/crates/app-runtime/src/lib.rs](../../crates/app-runtime/src/lib.rs) |
| Modify | [H/crates/app-runtime/Cargo.toml](../../crates/app-runtime/Cargo.toml) |
| Modify | [H/Cargo.lock](../../Cargo.lock) |

## Contract

The following is a proposed implementation contract, not an already-supported API:

```text
{
  "service": "storage.kv.get",
  "version": 1,
  "args": {"key": "notes"},
  "request_id": "host-issued",
  "result": {"ok": false, "code": "permission_denied", "retryable": false}
}
// Publisher input never supplies the authenticated app/release/instance.
// RuntimeDescriptor advertises supported service versions per platform.
```

## Implementation tasks

Each task is a small reviewable slice. Apply the five-step test/implementation cycle in the shared design to each scenario below; split a slice further when it cannot be reviewed independently. Preserve already passing behavior and commit each completed slice with only its own files.

### Task 1: Consume and settle host requests

**Touch:** `M/src/card_services.rs`, `F/widgets/src/splash_host.rs`, `H/crates/app-runtime/src/services.rs`.

1. **Write the regression/acceptance case** `every_host_request_resolves_or_times_out`: Queue an allowed request, denied capability, unknown method and dead-isolate request. Every live caller receives exactly one result or bounded timeout.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Connect take_splash_host_requests to the broker; authorize against the host's stored policy and respond through the owning isolate. Never trust app-supplied identity fields.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 2: Deliver storage and network primitives

**Touch:** `H/crates/app-runtime/src/services.rs`, `H/crates/app-runtime/tests/service_conformance.rs`.

1. **Write the regression/acceptance case** `storage_jail_and_network_host_limits_hold`: Exercise KV persistence/quota, HTTPS GET/POST JSON, redirect to an unlisted host, localhost, oversized response and offline mode. Include asset/data-loader network paths.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Reuse jailed storage and enforced network modules. Check every redirect/destination, timeout and response cap; represent credentials by host-owned secret handles and do not embed them in bundles.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 3: Wire device adapters in small slices

**Touch:** `M/src/card_services.rs`, `M/src/android_integration.rs`, `H/docs/reference/capabilities.md`.

1. **Write the regression/acceptance case** `device_permission_denial_is_a_typed_result`: Test prompt, clipboard, location, media/camera selection and approved shared-data read one adapter at a time on each advertised OS; test OS denial and service absence.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Implement adapters through existing platform services. Platform permission and per-app grant are both required. Unsupported adapters must return unavailable immediately and be omitted from the advertised matrix; notification/background/picker extras get separate capability versions before being advertised.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 4: Activate structured resource rules

**Touch:** `H/crates/app-hub/src/gate.rs`, `H/crates/app-runtime/tests/service_conformance.rs`.

1. **Write the regression/acceptance case** `visible_url_allowed_remote_asset_still_blocked`: Allow a displayed https URL and a request to an explicitly allowed API host. Reject remote artwork, path escapes, scheme tricks and computed requests to other hosts at runtime.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Activate Plan 03's structured inventory only after all fetch/resource loaders pass deny tests. Do not rely on text scanning as the containment boundary.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 5: Publish platform capability contracts

**Touch:** `H/docs/reference/capabilities.md`, `H/crates/app-runtime/src/services.rs`.

1. **Write the regression/acceptance case** `advertised_capability_requires_adapter_test`: Build a generated support table from registered adapter descriptors; remove/disable an adapter and verify compatibility selection refuses apps requiring it.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Version methods and errors; add working request examples for every supported method and an explicit unsupported list. Keep CI/device evidence tied to the runtime release.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

## Feature validation

Run from the Hub root unless a command changes directory. New packages/test targets are created by this plan or prerequisites. Use `--offline` only when dependencies are already cached; generate/update lockfiles once when intentionally adding dependencies, then use locked commands.

```sh
cargo test --locked -p octosense-app-runtime --test service_conformance
(cd ../OctoSense-mobile && cargo test --locked --bin octosense --features mobile-only,app-hub card_services)
```

Expected after implementation: all listed suites pass with zero failures. These commands have **not** been run to claim completion of the proposed feature. Native/device checks described in the tasks are additional acceptance evidence; a host-only test is not platform coverage.

## Acceptance criteria

- [ ] No queued service request silently waits forever; unsupported and denied calls are actionable.
- [ ] Network, storage and at least one real device-service example work in a contained installed app.
- [ ] Every advertised service/platform combination has passing conformance and device evidence.

## Rollout, migration and recovery

Enable storage/net first, then individual device adapters after tests. Free Card launch advertises only proven capabilities; expand the matrix through versioned runtime releases.

Keep the previous release/artifacts available while validating the new behavior. A catalog rollback publishes a newer signed sequence; never restore an older sequence to production. Preserve user data and report recovery failures rather than silently recreating it.

## Delivery checkpoint

Suggested commit subject after verified slices: `feat(runtime): broker contained Card host services`.

Use @superpowers:verification-before-completion before reporting success. Link the final test/native evidence and record updated dependency revisions in the owning pull requests. This planning document does not itself authorize deployment, credential creation, payments or public publication.
