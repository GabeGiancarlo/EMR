# Database strategy

PostgreSQL is the implemented persistence layer. SQLx migrations in root `migrations/` run on API startup. The local workspace uses PostgreSQL 17 through `compose.local.yml` or the ignored portable EDB installation. Schema changes ship with application changes and relevant regression tests.

The [data model](data-model.md) describes actual clinical, financial and operational relationships. Organization-qualified foreign keys prevent cross-tenant links. Amounts use integer cents. Signed originals and ledger entries have guarded correction paths. Transactions and row locks protect authorization reservations, payment limits and state transitions. Facility timezone defines a session's service date for coverage and authorization checks.

The ordinary `nexus` database starts empty. Backend tests use explicitly disposable `nexus_test`; browser fixtures use `nexus_ui_test`. Backup and restore helpers retain local artifacts under ignored `.local/backups`. Encrypted production backup, retention and recovery operations remain deployment work.
