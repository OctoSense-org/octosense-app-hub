# 06. Card application lifecycle, state and effects Implementation Plan

> **For implementers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make a downloaded Card bundle a functioning app with event handlers, durable state and bounded effects.

**Architecture:** Add a shared app-runtime crate used by installed Card hosts, the reference host and validation. Reuse Octoscript evaluation and native widget event identity; do not invent another UI language or permit arbitrary host code.

**Tech Stack:** Rust, serde/JSON, the existing Hub policy/client, Makepad/Octoscript where applicable; additional service/storage adapters follow the [shared design](2026-09-24-app-store-design.md).

**Status:** In progress. Bounded L0 event/state dispatch and shared native host wiring are implemented; durability, effects and device evidence remain. **Priority:** P0. **Phase:** B — Application platform. **Relative size:** L (complexity, not a delivery-date estimate).

**Prerequisites:** [03 — Runnable bundle admission and actionable validation](2026-09-24-store-03-bundle-admission.md); [05 — Versioned runtime contracts and release compatibility](2026-09-24-store-05-runtime-compatibility.md)

**Review coverage:** R06, R07 in the [roadmap coverage matrix](2026-09-24-app-store-roadmap.md#review-coverage).

Read the shared design first for repository aliases, wire-compatibility rules, isolated development, meaningful test requirements and coordinated revision-pin updates. File paths below are exact relative to their named repository. “Create” means new code; “Modify” may refer to a file introduced by a prerequisite plan.

## Files

| Action | Path |
| --- | --- |
| Create | [H/crates/app-runtime/Cargo.toml](../../crates/app-runtime/Cargo.toml) |
| Create | [H/crates/app-runtime/src/lib.rs](../../crates/app-runtime/src/lib.rs) |
| Create | [H/crates/app-runtime/src/lifecycle.rs](../../crates/app-runtime/src/lifecycle.rs) |
| Create | [H/crates/app-runtime/src/state.rs](../../crates/app-runtime/src/state.rs) |
| Create | [H/crates/app-runtime/tests/app_lifecycle.rs](../../crates/app-runtime/tests/app_lifecycle.rs) |
| Modify | [H/Cargo.toml](../../Cargo.toml) |
| Modify | [H/crates/appstore/src/cardapp.rs](../../crates/appstore/src/cardapp.rs) |
| Modify | [H/crates/card-host/src/main.rs](../../crates/card-host/src/main.rs) |
| Modify | [H/crates/app-validator/src/lib.rs](../../crates/app-validator/src/lib.rs) |
| Modify | [K/crates/octoscript-widgets/src/kit.rs](../../../octoscript-makepad/crates/octoscript-widgets/src/kit.rs) |
| Modify | [K/crates/octoscript-makepad/src/l0.rs](../../../octoscript-makepad/crates/octoscript-makepad/src/l0.rs) |
| Create | [H/docs/reference/app-runtime.md](../../docs/reference/app-runtime.md) |
| Modify | [H/Cargo.lock](../../Cargo.lock) |

## Contract

The following is a proposed implementation contract, not an already-supported API:

```text
// SDK contract, implemented in the existing script language:
init(context) -> state
update(state, event) -> {state, effects}
view(state) -> card_data
dispose(context) -> void
// Event = {control_id, action, payload}; asynchronous effect responses
// return through update. No app callback receives native host objects.
```

## Implementation tasks

Each task is a small reviewable slice. Apply the five-step test/implementation cycle in the shared design to each scenario below; split a slice further when it cannot be reviewed independently. Preserve already passing behavior and commit each completed slice with only its own files.

### Task 1: Prove the event-to-state path

**Touch:** `H/crates/app-runtime/src/lifecycle.rs`, `K/crates/octoscript-widgets/src/kit.rs`, `H/crates/app-runtime/tests/app_lifecycle.rs`.

1. **Write the regression/acceptance case** `button_event_updates_card_state`: Create a tiny app with a button and counter. Run its actual Card/kit/logic through the host and assert a native input changes rendered text exactly once.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Add v2 logic entrypoint loading after admission. Map stable control IDs and widget actions to serialized app events; use the same lifecycle adapter in both hosts.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 2: Add state persistence and restart

**Touch:** `H/crates/app-runtime/src/state.rs`, `H/crates/appstore/src/cardapp.rs`, `H/crates/card-host/src/main.rs`.

1. **Write the regression/acceptance case** `saved_note_survives_process_restart`: Create/edit a note, stop the host, restart and inspect persisted state. Denied storage must return a typed error and preserve usable UI.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Provide bounded app-scoped KV state through the service interface. Separate immutable bundle bytes from writable data; migrate the existing app-root layout without losing user files.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 3: Control asynchronous effects

**Touch:** `H/crates/app-runtime/src/lifecycle.rs`, `H/crates/app-runtime/src/lib.rs`.

1. **Write the regression/acceptance case** `late_response_cannot_mutate_closed_app`: Issue two effects, close/reopen the app, and deliver old responses out of order. A stale instance/generation response cannot enter a new instance.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Tag requests with host-owned app/release/instance identity. Serialize state updates; apply timeouts, cancellation and bounded queues; let Plan 07 supply effect implementations.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 4: Enforce lifecycle budgets

**Touch:** `H/crates/app-runtime/src/lifecycle.rs`, `H/crates/appstore/src/cardapp.rs`.

1. **Write the regression/acceptance case** `runaway_handler_is_stopped_without_freezing_shell`: Run a looping handler, oversized state and repeated init/dispose cycles. Host stays responsive, errors are visible, and memory/callback resources are reclaimed.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Apply existing instruction/heap limits to every entry and response; retain the host's containment settings. Add foreground/background/close events without granting background execution by default.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

### Task 5: Use the same runtime in validation

**Touch:** `H/crates/app-validator/src/lib.rs`, `H/docs/reference/app-runtime.md`.

1. **Write the regression/acceptance case** `validator_runs_real_app_interaction`: Run the counter/notes lifecycle through app-validator and installed host; require matching effects/outputs and reject undeclared entrypoints or handler exports.
2. **Run the relevant suite below before implementation.** Filter to that case where supported. Expected: the new behavior fails for the identified reason; if it already passes, inspect whether the scenario truly reaches the implementation and retain it only if it protects an uncovered contract.
3. **Implement:** Extend runtime validation to declared app tests and lifecycle initialization. Document source layout, async semantics, supported APIs and immutable code/data separation.
4. **Verify:** rerun the new case and its containing suite. Expected: the scenario and existing affected behavior pass; record actual output. For device/UI scenarios, collect native evidence as well as headless assertions.
5. **Review and checkpoint:** inspect the diff for this slice, update its documentation and record the validation. Commit only the owning repository's files; do not stage unrelated work.

## Feature validation

Run from the Hub root unless a command changes directory. New packages/test targets are created by this plan or prerequisites. Use `--offline` only when dependencies are already cached; generate/update lockfiles once when intentionally adding dependencies, then use locked commands.

```sh
cargo test --locked -p octosense-app-runtime --test app_lifecycle
cargo test --locked -p octosense-app-validator --test runtime_validation
```

Expected after implementation: all listed suites pass with zero failures. These commands have **not** been run to claim completion of the proposed feature. Native/device checks described in the tasks are additional acceptance evidence; a host-only test is not platform coverage.

## Acceptance criteria

- [ ] A downloaded app completes a real input/state/persistence journey without an external Python/browser controller.
- [ ] Reference, validator and installed hosts share the same runtime behavior and permission boundaries.
- [ ] Closing/updating apps cancels pending work and cannot leave callbacks attached to new instances.

## Rollout, migration and recovery

Gate logic entrypoints behind v2 compatibility. Keep existing UI-only Cards supported; change shared runtime repositories first, then pin their releases in Hub/mobile.

Keep the previous release/artifacts available while validating the new behavior. A catalog rollback publishes a newer signed sequence; never restore an older sequence to production. Preserve user data and report recovery failures rather than silently recreating it.

## Delivery checkpoint

Suggested commit subject after verified slices: `feat(runtime): add Card application lifecycle and state`.

Use @superpowers:verification-before-completion before reporting success. Link the final test/native evidence and record updated dependency revisions in the owning pull requests. This planning document does not itself authorize deployment, credential creation, payments or public publication.
