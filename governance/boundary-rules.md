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

## Extension checklists

Content-only addition:

- change a downstream TypeScript action or pure composition helper;
- update its downstream owner-local normalization expectation;
- regenerate the normalized RPG IR artifact;
- confirm no Rust, product protocol, host route, capability manifest, or
  certification/proof manifest changed.

New semantic operation:

- add or version the normalized IR declaration and strict decoder;
- register Rust reads, mutation owner, validation behavior, accepted
  DomainEvents, trace behavior, and replay implications;
- implement reference/requirement/semantic validation and staged execution;
- add Rust owner tests for acceptance, rejection atomicity, event, trace,
  randomness, final view, and replay implications;
- regenerate the public operation vocabulary;
- only then add TypeScript authoring sugar and type/normalization tests.

`npm run report:amplification` validates and reports these two change paths.
Exhaustive cross-product proof belongs downstream in `asha-rulebench-testing`.
