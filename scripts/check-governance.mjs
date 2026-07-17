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
    if (/rulebench|certification|golden|experiment|\bangular\b/i.test(source)) {
      failures.push(`portable source contains product/proof vocabulary: ${path.slice(root.length + 1)}`);
    }
  }
}

checkPackageImports('packages/ir/src', (specifier) => specifier.startsWith('./'));
checkPackageImports(
  'packages/authoring/src',
  (specifier) => specifier === '@asha-rpg/ir' || specifier.startsWith('./'),
);

const irPackage = JSON.parse(read('packages/ir/package.json'));
const authoringPackage = JSON.parse(read('packages/authoring/package.json'));
if (irPackage.private === true || authoringPackage.private === true) {
  failures.push('supported TypeScript packages may not be marked private');
}
if (Object.keys(irPackage.dependencies ?? {}).length !== 0) {
  failures.push('@asha-rpg/ir may not have runtime dependencies');
}
const authoringDependencies = Object.keys(authoringPackage.dependencies ?? {});
if (
  authoringDependencies.length !== 1 ||
  authoringDependencies[0] !== '@asha-rpg/ir'
) {
  failures.push('@asha-rpg/authoring may depend only on @asha-rpg/ir at runtime');
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

function checkPackageImports(relativeRoot, allowed) {
  const absolute = join(root, relativeRoot);
  for (const path of filesBelow(absolute).filter((entry) => entry.endsWith('.ts'))) {
    const source = readFileSync(path, 'utf8');
    const imports = source.matchAll(/(?:from\s+|import\s*\()\s*['"]([^'"]+)['"]/g);
    for (const match of imports) {
      const specifier = match[1];
      if (specifier !== undefined && !allowed(specifier)) {
        failures.push(
          `portable package import is outside its allowlist: ${path.slice(root.length + 1)} -> ${specifier}`,
        );
      }
    }
  }
}
