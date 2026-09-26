# Card application runtime (implementation in progress)

The shared `octosense-app-runtime` crate now owns the first event/state
boundary for Card apps. It checks the authored L0 Card, realizes it with
Octoscript's bounded renderer, and accepts an event only when its key and
event name occur in the current realized tree. The runtime assigns a fresh
generation to each instance, so a response from a closed instance cannot
change a replacement instance. Payloads, Card source, data and realization
work have explicit limits. A failed render leaves the previous state intact.

The current API returns the rendered tree and any declared durable writes or
stale sources to a host; it does not execute effects itself. The installed
Card host, reference host and native validator do not use this crate yet.
The next slice must map actual Makepad widget actions to the declared Card
keys, use this same runtime in all three hosts, and prove a real native tap
changes displayed text. Persistence, asynchronous services and lifecycle
budgets follow in the remaining Plan 06 tasks. Until that integration lands,
`card.ui@1` remains a static Card rendering contract; `app.logic@1` is not
advertised.
