import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

import {
  action,
  actionId,
  canonicalJson,
  composePlayBundle,
  constant,
  damage,
  defineActionDefinition,
  defineContentCatalog,
  defineContentPack,
  defineSupportDefinition,
  defineTemplateDefinition,
  definitionReference,
  hostile,
  noRoll,
  onCheck,
  preparePlayBundle,
  contentPackDependency,
  contentPackRequest,
  contentPackSource,
  withLowLevelDefinitionReferences,
} from '@asha-rpg/authoring';
import { lowLevelCatalogReference } from '@asha-rpg/authoring/low-level';
import type {
  PreparedPlayBundle,
  ContentPackManifest,
  ContentPackSource,
} from '@asha-rpg/authoring';
import { contractTestRuleset } from './test-ruleset.ts';

const root = fileURLToPath(new URL('../../../', import.meta.url));

test('explicit package bundle is immutable, closed, and load-order independent', () => {
  const fixture = packageFixture();
  const first = preparePlayBundle({
    bundle: fixture.bundle,
    contentPacks: fixture.contentPacks,
  });
  const second = preparePlayBundle({
    bundle: fixture.bundle,
    contentPacks: [...fixture.contentPacks].reverse(),
  });

  assert.equal(first.ok, true);
  assert.equal(second.ok, true);
  if (!first.ok || !second.ok) return;
  assert.equal(canonicalJson(first.prepared), canonicalJson(second.prepared));
  assert.equal(Object.isFrozen(first.prepared), true);
  assert.equal(Object.isFrozen(first.prepared.materializedDefinitions), true);
  assert.deepEqual(
    first.prepared.contentPacks.map((entry) => `${entry.id}@${entry.version}`),
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
      (entry) =>
        entry.code === 'CONTENT_PACK_DEFINITION_REFERENCE_MISSING' &&
        entry.packageId === 'sample.core' &&
        entry.definitionId === 'sample.spark' &&
        entry.source?.module === 'core/actions/spark.ts',
    ),
  );

  const unreachable = prepareFixture({ unreachableVisibility: 'public' });
  assert.equal(unreachable.ok, false);
  if (unreachable.ok) return;
  assert.ok(
    unreachable.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_PUBLIC_DEFINITION_UNREACHABLE',
    ),
  );

  const incompatible = prepareFixture({ languageVersion: '^2.0.0' });
  assert.equal(incompatible.ok, false);
  if (incompatible.ok) return;
  assert.ok(
    incompatible.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_LANGUAGE_INCOMPATIBLE',
    ),
  );

  const cycle = prepareFixture({ dependencyCycle: true });
  assert.equal(cycle.ok, false);
  if (cycle.ok) return;
  const cycleDiagnostic = cycle.diagnostics.find(
    (entry) => entry.code === 'CONTENT_PACK_DEPENDENCY_CYCLE',
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
      (entry) => entry.code === 'CONTENT_PACK_PRIVATE_CROSS_PACKAGE_REFERENCE',
    ),
  );
});

test('duplicate identities and incompatible relational declarations fail closed', () => {
  const fixture = packageFixture();
  const duplicate = preparePlayBundle({
    bundle: fixture.bundle,
    contentPacks: [fixture.contentPacks[0]!, fixture.contentPacks[0]!, fixture.contentPacks[1]!],
  });
  assert.equal(duplicate.ok, false, JSON.stringify(duplicate));
  if (duplicate.ok) return;
  assert.ok(
    duplicate.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_DUPLICATE_PACKAGE_IDENTITY',
    ),
  );

  const relationship = prepareFixture({ deferredRelationship: true });
  assert.equal(relationship.ok, false);
  if (relationship.ok) return;
  assert.ok(
    relationship.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_DERIVATION_DECLARATION_INCOMPATIBLE',
    ),
  );

  const duplicateAlias = prepareFixture({ duplicateDependencyAlias: true });
  assert.equal(duplicateAlias.ok, false);
  if (duplicateAlias.ok) return;
  assert.ok(
    duplicateAlias.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_DUPLICATE_IMPORT_ALIAS',
    ),
  );

  const duplicateLocal = prepareFixture({ duplicateLocalDefinition: true });
  assert.equal(duplicateLocal.ok, false);
  if (duplicateLocal.ok) return;
  assert.ok(
    duplicateLocal.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_DUPLICATE_LOCAL_DEFINITION',
    ),
  );

  const duplicateGlobal = prepareFixture({ duplicateGlobalDefinition: true });
  assert.equal(duplicateGlobal.ok, false);
  if (duplicateGlobal.ok) return;
  assert.ok(
    duplicateGlobal.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_DUPLICATE_DEFINITION_ID',
    ),
  );
});

