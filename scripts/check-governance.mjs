import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();
const requiredFiles = [
  'AGENTS.md',
  'README.md',
  'Cargo.toml',
  'docs/design.md',
  'docs/non-claims.md',
  'governance/architecture.md',
  'governance/ownership.toml',
  'governance/dependency-policy.toml',
  'governance/upstream-asha.toml',
  'governance/boundary-rules.md',
];

const failures = [];
for (const path of requiredFiles) {
  if (!existsSync(join(root, path))) failures.push(`missing required file: ${path}`);
}

const ownership = read('governance/ownership.toml');
for (const cell of ['rpg-core', 'rpg-ir', 'rpg-compiler', 'rpg-runtime', 'rpg-replay', 'asha-rpg', '@asha-rpg/ir', '@asha-rpg/authoring']) {
  if (!ownership.includes(cell)) failures.push(`ownership cell is missing: ${cell}`);
}

const upstream = read('governance/upstream-asha.toml');
if (!/revision = "[0-9a-f]{40}"/.test(upstream)) failures.push('upstream ASHA revision must be an exact 40-character SHA');
if (upstream.includes('path =')) failures.push('upstream ASHA policy may not use sibling path dependencies');

for (const sourceRoot of ['crates', 'packages']) {
  const absolute = join(root, sourceRoot);
  if (!existsSync(absolute)) continue;
  for (const path of filesBelow(absolute)) {
    const source = readFileSync(path, 'utf8');
    if (/rulebench|certification|golden|experiment|angular/i.test(source)) {
      failures.push(`portable source contains product/proof vocabulary: ${path.slice(root.length + 1)}`);
    }
  }
}

if (failures.length > 0) {
  console.error(failures.join('\n'));
  process.exit(1);
}
console.log('asha-rpg governance check ok');

function read(path) {
  return readFileSync(join(root, path), 'utf8');
}

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}
