import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

import {
  action,
  actionId,
  canonicalJson,
  composeRuleset,
  constant,
  damage,
  damageType,
  defineActionDefinition,
  defineRulesetPackage,
  defineSupportDefinition,
  defineTemplateDefinition,
  definitionReference,
  hostile,
  noRoll,
  onCheck,
  prepareRulesetCompilation,
  rulesetDependency,
  rulesetPackageRequest,
  rulesetPackageSource,
} from '@asha-rpg/authoring';
import type {
  PreparedRulesetCompilation,
  RulesetPackageManifest,
  RulesetPackageSource,
} from '@asha-rpg/authoring';

const root = fileURLToPath(new URL('../../../', import.meta.url));

test('explicit package composition is immutable, closed, and load-order independent', () => {
  const fixture = packageFixture();
  const first = prepareRulesetCompilation({
    composition: fixture.composition,
    packages: fixture.packages,
  });
  const second = prepareRulesetCompilation({
    composition: fixture.composition,
    packages: [...fixture.packages].reverse(),
  });

  assert.equal(first.ok, true);
  assert.equal(second.ok, true);
  if (!first.ok || !second.ok) return;
  assert.equal(canonicalJson(first.prepared), canonicalJson(second.prepared));
  assert.equal(Object.isFrozen(first.prepared), true);
  assert.equal(Object.isFrozen(first.prepared.materializedDefinitions), true);
  assert.deepEqual(
    first.prepared.sourcePackages.map((entry) => `${entry.id}@${entry.version}`),
    ['sample.core@1.0.0', 'sample.foundation@1.1.0'],
  );
  assert.deepEqual(first.prepared.exportedRoots, [
    'catalog.damage.arcane',
    'sample.spark',
  ]);
  assert.deepEqual(
    first.prepared.materializedDefinitions.map((entry) => entry.id),
    ['catalog.damage.arcane', 'sample.spark'],
  );
  assert.equal(
    first.prepared.materializedDefinitions.some(
      (entry) => entry.id === 'sample.private-template',
    ),
    false,
  );
  assert.ok(
    first.prepared.relationships.some((entry) => entry.kind === 'dependsOn'),
  );
  assert.ok(
    first.prepared.relationships.some((entry) => entry.kind === 'contributes'),
  );
});

test('typed source diagnostics fail before materialization and retain full graph context', () => {
  const missing = prepareFixture({ referenceId: 'catalog.damage.missing' });
  assert.equal(missing.ok, false);
  if (missing.ok) return;
  assert.ok(
    missing.diagnostics.some(
      (entry) => entry.code === 'RULESET_DEFINITION_REFERENCE_MISSING',
    ),
  );

  const unreachable = prepareFixture({ unreachableVisibility: 'public' });
  assert.equal(unreachable.ok, false);
  if (unreachable.ok) return;
  assert.ok(
    unreachable.diagnostics.some(
      (entry) => entry.code === 'RULESET_PUBLIC_DEFINITION_UNREACHABLE',
    ),
  );

  const incompatible = prepareFixture({ languageVersion: '^2.0.0' });
  assert.equal(incompatible.ok, false);
  if (incompatible.ok) return;
  assert.ok(
    incompatible.diagnostics.some(
      (entry) => entry.code === 'RULESET_LANGUAGE_INCOMPATIBLE',
    ),
  );

  const cycle = prepareFixture({ dependencyCycle: true });
  assert.equal(cycle.ok, false);
  if (cycle.ok) return;
  const cycleDiagnostic = cycle.diagnostics.find(
    (entry) => entry.code === 'RULESET_DEPENDENCY_CYCLE',
  );
  assert.deepEqual(cycleDiagnostic?.graphPath, [
    'sample.core@1.0.0',
    'sample.foundation@1.1.0',
    'sample.core@1.0.0',
  ]);

  const privateReference = prepareFixture({ referencePrivateDefinition: true });
  assert.equal(privateReference.ok, false);
  if (privateReference.ok) return;
  assert.ok(
    privateReference.diagnostics.some(
      (entry) => entry.code === 'RULESET_PRIVATE_CROSS_PACKAGE_REFERENCE',
    ),
  );
});

