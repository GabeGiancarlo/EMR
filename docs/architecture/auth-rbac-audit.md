# Identity, permissions and audit

Authentication is deferred at the owner's request. `x-nexus-actor` selects an active staff row in local development. Anyone able to call the API can supply that header; it is not verified identity. The server requires `NEXUS_LOCAL_DEV=1` and refuses a public bind.

Implemented policies enforce organization scope, resource read/write access, primary-client assignment for clinicians, expiring/revocable exceptions, and signed-record/financial transition restrictions. Administrative grants and staff management require an administrator. Billing and executive roles do not ordinarily read clinical narrative. The frontend exposes actions from effective permissions; the backend remains authoritative.

Facility references are operational context, not a complete facility ACL. Primary assignment is not a multi-member care-team policy. These require application work even after OAuth is installed.

Mutations and exports write audit events transactionally with actor, organization, action, resource, record identifier and time. Ordinary bootstrap/resource reads do not yet provide a complete sensitive-read audit trail. Events omit clinical text. Local audit storage is not an independently protected production audit system.

Offboarding deactivates staff, revokes exceptions and transfers open work and clinical assignments. Clinical assignments cannot transfer to billing-only staff, including when the departing operator is an administrator.

Public use requires verified identity, secure deployment, expanded access/read-audit policy and operational validation. See [verification](../development/verification.md); no compliance certification is implied.
