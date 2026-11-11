# Identity, permissions and audit

The API has two identity modes, chosen by `NEXUS_AUTH_MODE` (updated November 11, 2026).

**Local (`local`, the default).** `x-nexus-actor` selects an active staff row. Anyone able to call the API can supply that header; it is not verified identity. The server requires `NEXUS_LOCAL_DEV=1` and refuses a non-loopback bind.

**Hosted (`oidc`).** Every `/api` route passes a gate (`api/src/auth.rs`) that requires `Authorization: Bearer <JWT>` from one configured OpenID Connect provider. The gate checks the signature against the provider's published keys (cached, re-downloaded on an unknown key id at most once per 30 seconds and hourly), the issuer, audience, expiry and not-before, and requires a verified email. It resolves that email to exactly one active `staff` row and stores the staff id on the request; `access::staff_id` is the only place identity is read. `x-nexus-actor` is never read, the unauthenticated operator listing in `/api/bootstrap` is not served, and `/api/setup` is refused. Missing or invalid tokens get 401; unknown, inactive, ambiguous and unverified accounts get one identical 403. Only asymmetric signature algorithms are accepted. Three route groups carry their own single-purpose credential and are outside the staff gate: `/api/auth/config` (public sign-in description), `/api/telehealth/visit*` (patient invitation token) and `/api/recording-tracks/*` (per-track upload token).

Hosted identity is the staff email. It is not multi-factor policy, session revocation, device trust or patient/proxy identity: a deactivated staff member loses access on the next request, but a token cannot otherwise be revoked before it expires. The hosted mode is intended for a synthetic-data demonstration; see [demo hosting](../deployment/demo-hosting.md).

Implemented policies enforce organization scope, resource read/write access, primary-client assignment for clinicians, expiring/revocable exceptions, and signed-record/financial transition restrictions. Administrative grants and staff management require an administrator. Billing and executive roles do not ordinarily read clinical narrative. The frontend exposes actions from effective permissions; the backend remains authoritative.

Facility references are operational context, not a complete facility ACL. Primary assignment is not a multi-member care-team policy. These require application work regardless of sign-in.

Mutations and exports write audit events transactionally with actor, organization, action, resource, record identifier and time. Ordinary bootstrap/resource reads do not yet provide a complete sensitive-read audit trail. Events omit clinical text. Local audit storage is not an independently protected production audit system.

Offboarding deactivates staff, revokes exceptions and transfers open work and clinical assignments. Clinical assignments cannot transfer to billing-only staff, including when the departing operator is an administrator.

Public use requires verified identity, secure deployment, expanded access/read-audit policy and operational validation. See [verification](../development/verification.md); no compliance certification is implied.
