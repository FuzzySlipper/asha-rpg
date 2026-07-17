# Boundary rules

1. Rust owns semantic meaning and state mutation.
2. TypeScript builders construct immutable data only.
3. Normalized RPG IR never contains callbacks, source code, host, storage,
   protocol, UI, fixture, golden, experiment, or certification data.
4. Every operation declares typed reads, mutation owner, validation, accepted
   DomainEvents, trace behavior, replay implications, and a version.
5. Capability stores are private. Consumers receive typed views.
6. Public ASHA dependencies use the exact governed revision and public package
   roots. Sibling paths and private crates are forbidden.
7. Consumer content stays downstream. New named content is not a semantic
   primitive merely because it is unusual.
8. Owner-local tests stay with implementation. Exhaustive cross-product proof
   belongs to `asha-rulebench-testing`.
9. Temporary migration adapters require a named consumer and deletion task.
10. Planned cells are not implementation claims.
