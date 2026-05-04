# Coding Standards

## Rust Backend Expectations

- write idiomatic Rust with clear module boundaries
- prefer async-first APIs where I/O or concurrency is involved
- keep Actix handlers thin; move business logic into services
- keep data access logic in repositories
- keep domain concepts and reusable contracts in `core/`

## Error Handling

- use `Result` and `Option` deliberately
- use typed, structured errors (`thiserror`) for boundary-safe responses
- avoid `unwrap()` in request/runtime paths

## Data Contracts

- use `serde` DTOs for API boundaries
- avoid exposing internal models directly as external responses
- validate input at boundary layers and enforce core invariants in services

## Documentation And Maintainability

- add module docs (`//!`) where module intent is not obvious
- add `///` docs for public types and important functions
- document prototype/mock behavior clearly when present

## Testing

- prefer async tests (`tokio::test`) for async paths
- add unit tests for services and repositories as they are introduced
- add integration tests for route and persistence boundaries