test('bundle extensions require explicit materialization records', () => {
  const fixture = packageFixture();
  const overlay = preparePlayBundle({
    bundle: composePlayBundle({
      ...fixture.bundle,
      overlays: [contentPackRequest({ id: 'sample.foundation', version: '1.1.0' })],
    }),
    contentPacks: fixture.contentPacks,
  });
  assert.equal(overlay.ok, false);
  if (overlay.ok) return;
  assert.ok(
    overlay.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_OVERLAY_EMPTY',
    ),
  );

  const configure = preparePlayBundle({
    bundle: composePlayBundle({
      ...fixture.bundle,
      configure: { 'sample.spark.damage': 7 },
    }),
    contentPacks: fixture.contentPacks,
  });
  assert.equal(configure.ok, false);
  if (configure.ok) return;
  assert.ok(
    configure.diagnostics.some(
      (entry) => entry.code === 'CONTENT_PACK_CONFIGURATION_OPTION_UNAVAILABLE',
    ),
  );
});

test('package resolution selects one version satisfying the complete constraint graph', () => {
  const consumers = [
    packageSourceWithDependency('sample.consumer-a', '^1.0.0'),
    packageSourceWithDependency('sample.consumer-b', '~1.1.0'),
  ];
  const available = [
    emptyPackageSource('sample.shared', '1.1.0'),
    emptyPackageSource('sample.shared', '1.2.0'),
  ];
  const playBundle = composePlayBundle({
    identity: { id: 'sample.intersection', version: '1.0.0' },
    ruleset: contractTestRuleset,
    base: contentPackRequest({ id: 'sample.consumer-a', version: '1.0.0' }),
    add: [contentPackRequest({ id: 'sample.consumer-b', version: '1.0.0' })],
    overlays: [],
    configure: {},
  });

  const first = preparePlayBundle({
    bundle: playBundle,
    contentPacks: [...consumers, ...available],
  });
  const reordered = preparePlayBundle({
    bundle: playBundle,
    contentPacks: [...available].reverse().concat([...consumers].reverse()),
  });

  assert.equal(first.ok, true, JSON.stringify(first));
  assert.equal(reordered.ok, true, JSON.stringify(reordered));
  if (!first.ok || !reordered.ok) return;
  const sharedLocks = first.prepared.dependencyLock.filter(
    (entry) => entry.packageId === 'sample.shared',
  );
  assert.equal(sharedLocks.length, 2);
  assert.deepEqual(
    sharedLocks.map((entry) => entry.resolvedVersion),
    ['1.1.0', '1.1.0'],
  );
  assert.equal(canonicalJson(first.prepared), canonicalJson(reordered.prepared));
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
  const semanticVariantPrepared: PreparedPlayBundle = {
    ...baseline,
    materializedDefinitions: baseline.materializedDefinitions.map((definition) =>
      definition.id === 'catalog.damage.arcane'
        ? { ...definition, semantic: { catalog: 'damageType', id: 'shadow' } }
        : definition,
    ),
  };
  const catalogSemanticDiagnostics = failedCompilationDiagnostics(
    runCompilation(semanticVariantPrepared),
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

  assert.match(
    catalogSemanticDiagnostics,
    /CONTENT_PACK_DEFINITION_FINGERPRINT_MISMATCH/,
  );

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
      'validate_play_bundle',
    ],
    { cwd: root, encoding: 'utf8', input: encoded },
  );
  assert.equal(validation.status, 0, validation.stderr);
  assert.match(validation.stdout, /^accepted sample\.bundle@1\.0\.0:/);

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
      'validate_play_bundle',
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
  assert.match(tamperedValidation.stderr, /PLAY_BUNDLE_ARTIFACT_FINGERPRINT_MISMATCH/);

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
      'validate_play_bundle',
    ],
    {
      cwd: root,
      encoding: 'utf8',
      input: JSON.stringify({ ...repeated, unexpectedRuntimeDependency: 'forbidden' }),
    },
  );
  assert.notEqual(unknownFieldValidation.status, 0);
  assert.match(unknownFieldValidation.stderr, /PLAY_BUNDLE_ARTIFACT_DECODE_FAILED/);

  const semanticTamper = structuredClone(repeated);
  const supportDefinition = semanticTamper.materializedDefinitions.find(
    (definition) => definition.id === 'catalog.damage.arcane',
  );
  assert.ok(supportDefinition);
  supportDefinition.semantic = { catalog: 'damageType', id: 'shadow' };
  const semanticTamperValidation = validateArtifact(semanticTamper);
  assert.notEqual(semanticTamperValidation.status, 0);
  assert.match(
    semanticTamperValidation.stderr,
    /CONTENT_PACK_DEFINITION_FINGERPRINT_MISMATCH/,
  );
});

