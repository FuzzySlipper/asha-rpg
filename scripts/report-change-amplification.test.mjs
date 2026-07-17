import assert from 'node:assert/strict';
import test from 'node:test';

import { validateContract } from './report-change-amplification.mjs';

test('rejects a content-only contract that requires Rust amplification', () => {
  const contract = {
    schema: 'asha-rpg.change-amplification@1',
    contentOnly: {
      requiredLayers: ['consumer TypeScript', 'Rust adapter'],
      forbiddenLayers: ['Rust source'],
    },
    semanticOperation: {
      requiredLayers: ['1', '2', '3', '4', '5', '6', '7'],
    },
  };

  assert.ok(
    validateContract(contract).some((entry) => entry.includes('forbidden rust')),
  );
});
