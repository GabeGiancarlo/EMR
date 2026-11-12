# Security

Nexus handles clinical and billing records, so security reports are taken seriously and
handled privately.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting for this repository: open the **Security**
tab, then **Report a vulnerability**. Do not open a public issue or pull request for a
suspected vulnerability, and do not include real patient information in a report;
describe the problem with synthetic data.

Please include what you did, what happened, what you expected, and the commit or
deployment you tested. You should hear back within five working days.

## Scope

- The API in `api/` and the web application in `frontend/`, at the current `main`.
- The hosted configuration described in `docs/deployment/demo-hosting.md`.

Local development mode (`NEXUS_AUTH_MODE=local`) has no authentication by design and
only listens on a loopback address; that is not a vulnerability.

## What runs automatically

`docs/development/security-checks.md` lists the checks that run on every change: secret
scanning, dependency advisories and licences, static analysis, and an unsafe-code
inventory. Those checks are aids, not an assessment; no independent security test has
been performed.
