import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const contract = JSON.parse(
  readFileSync(join(root, 'governance/change-amplification.json'), 'utf8'),
);

const failures = validateContract(contract);
if (failures.length > 0) {
  console.error(failures.join('\n'));
  process.exit(1);
}

console.log(
  `change amplification: content-only=${contract.contentOnly.requiredLayers.length} layers; semantic-operation=${contract.semanticOperation.requiredLayers.length} layers`,
);
console.log(
  `content-only forbidden=${contract.contentOnly.forbiddenLayers.join(', ')}`,
);

export function validateContract(value) {
  const errors = [];
  if (value.schema !== 'asha-rpg.change-amplification@1') {
    errors.push('change amplification contract has an unknown schema');
  }
  requireNonEmptyList(value.contentOnly?.requiredLayers, 'contentOnly.requiredLayers', errors);
  requireNonEmptyList(value.contentOnly?.forbiddenLayers, 'contentOnly.forbiddenLayers', errors);
  requireNonEmptyList(
    value.semanticOperation?.requiredLayers,
    'semanticOperation.requiredLayers',
    errors,
  );
  const contentText = (value.contentOnly?.requiredLayers ?? []).join(' ').toLowerCase();
  for (const forbidden of ['rust', 'protocol', 'host route', 'certification']) {
    if (contentText.includes(forbidden)) {
      errors.push(`content-only required layers contain forbidden ${forbidden} amplification`);
    }
  }
  if ((value.semanticOperation?.requiredLayers ?? []).length < 7) {
    errors.push('semantic operation path omits one or more mandatory owner layers');
  }
  return errors;
}

function requireNonEmptyList(value, path, errors) {
  if (
    !Array.isArray(value) ||
    value.length === 0 ||
    value.some((entry) => typeof entry !== 'string' || entry.trim() === '')
  ) {
    errors.push(`${path} must be a non-empty string list`);
  }
}
