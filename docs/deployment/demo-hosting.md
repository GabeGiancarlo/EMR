# Hosting the synthetic demonstration

Written November 11, 2026. Nothing described here has been deployed. The steps were rehearsed on one Mac against a throwaway database and a mock identity provider; see [verification](../development/verification.md) for exactly what ran. Provider dashboards change, so treat the click paths as a guide and the values as the contract.

## What this is, and is not

This hosts Nexus as a **demonstration that contains synthetic records only, behind real sign-in**.

- **Do not enter information about real people.** No real patient, staff, payer or clinical data. The hosted configuration has not had a penetration test, a HIPAA risk analysis or business associate agreements, and it has no intrusion detection, monitoring, backup policy or key-recovery plan. The full list is in [production readiness](production-readiness.md).
- Sign-in proves who someone is. It does not make the system fit for protected health information.
- Speech recognition and clinical drafting are off when hosted (the image has no models). Recording is off unless you add a media key. Video calls across networks need a TURN server that is not provided.

## The pieces

| Piece | Where | Why |
| --- | --- | --- |
| Frontend | Vercel (your personal account) | Static site built from `frontend/`. |
| API | A container host | A long-running server with two background workers. Vercel cannot run it. |
| Database | Managed PostgreSQL 17 | The API applies its own migrations at startup. |
| Sign-in | An OpenID Connect identity provider | The API verifies the provider's tokens; it stores no passwords. |

The browser only ever talks to the Vercel address. `frontend/vercel.json` forwards `/api/*` and `/healthz` to the API, so there are no cross-site requests.

How sign-in works: the browser sends the person to the provider's own login page (authorization code flow with PKCE, no client secret). The provider returns an identity token. The browser keeps it in memory only and sends it with each API request. The API checks the signature against the provider's published keys, the issuer, the audience, the expiry and not-before times, and that the email is verified. It then looks for exactly one active staff member with that email. Anyone else gets a plain refusal. The development header `x-nexus-actor` is ignored.

## Recommendations (your decision)

- **Identity provider: Auth0.** It is plain OpenID Connect, has a free tier, supports a single-page application with PKCE and no secret, and lets you turn public sign-up off. Any provider works if it offers all of these:
  - a public client with PKCE and a discovery document;
  - browser (CORS) access to its token endpoint for your site;
  - an ID token that carries `email` **and** `email_verified: true`. Nexus refuses a token without a verified-email statement, and this is not configurable. Microsoft Entra ID does not send `email_verified`, so every sign-in through it is refused; do not choose it for this version.

  Okta, Google, Keycloak and Clerk are candidates. Only the mock provider has been tested.
- **Container host and database: Render** (a Docker web service plus Render PostgreSQL in the same region). It builds from a Dockerfile in a GitHub repository, holds secrets, and health-checks a path. Fly.io and Railway are equivalent choices. The API's workers check the database every two seconds, so it needs an always-on instance, and a database billed by active time will never go idle.

Tell me which you choose and I will adjust the files; I have not created any account.

## What you need to do

Do these in order. Steps marked **(you)** need your accounts and cannot be done for you.

### 1. Decide the public address (you)

Pick the Vercel project name, for example `nexus-demo`. Its address will be `https://nexus-demo.vercel.app`. This exact origin, with no trailing slash, is used in three places below. Call it `SITE`.

### 2. Create the identity provider application (you)

Auth0 names; other providers have equivalents.

1. Create a tenant, then **Applications → Create Application → Single Page Web Application**.
2. In the application's settings set:
   - Allowed Callback URLs: `SITE/` (with the trailing slash, for example `https://nexus-demo.vercel.app/`)
   - Allowed Logout URLs: `SITE/`
   - Allowed Web Origins and Allowed Origins (CORS): `SITE`
3. Turn off public sign-up (**Authentication → Database → your connection → Disable Sign Ups**) and create a user for yourself and for anyone else who should get in. Each must verify their email; an unverified email is refused.
4. Note three values:
   - Issuer: `https://YOUR-TENANT.REGION.auth0.com/`. Auth0's issuer ends with a slash. The API compares it exactly and tells you at startup if it differs.
   - Client ID.
   - There is no client secret to copy. Do not create one.

