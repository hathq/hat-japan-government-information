# Japan Government Information HAT

This external HAT turns an exact user-selected Japanese public-information topic into a bounded, source-preserving research request. It does not interpret law, decide tax treatment, guess an administrative area, or fetch network content itself.

The released source directory is intentionally small and names official publishers and HTTPS origins. Network retrieval remains an externally governed communication operation. Unknown scope stays unresolved and is returned to Hatter for user confirmation.

The worker follows Hatter's lease protocol and processes one content-addressed invocation at a time.

`scripts/prepare-owner-local-federation.mjs` creates a one-hour, signed local
execution-directory fixture from an already signed official catalog. It verifies
the catalog and package before publishing the exact owner-local worker location,
keeps no signing secret, and is intended for product acceptance and local HAT
development. Production iHAT/Crowsi federation refresh remains a deployment
service responsibility; Hatter does not invent or bypass a placement.

After selecting that location, `scripts/verify-owner-local-scenario.mjs` runs
the same public Hatter CLI used by a user, registers the external worker,
submits one content-addressed request, verifies the e-Gov result and its
unresolved date, and correlates the final Hatter status. The script never
imports Hatter source and removes its temporary input/output stores.