test('the closed definition graph is derived from runtime semantics only', () => {
  const baseline = acceptedPrepared(packageFixture());
  const missingSupport = {
    ...baseline,
    exportedRoots: baseline.exportedRoots.filter(
      (definitionId) => definitionId !== 'catalog.damage.arcane',
    ),
    materializedDefinitions: baseline.materializedDefinitions.filter(
      (definition) => definition.id !== 'catalog.damage.arcane',
    ),
    definitionProvenance: baseline.definitionProvenance.filter(
      (entry) => entry.definitionId !== 'catalog.damage.arcane',
    ),
  };
  const missingSupportCompilation = runCompilation(missingSupport);
  const missingSupportDiagnostics = failedCompilationDiagnostics(
    missingSupportCompilation,
  );
  assert.match(
    missingSupportDiagnostics,
    /CONTENT_PACK_ARTIFACT_REFERENCE_MISSING|CONTENT_PACK_DAMAGE_TYPE_DEFINITION_MISSING/,
  );

  const undeclaredFixture = packageFixture({
    runtimeDamageDefinitionId: 'catalog.damage.shadow',
  });
  const undeclaredRuntimeType = preparePlayBundle({
    bundle: undeclaredFixture.bundle,
    contentPacks: undeclaredFixture.contentPacks,
  });
  assert.equal(undeclaredRuntimeType.ok, false);
  if (!undeclaredRuntimeType.ok) {
    assert.ok(
      undeclaredRuntimeType.diagnostics.some(
        (diagnostic) =>
          diagnostic.code === 'CONTENT_PACK_DEFINITION_REFERENCE_MISSING',
      ),
    );
  }

  const parallelRuntimeStructure = {
    ...baseline,
    normalizedIr: { actions: [] },
  };
  const parallelStructureCompilation = runCompilation(parallelRuntimeStructure);
  const parallelStructureDiagnostics = failedCompilationDiagnostics(
    parallelStructureCompilation,
  );
  assert.match(
    parallelStructureDiagnostics,
    /PLAY_BUNDLE_PREPARED_DECODE_FAILED/,
  );
});

