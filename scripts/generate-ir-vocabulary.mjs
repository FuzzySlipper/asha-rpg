import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

const root = fileURLToPath(new URL('../', import.meta.url));
const outputPath = join(root, 'packages/ir/src/generated-vocabulary.ts');
const result = spawnSync(
  'cargo',
  [
    'run',
    '--quiet',
    '--manifest-path',
    join(root, 'Cargo.toml'),
    '-p',
    'rpg-compiler',
    '--bin',
    'export_vocabulary',
  ],
  { cwd: root, encoding: 'utf8' },
);
if (result.status !== 0) {
  process.stderr.write(result.stderr);
  process.exit(result.status ?? 1);
}

const vocabulary = JSON.parse(result.stdout);
const operations = [...vocabulary.operations].sort(compareId);
const capabilities = [...vocabulary.capabilities].sort(compareId);
for (const operation of operations) validateOperationContract(operation);
const generated = `// Generated from the Rust semantic registry. Do not edit by hand.
export const RPG_IR_IDENTITY = ${JSON.stringify(vocabulary.identity)} as const;
export const RPG_IR_MAJOR = ${JSON.stringify(vocabulary.major)} as const;

export const RPG_OPERATION_VERSIONS = ${renderVersions(operations)} as const;
export type RpgOperationId = keyof typeof RPG_OPERATION_VERSIONS;

export const RPG_CAPABILITY_VERSIONS = ${renderVersions(capabilities)} as const;
export type RpgCapabilityId = keyof typeof RPG_CAPABILITY_VERSIONS;
`;

if (process.argv.includes('--check')) {
  if (!existsSync(outputPath) || readFileSync(outputPath, 'utf8') !== generated) {
    console.error('generated RPG IR vocabulary is stale; run npm run generate:ir');
    process.exit(1);
  }
  console.log('generated RPG IR vocabulary is current');
} else {
  writeFileSync(outputPath, generated);
  console.log('wrote packages/ir/src/generated-vocabulary.ts');
}

function compareId(left, right) {
  return compareText(left.id, right.id);
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function renderVersions(registrations) {
  const lines = registrations.map(
    (registration) => `  ${JSON.stringify(registration.id)}: ${registration.version},`,
  );
  return `{
${lines.join('\n')}
}`;
}

function validateOperationContract(operation) {
  const requiredText = [
    'id',
    'mutationOwner',
    'validationBehavior',
    'traceBehavior',
    'replayImplications',
  ];
  for (const field of requiredText) {
    if (typeof operation[field] !== 'string' || operation[field].trim() === '') {
      throw new Error(`operation registry entry is missing ${field}`);
    }
  }
  if (!Number.isInteger(operation.version) || operation.version < 1) {
    throw new Error(`operation ${operation.id} has no positive integer version`);
  }
  for (const field of ['reads', 'acceptedEvents']) {
    if (!Array.isArray(operation[field]) || operation[field].length === 0) {
      throw new Error(`operation ${operation.id} is missing ${field}`);
    }
  }
}
