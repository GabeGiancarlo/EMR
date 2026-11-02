# Local setup

Use the [root README](../../README.md) for complete commands. The checkout is `C:\Users\ganeg\source\EMR`.

```powershell
cd C:\Users\ganeg\source\EMR
.\tools\start-database.ps1
.\tools\start-api.ps1
```

In a second terminal:

```powershell
cd C:\Users\ganeg\source\EMR\frontend
npm ci
npm run dev
```

The interface runs at `http://127.0.0.1:5173`, the API at `http://127.0.0.1:8090`, and PostgreSQL on port 55432. `/healthz` checks database connectivity; `/api/bootstrap` reports whether setup is required. Setup creates only the organization, facility and administrator entered by the operator.

Authentication remains deferred. The helper enables loopback-only development and uses a copied executable so Windows does not lock Cargo's build output. The API applies migrations automatically. Never put real patient data in this mode.

For isolated browser checks use `nexus_ui_test`, API port 8091 and Vite port 5174 with `NEXUS_API_PROXY` targeting 8091. Backend tests use only `nexus_test`. See [verification](verification.md), [delivery plan](delivery-plan.md) and [xAI workers](xai-workers.md).