test('Rust authority rejects unsupported requirements during compile and artifact load', () => {
  const baseline = acceptedPrepared(packageFixture());
  const unsupportedPrepared = {
    ...baseline,
    contentRequirements: {
      ...baseline.contentRequirements,
      operations: [{ id: 'operation.not-supported', version: 99 }],
    },
  };
  const compilation = runCompilation(unsupportedPrepared);
  assert.match(
    failedCompilationDiagnostics(compilation),
    /PLAY_BUNDLE_OPERATION_REQUIREMENT_MISSING/,
  );

  const artifact = structuredClone(compilePrepared(baseline));
  artifact.contentRequirements.operations = [
    { id: 'operation.not-supported', version: 99 },
  ];
  const validation = validateArtifact(artifact);
  assert.notEqual(validation.status, 0);
  assert.match(validation.stderr, /PLAY_BUNDLE_OPERATION_REQUIREMENT_MISSING/);

  const unsupportedModel = {
    ...baseline,
    ruleset: {
      ...baseline.ruleset,
      models: {
        ...baseline.ruleset.models,
        checks: { id: 'check.not-supported', version: 99 },
      },
    },
  };
  assert.match(
    failedCompilationDiagnostics(runCompilation(unsupportedModel)),
    /RULESET_MODEL_UNSUPPORTED/,
  );

  const modelArtifact = structuredClone(compilePrepared(baseline));
  modelArtifact.ruleset.models.checks = {
    id: 'check.not-supported',
    version: 99,
  };
  const modelValidation = validateArtifact(modelArtifact);
  assert.notEqual(modelValidation.status, 0);
  assert.match(modelValidation.stderr, /RULESET_MODEL_UNSUPPORTED/);
});

function acceptedPrepared(
  fixture: ReturnType<typeof packageFixture>,
): PreparedPlayBundle {
  const result = preparePlayBundle({
    bundle: fixture.bundle,
    contentPacks: fixture.contentPacks,
  });
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.prepared;
}

function prepareFixture(options: FixtureOptions) {
  const fixture = packageFixture(options);
  return preparePlayBundle({
    bundle: fixture.bundle,
    contentPacks: fixture.contentPacks,
  });
}

function compilePrepared(prepared: PreparedPlayBundle): CompiledArtifact {
  const compilation = runCompilation(prepared);
  assert.equal(compilation.status, 0, compilation.stderr);
  const result = JSON.parse(compilation.stdout) as CompilationEnvelope;
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.artifact;
}

function runCompilation(prepared: unknown) {
  return spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'compile_play_bundle',
    ],
    { cwd: root, encoding: 'utf8', input: canonicalJson(prepared) },
  );
}

function failedCompilationDiagnostics(
  compilation: ReturnType<typeof runCompilation>,
): string {
  assert.equal(compilation.status, 0, compilation.stderr);
  const result = JSON.parse(compilation.stdout) as CompilationEnvelope;
  assert.equal(result.ok, false, compilation.stdout);
  return JSON.stringify(result.diagnostics);
}

function validateArtifact(artifact: unknown) {
  return spawnSync(
    'cargo',
    [
      'run',
      '--quiet',
      '--manifest-path',
      join(root, 'Cargo.toml'),
      '-p',
      'rpg-compiler',
      '--bin',
      'validate_play_bundle',
    ],
    { cwd: root, encoding: 'utf8', input: canonicalJson(artifact) },
  );
}