**As set up on November 12, 2026.** Auth0 tenant `dev-vulcan-01` (US), issuer `https://dev-vulcan-01.us.auth0.com/`. Application "Nexus Demo" (single-page, no secret), client ID `hiSdQk1bhasqtTMo2Lu60Jba3qa2htJt`, with the callback, logout and web-origin addresses set to `https://nexus-demo-orcin.vercel.app`. Refresh token rotation is on. The application signs in through two connections: Google (`google-oauth2`), and the database connection `Nexus-Staff`, which has sign-ups disabled. The tenant's shared `Username-Password-Authentication` connection is switched off for this application, so password accounts from other applications in the tenant cannot reach the sign-in. Google was switched on for the first demonstration so the owner can sign in without a new password; anyone with a Google account can reach the provider's sign-in, and the API then refuses every email that is not an active staff member. The tenant's Google connection uses Auth0's shared development keys, which Auth0 does not support for production: before real use, give it its own Google client or switch it off again. People are added under **User Management → Users → Create User**, connection `Nexus-Staff`, and must verify their email.

Recommended: enable refresh token rotation for the application and set `NEXUS_OIDC_SCOPES` to `openid email profile offline_access`. Without it a session ends when the identity token expires (ten hours by default on Auth0) and anything unsaved at that moment is lost. The session then renews about a minute before each ID token expires, provided the provider returns a new ID token from the refresh (Auth0 does). If a provider answers a refresh without an ID token, which OpenID Connect allows, the session simply runs to the end of the current ID token and is not cut short. Without refresh, or with such a provider, a session lasts as long as the provider's ID token (Auth0 default: 10 hours) and then shows "Your session ended"; signing in again is one click while the provider's own session is alive.

### 3. Create the database (you)

