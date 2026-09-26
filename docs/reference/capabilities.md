# App host capabilities (implementation in progress)

The v2 runtime descriptor currently advertises `storage`, `net` and `prompt`.
The manifest can request only names in `KNOWN_CAPABILITIES`; a request is not
evidence that every platform supplies a host adapter. `card.ui@1` and
`script.ui@1` are the advertised UI contracts. `app.logic@1`, agent sessions
and individual host-service versions are not advertised.

The Splash `host.request` bridge checks the isolate's granted capability
before queueing. Installed and reference Card hosts drain only their own
isolate requests, bind the app ID in the host, and return an answer for an
unknown service. A pending service has a 30-second timeout, at most 32
requests per isolate and 256 per process. Closing a Card cancels its pending
requests; late replies are discarded. Arguments are limited to 64 KiB and
answers to 1 MiB. A background surface cannot raise a host sheet.

`storage` permits a Card's bounded state snapshot; its state file is host-owned.
The existing Splash jail and network gates enforce script access to the app's
file jail and declared network hosts. `mail` is registered by the System Apps
Mail host service where available. Location, clipboard, camera, media and
other device services need adapter and device conformance before they can be
listed in a platform support matrix. Source refetches and durable collection
writes from portable L0 Cards currently return a refusal instead of silently
pretending to complete an unimplemented effect.