test('duplicate identities and unresolved relational execution fail closed', () => {
  const fixture = packageFixture();
  const duplicate = prepareRulesetCompilation({
    composition: fixture.composition,
    packages: [fixture.packages[0]!, fixture.packages[0]!, fixture.packages[1]!],
  });
  assert.equal(duplicate.ok, false, JSON.stringify(duplicate));
  if (duplicate.ok) return;
  assert.ok(
    duplicate.diagnostics.some(
      (entry) => entry.code === 'RULESET_DUPLICATE_PACKAGE_IDENTITY',
    ),
  );

  const relationship = prepareFixture({ deferredRelationship: true });
  assert.equal(relationship.ok, false);
  if (relationship.ok) return;
  assert.ok(
    relationship.diagnostics.some(
      (entry) => entry.code === 'RULESET_RELATIONSHIP_EXECUTION_DEFERRED',
    ),
  );

  const duplicateAlias = prepareFixture({ duplicateDependencyAlias: true });
  assert.equal(duplicateAlias.ok, false);
  if (duplicateAlias.ok) return;
  assert.ok(
    duplicateAlias.diagnostics.some(
      (entry) => entry.code === 'RULESET_DUPLICATE_IMPORT_ALIAS',
    ),
  );

  const duplicateLocal = prepareFixture({ duplicateLocalDefinition: true });
  assert.equal(duplicateLocal.ok, false);
  if (duplicateLocal.ok) return;
  assert.ok(
    duplicateLocal.diagnostics.some(
      (entry) => entry.code === 'RULESET_DUPLICATE_LOCAL_DEFINITION',
    ),
  );

  const duplicateGlobal = prepareFixture({ duplicateGlobalDefinition: true });
  assert.equal(duplicateGlobal.ok, false);
  if (duplicateGlobal.ok) return;
  assert.ok(
    duplicateGlobal.diagnostics.some(
      (entry) => entry.code === 'RULESET_DUPLICATE_DEFINITION_ID',
    ),
  );
});

test('composition extensions remain explicit records and fail closed before #5957', () => {
  const fixture = packageFixture();
  const overlay = prepareRulesetCompilation({
    composition: composeRuleset({
      ...fixture.composition,
      overlays: [rulesetPackageRequest({ id: 'sample.foundation', version: '1.1.0' })],
    }),
    packages: fixture.packages,
  });
  assert.equal(overlay.ok, false);
  if (overlay.ok) return;
  assert.ok(
    overlay.diagnostics.some(
      (entry) => entry.code === 'RULESET_OVERLAY_EXECUTION_DEFERRED',
    ),
  );

  const configure = prepareRulesetCompilation({
    composition: composeRuleset({
      ...fixture.composition,
      configure: { 'sample.spark.damage': 7 },
    }),
    packages: fixture.packages,
  });
  assert.equal(configure.ok, false);
  if (configure.ok) return;
  assert.ok(
    configure.diagnostics.some(
      (entry) => entry.code === 'RULESET_CONFIGURATION_EXECUTION_DEFERRED',
    ),
  );
});

