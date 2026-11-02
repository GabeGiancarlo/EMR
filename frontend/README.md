# Nexus workspace

Custom React and TypeScript interface for the local Nexus API. Runtime packages are React and React DOM. Vite and TypeScript are build tools; there is no UI component framework, chart library, CSS framework, remote font service, or third-party analytics client.

## Run locally

Start the database and Rust API using the repository's local setup instructions, then:

```powershell
cd C:\Users\ganeg\source\EMR\frontend
npm ci
npm run dev
```

The local interface listens on `http://127.0.0.1:5173`. Vite proxies `/api` and `/healthz` to `http://127.0.0.1:8090`. For an isolated test server, set `NEXUS_API_PROXY` before starting Vite:

```powershell
$env:NEXUS_API_PROXY = 'http://127.0.0.1:8091'
npm run dev -- --port 5174
```

`VITE_API_BASE_URL` optionally selects a separate API origin at build time. It is public configuration, never a place for a secret. The current API intentionally refuses public deployment; hosting the static interface alone does not create a deployable EMR.

## Data and access

First-run setup creates only the organization, facility, and administrator entered by the operator. No sample clients, notes, claims, payments, or outcomes are seeded. All clinical and operational records are fetched from the API and persisted by the server. Empty states show zero counts and guide the next real action.

Authentication is deferred. Local operator selection passes a staff identifier to the server so its resource policies can be exercised. It is not proof of identity. Local storage contains only the selected operator identifier; clinical records and imported transcripts are not cached there. The server's effective permissions determine whether write actions are available.

## Implemented interface paths

- Organization setup and local operator selection
- Overview with counts and balances derived from persisted records
- Client intake, client chart, care episodes, consent attestations, and episode discharge
- Individual sessions and shared group encounters with per-client attendance
- Treatment plans, measurable goals, plan signing, revisions, completion, and append-only reviews
- Clinical note drafts, signatures, and amendments
- User-authored form definitions and a note composer using those definitions
- Reviewed text import with explicit excerpt-to-field mapping; no speech recognition or automatic AI generation is claimed
- Coverage, authorizations, internal claim readiness, external-submission recording, denial recording, and payment entry
- Shared tasks, staff administration, offboarding with responsibility transfer, time-bound permission exceptions, facilities, and audit history

The Connections view explains unconfigured services. It does not pretend to transmit claims, verify eligibility, perform sign-in, sync accounting, or transcribe audio.

## Validation

```powershell
npm run typecheck
npm run build
```

The build checks the complete TypeScript application and emits static assets to `dist`. Browser workflow tests should use a separate database and API instance so test records do not enter the user's empty workspace. Keep an actual browser flow check alongside server tests: a successful build alone does not prove that forms, state transitions, or access policies work end to end.

## Source organization

- `App.tsx`: workspace shell, routing state, core record views, and mutation coordination
- `Setup.tsx`: first-run organization setup
- `NoteComposer.tsx`: note composition and human-reviewed source-text mapping
- `CarePlanning.tsx`: treatment plans, goals, revisions, and reviews
- `GroupEncounters.tsx`: group encounter roster and individual attendance
- `forms.tsx`: resource form definitions, reference choices, field builder, and input normalization
- `api.ts`: API contracts, requests, and display formatting
- `ui.tsx`: custom SVG icons and reusable accessible interface primitives
- `styles.css` and `workflows.css`: responsive Nexus visual system
