# Production readiness ledger

Last updated November 12, 2026. This page says what is in place, what is still needed before the hosted demonstration is live, and what is still needed before Nexus may hold information about real people. It records evidence, not intent: "done" means the thing exists and the check named beside it ran. Details of every check are in [verification](../development/verification.md).

## Where it stands

- **A synthetic-data demonstration is live behind real sign-in** since November 12, 2026: the site at `https://nexus-demo-orcin.vercel.app` (Vercel), the API and PostgreSQL on Render, sign-in through Auth0. The owner has signed in and reached the seeded workspace.
- **Only the first steps have been exercised on the real infrastructure**: health, the sign-in redirect, one sign-in, seeding. The list under "Before the demonstration is live" shows what is still open.
- **It is not ready for real patient data.** The reasons are listed under "Before real patient data". Some are engineering; several are contracts, policies and independent testing that code cannot supply.
- **Product scope is separate.** The [acceptance ledger](../product/acceptance-ledger.md) lists 50 capabilities, none accepted as complete. This page is about operating the system safely, not about whether the product is finished.

## In place

| Area | What exists | Evidence |
| --- | --- | --- |
| Sign-in | Hosted mode verifies an identity-provider token (signature, issuer, audience, expiry, verified email) and maps the email to exactly one active staff member. Local operator headers are ignored when hosted. | Unit and PostgreSQL tests; a browser pass against a mock provider. Not yet run against Auth0. |
| Identity provider | Auth0 application "Nexus Demo": single-page, no secret, refresh token rotation on; sign-in through Google (on Auth0's shared development keys) and a staff-only password connection with sign-up disabled. | Configured November 12. No one has signed in through it yet. |
| Access control | Organization and patient scope, role policies, primary-clinician scope and time-limited exceptions, checked on every route behind one gate. | Two code audits found no sign-in bypass and no cross-organization access. These were reviews, not a penetration test. |
| Records | Immutable signatures and source versions, integer-cent money, audit rows written in the same transaction as the change, explicit corrections. | PostgreSQL integration suites in CI. |
| Availability | Bounded database pool; statement and lock timeouts answered as retryable errors; liveness and readiness probes; supervised background workers; sign-in provider retried at startup; a handler crash answered instead of dropping the connection. | Unit and PostgreSQL tests; a short local smoke run. Not exercised under load. |
| Abuse limits | Per-address request limits; body-size and read-time caps; visit and upload tokens checked before the body is read; a cap on events per visit participant. | In-process tests. Not exercised behind real proxies. |
| Operability | One error format; a request id on every response and in the log; an access log with no addresses, tokens, emails or record content; startup fails with one line naming a missing setting. | Tests and the local smoke run. |
| Browser | Content security policy limited to the site and the identity provider; framing denied; strict transport security; an error boundary and plain messages for network failures; session renewal. | Frontend tests and build. The boundary and failure screens were not exercised in a browser. |
| Build and repository | `main` and `dev` protected: pull request and seven required checks, no force-push. CI runs format, lint, build, unit and PostgreSQL tests, frontend build, container build, secret scanning, dependency advisories, CodeQL and licence checks. Actions pinned to commits. Dependabot, `SECURITY.md`, `CODEOWNERS`. | Running on every pull request. |
| Deployment definition | `render.yaml` blueprint (API and PostgreSQL on Render's smallest paid plans, deploys `main` after checks pass); `frontend/vercel.json` builds only `main` and refuses to build with placeholder addresses. | Files reviewed; never applied. |

## Before the demonstration is live

| Step | Who | State |
| --- | --- | --- |
| A way to sign in | Owner | Done November 12: Google sign-in is on for the application. No `Nexus-Staff` password account exists yet. |
| Create the API and database on Render | Owner | Done November 12, on the paid plans (about $17.50 a month). |
| Put the API's address into `frontend/vercel.json` | Developer | Done November 12 (release 31). |
| Seed synthetic data ([demo hosting](demo-hosting.md), step 4) | Owner and developer | Done November 12. One request failed with a temporary database error and the run was resumed; the cause is not known. |
| Deploy the frontend and check sign-in ([demo hosting](demo-hosting.md), steps 7 and 8) | Owner | Done November 12: the owner signed in with Google and reached the workspace. |
| Set `NEXUS_TRUSTED_PROXIES` from the API log ([demo hosting](demo-hosting.md), step 9) | Developer | Done November 12: 2, measured from a direct call. |
| Replace the database credential | Owner | Open. The connection string was pasted into a development chat during seeding. |
| Exercise the rest on the live system: staying signed in, signing out, a second role, a visit, an over-limit caller | Owner and developer | Open |
| Review the ten open Dependabot updates, three of them major versions (`sqlx`, `thiserror`, `sha2`) | Developer | Open. Not needed for the demonstration. |

Confirmed on the first deploy: the container image starts on Render and reaches its database; Auth0's sign-in and token exchange work under the narrowed content security policy (the owner signed in); Render adds two proxy addresses to a direct request. Still unknown: refresh-token renewal; sign-out; recording chunks of up to 8 MB through the Vercel rewrite.

## Before real patient data

None of these is done. The hosted demonstration must hold synthetic records only until all of them are.

**Agreements and policy (owner)**

- Business associate agreements with every vendor that would hold or carry protected health information: the container and database host, the identity provider, the frontend host, and any TURN, email or speech provider. Several vendors offer these only on specific paid plans; some do not offer them at all, which would mean choosing a different vendor.
- A HIPAA security risk analysis, and the written policies it leads to: access management, incident response, breach notification, workforce training, device and media handling, retention and disposal.
- A decision on state law, payer and care-setting requirements beyond outpatient, which are not yet specified.

**Identity and session**

- Multi-factor authentication required for every staff account, a production identity-provider tenant separate from the development one, and Google sign-in either removed or given its own Google client instead of Auth0's shared development keys.
- Sign-out after inactivity. Today a session lasts as long as the identity provider's token and its renewals.
- A reviewed process for adding, changing and removing staff, including what happens to an open session when someone is deactivated.

**Data protection and recovery**

- Scheduled database backups with point-in-time recovery, and a restore that has actually been performed and timed. No restore drill has been run.
- Custody and recovery of the media encryption key. Losing it makes recordings unreadable, and there is no recovery plan.
- Encryption at rest confirmed for the database and its backups on the chosen host.
- Retention, legal hold and disposal for records, recordings and logs.

**Availability and capacity**

- Recording playback and transcription read a whole track into memory, and the transcription worker holds the organization's lock while it does. Both need streaming or a narrower lock.
- The workspace bootstrap and several lists load everything the caller may see, without pages. They need paging and limits before real volumes.
- Writes for one organization run one at a time, and knowledge search waits behind them. This has not been measured under load; it needs measuring and probably narrowing.
- The request limiter is in one process's memory, and people who come through the site are counted together because the API's own address is public. Real use needs the API reachable only from the site, limits counted per person, and a network-level limit in front.
- One API instance. A second instance, graceful shutdown of in-flight work, and a zero-downtime migration practice are not in place.
- Video calls across networks need a TURN service; none is provided.
- Monitoring and alerting: nothing watches the health probes, error rates or logs and notifies anyone.

**Security assurance**

- An independent penetration test of the hosted system, and fixes for what it finds.
- Dependency and container image scanning acted on as a routine, including the open updates above.
- Log retention and review, and audit-log export, defined and tested.

**Clinical and product acceptance**

- Review by clinicians and billers of the workflows they would use. None has happened.
- Speech recognition and clinical drafting are not available when hosted, and the drafting evaluation found the model could produce unsupported content. Drafting stays off for real use until that is resolved.
- The capabilities in the acceptance ledger that are missing or partial and are required for the first customer.

## Keeping this page honest

Change a row only when the evidence exists, and say what the evidence is. When something is deployed for the first time, move its unknowns from "first-deploy unknowns" to "In place" or to a new open item, with the date. Do not mark anything under "Before real patient data" as done on the strength of code alone where the item is a contract, a policy or an independent test.
