import assert from 'node:assert/strict';
import test from 'node:test';

import { inspectPortableRustSource } from './check-governance.mjs';

test('rejects Rust portable source that imports product host and proof owners', () => {
  const source = `
    use rulebench_process_host::Router;
    use rulebench_fixtures::goldens;
  `;

  assert.ok(inspectPortableRustSource(source).length > 0);
});

test('accepts Rust portable source that names only RPG-domain owners', () => {
  const source = `
    use rpg_core::{RpgDomainEvent, RpgIntent};
    use rpg_ir::NormalizedRpgIr;
  `;

  assert.deepEqual(inspectPortableRustSource(source), []);
});
