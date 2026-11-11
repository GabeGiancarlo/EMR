# Hosting the synthetic demonstration

Written November 11, 2026. Nothing described here has been deployed. The steps were rehearsed on one Mac against a throwaway database and a mock identity provider; see [verification](../development/verification.md) for exactly what ran. Provider dashboards change, so treat the click paths as a guide and the values as the contract.

## What this is, and is not

This hosts Nexus as a **demonstration that contains synthetic records only, behind real sign-in**.

- **Do not enter information about real people.** No real patient, staff, payer or clinical data. The hosted configuration has not had a security review, penetration test, HIPAA risk analysis or business associate agreements, and it has no rate limiting, intrusion detection, backup policy or key-recovery plan.
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

- **Identity provider: Auth0.** It is plain OpenID Connect, has a free tier, supports a single-page application with PKCE and no secret, and lets you turn public sign-up off. Any provider works if it offers: a public client with PKCE, a discovery document, browser (CORS) access to its token endpoint for your site, and `email` plus `email_verified` in the ID token. Clerk, Okta, Microsoft Entra, Google and Keycloak are candidates; only the mock provider has been tested.
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

Optional: to let long sessions renew without a redirect, enable refresh token rotation for the application and add `offline_access` to `NEXUS_OIDC_SCOPES`. Without it, a session lasts as long as the provider's ID token (Auth0 default: 10 hours) and then shows "Your session ended".

### 3. Create the database (you)

Create a managed PostgreSQL 17 database. Copy its connection string and make sure it ends with `?sslmode=require` (or the host's equivalent). This string is a secret.

### 4. Seed synthetic data (you, on your Mac)

Seeding runs through a local-mode API on your own machine that is pointed at the hosted database, because it acts as the workspace administrator before anyone can sign in. From the repository root:

```bash
NEXUS_LOCAL_DEV=1 PORT=8095 DATABASE_URL='PASTE-THE-HOSTED-CONNECTION-STRING' cargo run -p emr-api
```

In a second terminal, using the emails of the provider accounts that should be able to sign in:

```bash
python3 tools/seed-demo.py --api http://127.0.0.1:8095 --admin-email you@example.com --staff clinician=someone@example.com --confirm-synthetic-demo
```

Then stop the API with Ctrl-C. Notes:

- The script creates one organization, six staff, three synthetic clients and example scheduling, documentation and billing records. It never deletes anything and refuses a database that already holds a different workspace.
- Roles you do not name (`clinical_lead`, `clinician`, `billing`, `executive`, `support`) get placeholder emails that cannot sign in. To try another role, pass `--staff role=email` with a second provider account, or add staff later under **Team & access** as the administrator.
- Each email can belong to only one staff member across the whole database.
- If your host gives the database an internal and an external address, this step needs the external one; the hosted API (step 5) can use the internal one.

### 5. Deploy the API (you)

1. Create a web service from the GitHub repository `GabeGiancarlo/EMR`, runtime Docker, Dockerfile path `infra/dockerfiles/Dockerfile.api`, build context the repository root.
2. Health check path: `/healthz`.
3. Environment variables (see `.env.example` for all of them):

| Variable | Value |
| --- | --- |
| `DATABASE_URL` | The connection string from step 3. Secret. |
| `NEXUS_OIDC_ISSUER` | The issuer from step 2, exactly. |
| `NEXUS_OIDC_CLIENT_ID` | The client ID from step 2. |
| `NEXUS_OIDC_AUDIENCE` | The same client ID. |
| `NEXUS_ALLOWED_ORIGINS` | `SITE` |

The image already sets `NEXUS_AUTH_MODE=oidc` and `HOST=0.0.0.0`. Do not set `NEXUS_LOCAL_DEV`. If the host assigns `PORT`, leave it alone; otherwise the API listens on 8090.

If something is missing the service exits at once with one line naming it, for example `Nexus API did not start: Hosted mode (NEXUS_AUTH_MODE=oidc) cannot start. Missing: NEXUS_OIDC_ISSUER.` Note the service's public address, for example `https://nexus-api.onrender.com`. Call it `API`. Opening `API/healthz` should show `"authentication":"oidc"`.

### 6. Point the frontend at the API (me, once you send the address)

`frontend/vercel.json` contains the placeholder `https://REPLACE-WITH-API-HOST.example.invalid` twice. Replace both with `API`. This is a normal change: a `feature/` branch and a pull request into `dev`. Send me the address and I will make it.

### 7. Deploy the frontend (you)

1. In Vercel (`gabegiancarlogk-8268s-projects`), **Add New → Project**, import `GabeGiancarlo/EMR`.
2. Root Directory: `frontend`. The framework, build command and output directory come from `vercel.json`.
3. No environment variables are needed. Leave `VITE_API_BASE_URL` unset.
4. Production Branch: `main`. The work reaches `main` by a pull request from `dev`; until then, set the production branch to `dev` if you want to try it before releasing.
5. Check the project's address equals `SITE`. If it differs, update the provider's URLs (step 2) and `NEXUS_ALLOWED_ORIGINS` (step 5).

Preview deployments get a different address each time, so sign-in only works on the production address unless you add each preview origin to both places.

### 8. Check it

1. Open `SITE`. You should see "Sign in to Nexus." and be sent to the provider.
2. Sign in as the administrator email you seeded. You should land in the workspace, with "Demonstration workspace · Synthetic data only" in the sidebar.
3. Sign in with an account that was not seeded. You should see "This account is not set up here."
4. `SITE/healthz` should answer through the rewrite.

## If something goes wrong

| What you see | Likely cause |
| --- | --- |
| The API exits at startup naming a variable | That variable is missing or malformed. The message says which. |
| "The identity provider reports its issuer as …" | `NEXUS_OIDC_ISSUER` differs from the provider's value, often by a trailing slash. |
| The provider shows a callback or redirect error | `SITE/` is not in Allowed Callback URLs, or the address differs (preview deployment, custom domain). |
| "The identity provider did not complete sign-in" | The provider refused the code exchange. Check Allowed Web Origins / CORS, and that the application is a single-page (public) application. |
| "This account is not set up here" for someone who should get in | Their email is not a staff member, the staff member is inactive, the provider has not verified their email, or the same email exists twice. |
| Saving fails with HTTP 400 while reading works | The site's origin is not in `NEXUS_ALLOWED_ORIGINS`. |
| "The Nexus service is not responding" | The API is down, or `vercel.json` still has the placeholder address. |

## Things not yet known

- **Not deployed.** The Dockerfile is built by CI only; Docker is not installed on the development Mac. Vercel, a container host, managed PostgreSQL and a real identity provider have not been exercised.
- **Upload size through Vercel.** Recording chunks are up to 8 MB. Whether Vercel forwards request bodies that large to an external address was not tested. If uploads fail, set `VITE_API_BASE_URL` to `API` in Vercel so the browser calls the API directly; the API already allows `SITE` as an origin.
- **Sign-out.** If the provider publishes an end-session address the browser is sent there; otherwise only the Nexus session ends and the provider may sign the person straight back in.
- **Abuse limits.** There is no request rate limiting. The API refuses unauthenticated requests cheaply, but the address is public.
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