test('Rust emits byte-stable closed artifacts and separates fingerprint planes', () => {
  const baseline = acceptedPrepared(packageFixture());
  const repeated = compilePrepared(baseline);
  const repeatedAgain = compilePrepared(baseline);
  assert.deepEqual(repeated, repeatedAgain);

  const sourceOnly = compilePrepared(
    acceptedPrepared(packageFixture({ sourceModule: 'moved/core-ruleset.ts' })),
  );
  const presentationOnly = compilePrepared(
    acceptedPrepared(packageFixture({ label: 'Spark with a new label' })),
  );
  const semantic = compilePrepared(
    acceptedPrepared(packageFixture({ damageAmount: 7 })),
  );

  assert.notEqual(repeated.fingerprints.source, sourceOnly.fingerprints.source);
  assert.equal(repeated.fingerprints.semantic, sourceOnly.fingerprints.semantic);
  assert.equal(repeated.fingerprints.presentation, sourceOnly.fingerprints.presentation);

  assert.notEqual(repeated.fingerprints.source, presentationOnly.fingerprints.source);
  assert.equal(repeated.fingerprints.semantic, presentationOnly.fingerprints.semantic);
  assert.notEqual(
    repeated.fingerprints.presentation,
    presentationOnly.fingerprints.presentation,
  );

  assert.notEqual(repeated.fingerprints.source, semantic.fingerprints.source);
  assert.notEqual(repeated.fingerprints.semantic, semantic.fingerprints.semantic);
  assert.equal(repeated.fingerprints.presentation, semantic.fingerprints.presentation);

  const encoded = JSON.stringify(repeated);
  assert.equal(encoded.includes('privatePlan'), false);
  assert.equal(encoded.includes('compiledProgram'), false);
  assert.equal(encoded.includes('callback'), false);
  const validation = spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'validate_ruleset_artifact',
    ],
    { cwd: root, encoding: 'utf8', input: encoded },
  );
  assert.equal(validation.status, 0, validation.stderr);
  assert.match(validation.stdout, /^accepted sample\.composition@1\.0\.0:/);

  const tamperedValidation = spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'validate_ruleset_artifact',
    ],
    {
      cwd: root,
      encoding: 'utf8',
      input: JSON.stringify({
        ...repeated,
        artifactId: `${repeated.artifactId}:tampered`,
      }),
    },
  );
  assert.notEqual(tamperedValidation.status, 0);
  assert.match(tamperedValidation.stderr, /RULESET_ARTIFACT_FINGERPRINT_MISMATCH/);

  const unknownFieldValidation = spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'validate_ruleset_artifact',
    ],
    {
      cwd: root,
      encoding: 'utf8',
      input: JSON.stringify({ ...repeated, unexpectedRuntimeDependency: 'forbidden' }),
    },
  );
  assert.notEqual(unknownFieldValidation.status, 0);
  assert.match(unknownFieldValidation.stderr, /RULESET_ARTIFACT_DECODE_FAILED/);
});

function acceptedPrepared(
  fixture: ReturnType<typeof packageFixture>,
): PreparedRulesetCompilation {
  const result = prepareRulesetCompilation({
    composition: fixture.composition,
    packages: fixture.packages,
  });
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.prepared;
}

function prepareFixture(options: FixtureOptions) {
  const fixture = packageFixture(options);
  return prepareRulesetCompilation({
    composition: fixture.composition,
    packages: fixture.packages,
  });
}

function compilePrepared(prepared: PreparedRulesetCompilation): CompiledArtifact {
  const compilation = spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'compile_ruleset',
    ],
    { cwd: root, encoding: 'utf8', input: canonicalJson(prepared) },
  );
  assert.equal(compilation.status, 0, compilation.stderr);
  const result = JSON.parse(compilation.stdout) as CompilationEnvelope;
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.artifact;
}

interface FixtureOptions {
  readonly damageAmount?: number;
  readonly label?: string;
  readonly sourceModule?: string;
  readonly referenceId?: string;
  readonly referencePrivateDefinition?: boolean;
  readonly unreachableVisibility?: 'public' | 'private';
  readonly languageVersion?: string;
  readonly dependencyCycle?: boolean;
  readonly deferredRelationship?: boolean;
  readonly duplicateDependencyAlias?: boolean;
  readonly duplicateLocalDefinition?: boolean;
  readonly duplicateGlobalDefinition?: boolean;
}

