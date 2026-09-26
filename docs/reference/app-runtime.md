# Card application runtime (implementation in progress)

The shared `octosense-app-runtime` crate now owns the first event/state
boundary for Card apps. It checks the authored L0 Card, realizes it with
Octoscript's bounded renderer, and accepts an event only when its key and
event name occur in the current realized tree. The runtime assigns a fresh
generation to each instance, so a response from a closed instance cannot
change a replacement instance. Payloads, Card source, data and realization
work have explicit limits. A failed render leaves the previous state intact.

The installed Card host, reference host and native validator now share a
`CardSession`. Portable L0 controls lower to native Makepad widgets. Their
callbacks use a random, host-owned channel and the session checks the current
Card tree before applying a declared transition and rebuilding the UI. The
channel grant is limited to `agent.notify`; it does not grant the app's agent
family or network access. Native-kit and existing script bundles retain their
prior rendering path.

When the manifest grants `storage`, the installed and reference hosts persist
portable Card state in a host-owned `.host/card-state` file keyed by app ID.
The file is outside the app's writable jail because it contains trusted value
origins. Snapshots have a versioned format and a 1 MiB limit, and each write
is also bounded by the app's declared storage quota. A state transition is
shown only after its snapshot replaces the previous file; a failed save
restores the previous in-memory state and keeps the app open. Without the
storage grant, Card state is usable in memory for that instance and starts
fresh on restart. The native validator loads actual widget source; restart,
wrong-channel and failed-save cases are tested through the same session API.

Physical tap/device evidence, migration across incompatible Card schemas,
asynchronous effects, lifecycle budgets and explicit `app.logic@1` entrypoints
remain Plan 06 work. Snapshot usage is capped independently of the isolate's
file-jail usage, so combined quota accounting also remains. `app.logic@1` is
not advertised yet.