interface FixtureOptions {
  readonly damageAmount?: number;
  readonly damageSemanticId?: string;
  readonly runtimeDamageDefinitionId?: string;
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
  readonly bundle: ReturnType<typeof composePlayBundle>;
  readonly contentPacks: readonly ContentPackSource[];
} {
  const privateTemplate = defineTemplateDefinition({
    kind: 'template',
    id: 'sample.private-template',
    visibility: options.unreachableVisibility ?? 'private',
    extensionPolicy: 'derivable',
    source: { module: 'foundation/templates.ts', declaration: 'privateTemplate' },
  });
  const catalogs = defineContentCatalog({
    packageId: 'sample.foundation',
    sourceModule: 'foundation/damage-types.ts',
    entries: {
      arcane: {
        definitionId: 'catalog.damage.arcane',
        category: 'damageType',
        id: options.damageSemanticId ?? 'arcane',
        label: 'Arcane',
      },
    },
  });
  const foundationManifest: ContentPackManifest = defineContentPack({
    identity: { id: 'sample.foundation', version: '1.1.0' },
    entry: { module: 'foundation/ruleset.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: options.dependencyCycle
      ? [contentPackDependency({ id: 'sample.core', version: '^1.0.0', importAs: 'core' })]
      : [],
    requirements: { operations: [], capabilities: [] },
    definitions: [
      ...catalogs.definitions,
      privateTemplate,
      ...(options.duplicateGlobalDefinition
        ? [
            defineSupportDefinition({
              kind: 'support',
              id: 'sample.spark',
              visibility: 'public',
              extensionPolicy: 'sealed',
              source: { module: 'foundation/conflict.ts', declaration: 'spark' },
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
        type: options.runtimeDamageDefinitionId === undefined
          ? catalogs.references.arcane
          : lowLevelCatalogReference({
              category: 'damageType',
              packageId: 'sample.foundation',
              definitionId: options.runtimeDamageDefinitionId,
            }),
      }),
    }),
  });
  const ordinarySpark = defineActionDefinition({
    kind: 'action',
    id: 'sample.spark',
    visibility: 'public',
    extensionPolicy: 'patchable',
    source: {
      module: options.sourceModule ?? 'core/actions/spark.ts',
      declaration: 'spark',
    },
    presentation: { label: options.label ?? 'Spark' },
    action: sparkAction,
  });
  const explicitReferenceId = options.referencePrivateDefinition
    ? 'sample.private-template'
    : options.referenceId;
  const spark =
    explicitReferenceId === undefined
      ? ordinarySpark
      : withLowLevelDefinitionReferences(ordinarySpark, [
          definitionReference({
            importAs: 'foundation',
            definitionId: explicitReferenceId,
          }),
        ]);
  const coreManifest = defineContentPack({
    identity: { id: 'sample.core', version: '1.0.0' },
    entry: { module: 'core/ruleset.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: options.languageVersion ?? '^1.0.0' },
    dependencies: [
      contentPackDependency({
        id: 'sample.foundation',
        version: '^1.0.0',
        importAs: 'foundation',
      }),
      ...(options.duplicateDependencyAlias
        ? [
            contentPackDependency({
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
            mixins: [],
            localPatch: { version: 1, operations: [] },
            version: 1,
          },
        ]
      : [],
  });
  return {
    bundle: composePlayBundle({
      identity: { id: 'sample.bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({ id: 'sample.core', version: '1.0.0' }),
      add: [contentPackRequest({ id: 'sample.foundation', version: '^1.0.0' })],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(coreManifest), contentPackSource(foundationManifest)],
  };
}

function emptyPackageSource(id: string, version: string): ContentPackSource {
  return contentPackSource(
    defineContentPack({
      identity: { id, version },
      entry: { module: `${id}.ts`, declaration: 'default' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      dependencies: [],
      requirements: { operations: [], capabilities: [] },
      definitions: [],
      exports: [],
      policyBindings: [],
      relationships: [],
    }),
  );
}

function packageSourceWithDependency(
  id: string,
  sharedVersion: string,
): ContentPackSource {
  return contentPackSource(
    defineContentPack({
      identity: { id, version: '1.0.0' },
      entry: { module: `${id}.ts`, declaration: 'default' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      dependencies: [
        contentPackDependency({
          id: 'sample.shared',
          version: sharedVersion,
          importAs: 'shared',
        }),
      ],
      requirements: { operations: [], capabilities: [] },
      definitions: [],
      exports: [],
      policyBindings: [],
      relationships: [],
    }),
  );
}

interface CompiledArtifact {
  readonly artifactId: string;
  ruleset: {
    models: {
      checks: { id: string; version: number };
    };
  };
  contentRequirements: {
    operations: { id: string; version: number }[];
  };
  readonly materializedDefinitions: {
    readonly id: string;
    semantic: unknown;
  }[];
  readonly fingerprints: {
    readonly source: string;
    readonly semantic: string;
    readonly presentation: string;
  };
}

type CompilationEnvelope =
  | { readonly ok: true; readonly artifact: CompiledArtifact; readonly diagnostics: readonly [] }
  | { readonly ok: false; readonly diagnostics: readonly unknown[] };