function packageFixture(options: FixtureOptions = {}): {
  readonly composition: ReturnType<typeof composeRuleset>;
  readonly packages: readonly RulesetPackageSource[];
} {
  const privateTemplate = defineTemplateDefinition({
    kind: 'template',
    id: 'sample.private-template',
    visibility: options.unreachableVisibility ?? 'private',
    extensionPolicy: 'derivable',
    source: { module: 'foundation/templates.ts', declaration: 'privateTemplate' },
    references: [],
  });
  const arcane = defineSupportDefinition({
    kind: 'support',
    id: 'catalog.damage.arcane',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'foundation/damage-types.ts', declaration: 'arcane' },
    references: [],
    semantic: { catalog: 'damageType', id: 'arcane' },
  });
  const foundationManifest: RulesetPackageManifest = defineRulesetPackage({
    identity: { id: 'sample.foundation', version: '1.1.0' },
    entry: { module: 'foundation/ruleset.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: options.dependencyCycle
      ? [rulesetDependency({ id: 'sample.core', version: '^1.0.0', importAs: 'core' })]
      : [],
    requirements: { operations: [], capabilities: [] },
    definitions: [
      arcane,
      privateTemplate,
      ...(options.duplicateGlobalDefinition
        ? [
            defineSupportDefinition({
              kind: 'support',
              id: 'sample.spark',
              visibility: 'public',
              extensionPolicy: 'sealed',
              source: { module: 'foundation/conflict.ts', declaration: 'spark' },
              references: [],
              semantic: { catalog: 'damageType', id: 'spark' },
            }),
          ]
        : []),
    ],
    exports: options.referencePrivateDefinition
      ? ['catalog.damage.arcane', 'sample.private-template']
      : [
          'catalog.damage.arcane',
          ...(options.duplicateGlobalDefinition ? ['sample.spark'] : []),
        ],
    policyBindings: [],
    relationships: [],
  });

  const sparkAction = action({
    id: actionId('sample.spark'),
    name: 'Spark',
    sourcePath: 'core/actions/spark.ts',
    targets: hostile({ range: 4 }),
    check: noRoll(),
    program: onCheck({
      noRoll: damage({
        amount: constant(options.damageAmount ?? 5),
        type: damageType('arcane'),
      }),
    }),
  });
  const spark = defineActionDefinition({
    kind: 'action',
    id: 'sample.spark',
    visibility: 'public',
    extensionPolicy: 'patchable',
    source: {
      module: options.sourceModule ?? 'core/actions/spark.ts',
      declaration: 'spark',
    },
    references: [
      definitionReference({
        importAs: 'foundation',
        definitionId: options.referencePrivateDefinition
          ? 'sample.private-template'
          : options.referenceId ?? 'catalog.damage.arcane',
      }),
    ],
    presentation: { label: options.label ?? 'Spark' },
    action: sparkAction,
  });
  const coreManifest = defineRulesetPackage({
    identity: { id: 'sample.core', version: '1.0.0' },
    entry: { module: 'core/ruleset.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: options.languageVersion ?? '^1.0.0' },
    dependencies: [
      rulesetDependency({
        id: 'sample.foundation',
        version: '^1.0.0',
        importAs: 'foundation',
      }),
      ...(options.duplicateDependencyAlias
        ? [
            rulesetDependency({
              id: 'sample.foundation',
              version: '1.1.0',
              importAs: 'foundation',
            }),
          ]
        : []),
    ],
    requirements: {
      operations: [{ id: 'operation.damage', version: 1 }],
      capabilities: [{ id: 'capability.vitality', version: 1 }],
    },
    definitions: options.duplicateLocalDefinition ? [spark, spark] : [spark],
    exports: ['sample.spark'],
    policyBindings: [],
    relationships: options.deferredRelationship
      ? [
          {
            kind: 'derivesFrom',
            definitionId: 'sample.spark',
            target: definitionReference({
              importAs: 'foundation',
              definitionId: 'sample.private-template',
            }),
            version: 1,
          },
        ]
      : [],
  });
  return {
    composition: composeRuleset({
      identity: { id: 'sample.composition', version: '1.0.0' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      base: rulesetPackageRequest({ id: 'sample.core', version: '1.0.0' }),
      add: [rulesetPackageRequest({ id: 'sample.foundation', version: '^1.0.0' })],
      overlays: [],
      configure: {},
    }),
    packages: [rulesetPackageSource(coreManifest), rulesetPackageSource(foundationManifest)],
  };
}

interface CompiledArtifact {
  readonly artifactId: string;
  readonly fingerprints: {
    readonly source: string;
    readonly semantic: string;
    readonly presentation: string;
  };
}

type CompilationEnvelope =
  | { readonly ok: true; readonly artifact: CompiledArtifact; readonly diagnostics: readonly [] }
  | { readonly ok: false; readonly diagnostics: readonly unknown[] };