Create a managed PostgreSQL 17 database. Copy its connection string and make sure it ends with `?sslmode=require` (or the host's equivalent). This string is a secret.

### 4. Seed synthetic data (you, on your Mac)

Seeding runs through a local-mode API on your own machine that is pointed at the hosted database, because it acts as the workspace administrator before anyone can sign in.

**One command.** From the repository root, on an up-to-date `main` checkout:

```bash
tools/seed-hosted-demo.sh you@example.com
```

It asks you to paste the database's external connection string (nothing is shown as you paste, and it is never printed or saved), refuses a string that points at this machine, starts the local-mode API on port 8095, seeds, and stops the API. Add `role=email` arguments to give other roles a sign-in, for example `clinician=someone@example.com`.

**By hand**, the same thing in two terminals. From the repository root:

```bash
NEXUS_LOCAL_DEV=1 PORT=8095 DATABASE_URL='PASTE-THE-HOSTED-CONNECTION-STRING' cargo run -p emr-api
```

In a second terminal, using the emails of the provider accounts that should be able to sign in:

```bash
python3 tools/seed-demo.py --api http://127.0.0.1:8095 --admin-email you@example.com --staff clinician=someone@example.com --confirm-synthetic-demo
```

Then stop the API with Ctrl-C. Notes:

- **Seed from the commit that is deployed.** The local API applies its own database migrations when it starts, so run this step from an up-to-date `main` checkout (`git switch main && git pull`). Seeding from a branch with newer migrations would move the hosted database ahead of the hosted API.
- **One journal per database.** The script records what it created in `.local/demo-seed-state.json`. If you delete and recreate the hosted database, pass a fresh `--journal` path (or delete the old file) so the script does not skip setup and then fail on records that no longer exist.
- Before running it, read the database host in your connection string once more. This step writes to whatever database that string names.

- The script creates one organization, six staff, three synthetic clients and example scheduling, documentation and billing records. It never deletes anything and refuses a database that already holds a different workspace.
- Roles you do not name (`clinical_lead`, `clinician`, `billing`, `executive`, `support`) get placeholder emails that cannot sign in. To try another role, pass `--staff role=email` with a second provider account, or add staff later under **Team & access** as the administrator.
- Each email can belong to only one staff member across the whole database.
- If your host gives the database an internal and an external address, this step needs the external one; the hosted API (step 5) can use the internal one.

### 5. Deploy the API (you)

**Fast path on Render.** The repository has a blueprint, `render.yaml`, that creates the API and the database together, so steps 3 and 5 become one: in Render choose **New → Blueprint**, pick `GabeGiancarlo/EMR`, and apply it. It asks for nothing: the identity provider values and the site address are written in the blueprint (none is a secret; the browser is sent all of them), and `DATABASE_URL` is wired automatically. If the tenant, the application or the site address changes, change them in `render.yaml`. It uses Render's smallest paid plans on purpose: a free web service sleeps when idle and a free database is deleted after a trial period, and neither has backups. The owner created it on November 12, 2026 as blueprint "Nexus": the API is `https://nexus-api-j435.onrender.com` and the database is `nexus-db`, with the plan's default 15 GB disk. At Render's published prices that day this is $7 a month for the API, $6 a month for the database and $4.50 a month for 15 GB of storage, about $17.50 a month, billed by the second. A disk cannot be made smaller; the only way to pay less for storage is to delete the database and create a new one. It deploys a commit on `main` only after that commit's GitHub checks pass. Vercel's deployment counts as one of those checks, so a failed Vercel build of a commit also holds back the API's deploy of that commit; the first deploy, made when the blueprint is created, does not wait. If you use the blueprint, do step 4 (seeding) after it has created the database, using the database's **external** connection string.

**By hand, on any container host:**

1. Create a web service from the GitHub repository `GabeGiancarlo/EMR`, runtime Docker, Dockerfile path `infra/dockerfiles/Dockerfile.api`, build context the repository root.
2. Health check path: `/livez` (the process is serving; it never touches the database, so a database blip does not get a working process restarted). `/healthz` is readiness: it answers 503 when the database does not respond within two seconds, and lists the background workers.
3. Environment variables (see `.env.example` for all of them):

| Variable | Value |
| --- | --- |
| `DATABASE_URL` | The connection string from step 3. Secret. |
| `NEXUS_OIDC_ISSUER` | The issuer from step 2, exactly. |
| `NEXUS_OIDC_CLIENT_ID` | The client ID from step 2. |
| `NEXUS_OIDC_AUDIENCE` | The same client ID. |
| `NEXUS_ALLOWED_ORIGINS` | `SITE` |
| `NEXUS_TRUSTED_PROXIES` | The number of the host's own proxies in front of the API. See "Request limits" below. |
| `NEXUS_OIDC_SCOPES` | `openid email profile offline_access`, if refresh token rotation is on (step 2). Optional. |

The image already sets `NEXUS_AUTH_MODE=oidc`, `HOST=0.0.0.0` and `RUST_LOG=info` (so refused sign-ins and provider problems appear in the host's log). Do not set `NEXUS_LOCAL_DEV`. If the host assigns `PORT`, leave it alone; otherwise the API listens on 8090.

If something is missing the service exits at once with one line naming it, for example `Nexus API did not start: Hosted mode (NEXUS_AUTH_MODE=oidc) cannot start. Missing: NEXUS_OIDC_ISSUER.` Note the service's public address, for example `https://nexus-api.onrender.com`. Call it `API`. Opening `API/healthz` should show `"authentication":"oidc"`.

### 6. Point the frontend at the API and the identity provider (me, once you send the addresses)

`vercel.json` cannot read environment variables, so two addresses are written into `frontend/vercel.json` by hand. Three edits, all done as of November 12, 2026: the rewrites name `nexus-api-j435.onrender.com` and `connect-src` names `https://dev-vulcan-01.us.auth0.com`. They are listed here for when an address changes:

| Line to change | Replace | With |
| --- | --- | --- |
| `rewrites`, source `/api/:path*` | `REPLACE-WITH-API-HOST.example.invalid` | the host of `API`, for example `nexus-api.onrender.com` |
| `rewrites`, source `/healthz` | `REPLACE-WITH-API-HOST.example.invalid` | the same host |
| `Content-Security-Policy`, `connect-src` | the identity provider origin listed after `'self'` | the provider's origin, for example `https://your-tenant.us.auth0.com` (scheme and host only, no path, no trailing slash) |

`connect-src` lists the only places the page may send requests: its own origin and the identity provider, whose token endpoint finishes sign-in. Do not widen it to `https:`.

Until all three are filled in, Vercel refuses to build. The build command runs `node scripts/check-hosting-config.mjs` first, and the deployment log shows:

```
Nexus cannot be deployed yet: frontend/vercel.json is not filled in.
  - rewrites: the destination for /api/:path* is "https://REPLACE-WITH-API-HOST.example.invalid/api/:path*". Replace its host with the API's public HTTPS address.
```

To check before pushing, run this in `frontend/`:

```bash
VERCEL=1 node scripts/check-hosting-config.mjs
```

This is a normal change: a `feature/` branch and a pull request into `dev`. Send me the two addresses and I will make it.

### 7. Deploy the frontend (you)

1. In Vercel (`gabegiancarlogk-8268s-projects`), **Add New → Project**, import `GabeGiancarlo/EMR`.
2. Root Directory: `frontend`. The framework, build command and output directory come from `vercel.json`.
3. No environment variables are needed. Leave `VITE_API_BASE_URL` unset.
4. Production Branch: `main`. The work reaches `main` by a pull request from `dev`; until then, set the production branch to `dev` if you want to try it before releasing.
5. Check the project's address equals `SITE`. If it differs, update the provider's URLs (step 2) and `NEXUS_ALLOWED_ORIGINS` (step 5).

**Only `main` is built.** `frontend/vercel.json` tells Vercel to skip every other branch (`ignoreCommand`), so pull requests and `dev` do not produce a failed or half-working preview. If you ever want previews, remove that line and read the next paragraph first.

**Preview deployments do not work for sign-in or saving.** Every Vercel preview gets its own address, such as `https://nexus-demo-git-some-branch-you.vercel.app`. That address is not in the provider's callback list, so sign-in is refused there, and it is not in `NEXUS_ALLOWED_ORIGINS`, so the API answers HTTP 400 to anything that saves. Use the production address. If you need one preview to work, add its exact origin to the provider's four URL fields and to `NEXUS_ALLOWED_ORIGINS` (comma-separated), and remove it afterwards.

### 8. Check it

1. Open `SITE`. You should see "Sign in to Nexus." and be sent to the provider.
2. Sign in as the administrator email you seeded. You should land in the workspace, with "Demonstration workspace · Synthetic data only" in the sidebar.
3. Sign in with an account that was not seeded. You should see "This account is not set up here."
4. `SITE/healthz` should answer through the rewrite, with `"database":"connected"` and both workers listed.
5. Set the request limits' proxy count (next section). Until then every caller shares one limit.

### 9. Request limits (you, once, after the first deploy)

The API refuses a client that sends too many requests (HTTP 429, "Too many requests. Wait a moment and try again."). It counts per client address, and behind proxies it has to be told how many of the addresses in `X-Forwarded-For` were written by proxies it can trust.

1. From a terminal, call the API **directly**, not through the site: `curl -s https://YOUR-API-HOST/api/auth/config > /dev/null`.
2. In the API's log on the host, find that request's line. It ends with `forwarded_hops=N`.
3. Set `NEXUS_TRUSTED_PROXIES` to that `N` (in `render.yaml`, by pull request, or in the host's environment) and redeploy.

Use the number from a direct call, not from a request that came through the site. The API's own address is public, so anyone can call it without passing through Vercel; counting Vercel as a trusted proxy would let such a caller put any address they like in the header and be counted under it. The cost of the safe setting is that people who come through the site are counted together, per Vercel address, which is why the blueprint raises the limits above the defaults. A deployment for real use should close the API's public address to everything but the site (a private network, or a secret header checked at the API) and then count per person.

## Before every later deploy

- **Take a database snapshot first.** Migrations run when the API starts and only go forward; there is no down-migration. Rolling back a release that changed the schema means restoring the snapshot, not redeploying the old image.
- **Deploy `main` only.** Changes reach `main` by a pull request from `dev` after its checks pass (`docs/development/git-workflow.md`).
- **Watch the first start.** The service log shows one line if a migration or a setting fails, and the health check keeps the previous version serving until the new one is ready.

## If something goes wrong

| What you see | Likely cause |
| --- | --- |
| The API exits at startup naming a variable | That variable is missing or malformed. The message says which. |
| "The identity provider reports its issuer as …" | `NEXUS_OIDC_ISSUER` differs from the provider's value, often by a trailing slash. |
| The provider shows a callback or redirect error | `SITE/` is not in Allowed Callback URLs, or the address differs (preview deployment, custom domain). |
| "The identity provider did not complete sign-in" | The provider refused the code exchange. Check Allowed Web Origins / CORS, and that the application is a single-page (public) application. |
| Vercel build fails with "Nexus cannot be deployed yet" | `frontend/vercel.json` still has a placeholder address, or `connect-src` is too wide. The log names each line (step 6). |
| "This account is not set up here" for someone who should get in | Their email is not a staff member, the staff member is inactive, the provider has not verified their email, or the same email exists twice. The page never says which; the API log does (below). |
| Everyone gets "This account is not set up here" | The provider is not sending `email_verified: true`, or the `email` scope is missing. The API log says so on every attempt. |
| Sign-in or saving fails on a preview deployment | Preview addresses are not the production address (step 7). |
| Saving fails with HTTP 400 while reading works | The site's origin is not in `NEXUS_ALLOWED_ORIGINS`. |
| "The Nexus service is not responding" | The API is down, or `vercel.json` still has the placeholder address. |
| "Too many requests. Wait a moment and try again." for everyone at once | `NEXUS_TRUSTED_PROXIES` is still 0, so all callers share one limit (step 9), or the limits are too low for the number of people in visits. |
| "The workspace is busy. Nothing was changed; try again in a moment." | A request waited 15 seconds for a lock another request held. Retrying is safe. If it persists, the API log shows which requests are slow (`duration_ms`). |
| A person reports an error and you need the matching log line | Every response carries an `X-Request-Id` header; the API log has the same `request_id`. |

## What the API logs when it refuses a sign-in

The person always sees the same generic message. The API log, on the container host, gives the cause, without the token or the email:

- `Sign-in refused for a correctly signed identity token: it has no email_verified claim; …`: the provider does not state that emails are verified. Choose a provider that does.
- `… its email_verified claim is not true; …`: that account has not verified its email at the provider.
- `… it has no email claim; …`: the `email` scope is missing from `NEXUS_OIDC_SCOPES`, or the provider does not release it.
- `Sign-in refused: the verified email matches no single active staff member`: add the person under **Team & access**, reactivate them, or remove a duplicate email.

Expired, malformed and wrongly signed tokens are not logged; the browser signs in again.

## Things not yet known

- **Request limits behind the real proxies.** On November 12, 2026 a direct call to the API logged `forwarded_hops=2`, and the blueprint sets `NEXUS_TRUSTED_PROXIES` to 2. Whether that number is stable over time, and that a caller over the limit actually receives a 429 through Render and Vercel, has not been tested.
- **Not deployed.** The Dockerfile is built by CI only; Docker is not installed on the development Mac. Vercel, a container host, managed PostgreSQL and a real identity provider have not been exercised.
- **Upload size through Vercel.** Recording chunks are up to 8 MB. Whether Vercel forwards request bodies that large to an external address was not tested. If uploads fail, set `VITE_API_BASE_URL` to `API` in Vercel so the browser calls the API directly, and add `API` as a second origin in `connect-src` in `frontend/vercel.json`; the API already allows `SITE` as an origin.
- **Sign-out.** If the provider publishes an end-session address the browser is sent there; otherwise only the Nexus session ends and the provider may sign the person straight back in.
- **Identity provider with a separate token host.** `connect-src` in `frontend/vercel.json` must name the origin of the provider's **token endpoint**. For Auth0 that is the tenant's own address. For a provider that serves tokens from a different host than its login page, use the token endpoint's origin.
- **Local tooling.** `tools/mock-oidc-provider.mjs` signs a token for any email typed into it. It listens on loopback only and exists to try hosted mode on your own machine. Never configure a hosted API to trust it.

## Optional features when hosted

| Feature | To enable | Notes |
| --- | --- | --- |
| Visit recording | Set `NEXUS_MEDIA_KEY_HEX` to a new key from `openssl rand -hex 32` | Store it as a host secret, not in the database. Do not reuse the local key. Losing it makes recordings unreadable. |
| Calls across networks | Set `NEXUS_ICE_SERVERS_JSON` with a TURN server | No TURN service is included. |
| Speech and drafting | Not available | They run local models that are not in the image. Their endpoints answer "not configured". |

## Trying hosted mode on your own machine

```bash
node tools/mock-oidc-provider.mjs --port 9400
```

```bash
NEXUS_AUTH_MODE=oidc PORT=8095 DATABASE_URL='postgres://…/a_disposable_database' NEXUS_OIDC_ISSUER=http://127.0.0.1:9400 NEXUS_OIDC_AUDIENCE=nexus-local-demo NEXUS_OIDC_CLIENT_ID=nexus-local-demo NEXUS_ALLOWED_ORIGINS=http://127.0.0.1:5175 cargo run -p emr-api
```

```bash
cd frontend && NEXUS_API_PROXY=http://127.0.0.1:8095 npx vite --host 127.0.0.1 --port 5175
```

Use a disposable database, never `nexus` or `nexus_walkthrough`.
