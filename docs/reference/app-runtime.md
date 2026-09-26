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

The native validator loads the actual widget source and tests a state change
through the same session API. A physical tap in the installed shell has not
yet been captured as device evidence. State is currently in memory: durable
storage, asynchronous effects, lifecycle budgets and explicit `app.logic@1`
entrypoints remain Plan 06 work. `app.logic@1` is not advertised yet.
