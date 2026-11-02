# Frontend architecture

The active `frontend/` workspace uses React, TypeScript and Vite. Interface primitives, SVG icons, forms, tables and responsive styling are custom repository code. Runtime dependencies are React and React DOM; no component framework, remote fonts or analytics service is required.

The workspace connects onboarding, intake, client charts, care plans, individual/group sessions, documentation, forms, billing, tasks, team access and settings. Empty states guide real record creation. Totals and effective permissions come from the API, which independently enforces access.

Warm neutral surfaces, green accents and readable typography support practical work areas. Tables scroll within their containers on small screens and modals use native dialogs. Local storage contains only the selected development operator identifier, not clinical records or imported text.

See [the frontend README](../../frontend/README.md) for setup and source organization and [verification](../development/verification.md) for browser evidence. Authentication and public deployment remain deferred.
