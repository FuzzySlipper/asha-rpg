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
  defineDerivedDefinition,
  defineMixinDefinition,
  defineRulesetPackage,
  defineRulesetRelationship,
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
  RulesetMixinApplication,
  RulesetPackageSource,
} from '@asha-rpg/authoring';

const root = fileURLToPath(new URL('../../../', import.meta.url));

type CompilePreparedResult =
  | {
      readonly ok: true;
      readonly artifact: {
        readonly materializedDefinitions: readonly unknown[];
        readonly derivationProvenance: readonly unknown[];
        readonly overlayProvenance: readonly unknown[];
        readonly fingerprints: {
          readonly source: string;
          readonly semantic: string;
          readonly presentation: string;
        };
      };
      readonly diagnostics: readonly unknown[];
    }
  | {
      readonly ok: false;
      readonly diagnostics: readonly { readonly code: string }[];
    };

test('ordered mixins, local patches, and overlays materialize deterministically', () => {
  const baseSources = materializationSources('multiplyThenAdd');
  const baseline = acceptedPrepared(composition([]), baseSources);
  const derived = baseline.derivationProvenance[0];
  assert.ok(derived);
  assert.deepEqual(
    derived.mixins.map((mixin) => mixin.definitionId),
    ['sample.multiply-range', 'sample.add-range'],
  );
  assert.equal(derived.changes.length, 3);
  assert.equal(derived.materializedFingerprint.startsWith('fnv1a64:'), true);

  const semanticOverlay = overlayPackage({
    id: 'sample.semantic-overlay',
    expectedFingerprint: derived.materializedFingerprint,
    plane: 'semantic',
    path: ['targets', 'maximumRange'],
    value: 11,
  });
  const semanticPrepared = acceptedPrepared(
    composition(['sample.semantic-overlay']),
    [...baseSources, semanticOverlay],
  );
  const semanticProvenance = semanticPrepared.overlayProvenance[0];
  assert.ok(semanticProvenance);
  assert.equal(semanticProvenance.beforeFingerprint, derived.materializedFingerprint);
  assert.notEqual(semanticProvenance.afterFingerprint, semanticProvenance.beforeFingerprint);

  const presentationOverlay = overlayPackage({
    id: 'sample.presentation-overlay',
    expectedFingerprint: semanticProvenance.afterFingerprint,
    plane: 'presentation',
    path: ['label'],
    value: 'Arc Variant: Stormfront',
  });
  const finalPrepared = acceptedPrepared(
    composition(['sample.semantic-overlay', 'sample.presentation-overlay']),
    [...baseSources, semanticOverlay, presentationOverlay],
  );
  const reorderedSources = [...baseSources, presentationOverlay, semanticOverlay].reverse();
  const repeated = acceptedPrepared(
    composition(['sample.semantic-overlay', 'sample.presentation-overlay']),
    reorderedSources,
  );
  assert.equal(canonicalJson(finalPrepared), canonicalJson(repeated));
  assert.deepEqual(
    finalPrepared.overlayProvenance.map((overlay) => overlay.plane),
    ['semantic', 'presentation'],
  );
  const actionDefinition = finalPrepared.materializedDefinitions.find(
    (definition) => definition.id === 'sample.arc-variant',
  );
  assert.ok(actionDefinition);
  assert.equal(
    readPath(actionDefinition.semantic, ['targets', 'maximumRange']),
    11,
  );
  assert.equal(actionDefinition.presentation?.label, 'Arc Variant: Stormfront');
  assert.equal(actionDefinition.fingerprint.startsWith('fnv1a64:'), true);

  const baselineArtifact = compilePrepared(baseline);
  const semanticArtifact = compilePrepared(semanticPrepared);
  const artifact = compilePrepared(finalPrepared);
  assert.equal(artifact.materializedDefinitions.length, 3);
  assert.equal(artifact.derivationProvenance.length, 1);
  assert.equal(artifact.overlayProvenance.length, 2);
  assert.notEqual(
    semanticArtifact.fingerprints.semantic,
    baselineArtifact.fingerprints.semantic,
  );
  assert.equal(
    semanticArtifact.fingerprints.presentation,
    baselineArtifact.fingerprints.presentation,
  );
  assert.equal(artifact.fingerprints.semantic, semanticArtifact.fingerprints.semantic);
  assert.notEqual(
    artifact.fingerprints.presentation,
    semanticArtifact.fingerprints.presentation,
  );

  const reversedMixins = acceptedPrepared(
    composition([]),
    materializationSources('addThenMultiply'),
  );
  assert.notEqual(
    reversedMixins.derivationProvenance[0]?.materializedFingerprint,
    derived.materializedFingerprint,
  );
  assert.notEqual(
    compilePrepared(reversedMixins).fingerprints.semantic,
    baselineArtifact.fingerprints.semantic,
  );
});

test('Rust rejects provenance fingerprints and coverage that do not match materialized definitions', () => {
  const sources = materializationSources('multiplyThenAdd');
  const baseline = acceptedPrepared(composition([]), sources);
  const falseDerivationFingerprint: PreparedRulesetCompilation = {
    ...baseline,
    derivationProvenance: baseline.derivationProvenance.map((provenance, index) =>
      index === 0
        ? { ...provenance, materializedFingerprint: 'fnv1a64:0000000000000000' }
        : provenance,
    ),
  };
  assertCompilationFailsWith(
    falseDerivationFingerprint,
    'RULESET_DERIVATION_MATERIALIZED_FINGERPRINT_MISMATCH',
  );

  const derived = baseline.derivationProvenance[0];
  assert.ok(derived);
  const semanticOverlay = overlayPackage({
    id: 'sample.semantic-overlay',
    expectedFingerprint: derived.materializedFingerprint,
    plane: 'semantic',
    path: ['targets', 'maximumRange'],
    value: 11,
  });
  const semanticPrepared = acceptedPrepared(
    composition(['sample.semantic-overlay']),
    [...sources, semanticOverlay],
  );
  const falseOverlayFingerprint: PreparedRulesetCompilation = {
    ...semanticPrepared,
    overlayProvenance: semanticPrepared.overlayProvenance.map((provenance, index) =>
      index === 0
        ? { ...provenance, afterFingerprint: 'fnv1a64:0000000000000000' }
        : provenance,
    ),
  };
  assertCompilationFailsWith(
    falseOverlayFingerprint,
    'RULESET_OVERLAY_AFTER_FINGERPRINT_MISMATCH',
  );

  const missingOverlayProvenance: PreparedRulesetCompilation = {
    ...semanticPrepared,
    overlayProvenance: [],
  };
  assertCompilationFailsWith(
    missingOverlayProvenance,
    'RULESET_OVERLAY_PROVENANCE_COVERAGE_MISMATCH',
  );
});

test('derivation and overlay errors retain exact path and policy diagnostics', () => {
  const cycle = prepareRulesetCompilation({
    composition: composeRuleset({
      identity: { id: 'sample.cycle-composition', version: '1.0.0' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      base: rulesetPackageRequest({ id: 'sample.cycle', version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: {},
    }),
    packages: [cyclePackage()],
  });
  assert.equal(cycle.ok, false);
  if (!cycle.ok) {
    const diagnostic = cycle.diagnostics.find(
      (entry) => entry.code === 'RULESET_DERIVATION_CYCLE',
    );
    assert.deepEqual(diagnostic?.graphPath, [
      'sample.cycle@1.0.0#sample.a',
      'sample.cycle@1.0.0#sample.b',
      'sample.cycle@1.0.0#sample.a',
    ]);
  }

  const incompatible = prepareRulesetCompilation({
    composition: composeRuleset({
      identity: { id: 'sample.incompatible-composition', version: '1.0.0' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      base: rulesetPackageRequest({ id: 'sample.incompatible', version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: {},
    }),
    packages: [incompatibleBasePackage()],
  });
  assert.equal(incompatible.ok, false);
  if (!incompatible.ok) {
    assert.ok(
      incompatible.diagnostics.some(
        (entry) =>
          entry.code === 'RULESET_DERIVATION_KIND_INCOMPATIBLE' &&
          entry.definitionId === 'sample.invalid-derived' &&
          entry.actual === 'template',
      ),
    );
  }

  const sources = materializationSources('multiplyThenAdd');
  const baseline = acceptedPrepared(composition([]), sources);
  const expected = baseline.derivationProvenance[0]?.materializedFingerprint;
  assert.ok(expected);
  const mismatch = overlayPackage({
    id: 'sample.mismatch-overlay',
    expectedFingerprint: 'fnv1a64:0000000000000000',
    plane: 'semantic',
    path: ['targets', 'maximumRange'],
    value: 9,
  });
  const mismatchResult = prepareRulesetCompilation({
    composition: composition(['sample.mismatch-overlay']),
    packages: [...sources, mismatch],
  });
  assert.equal(mismatchResult.ok, false);
  if (!mismatchResult.ok) {
    assert.ok(
      mismatchResult.diagnostics.some(
        (entry) =>
          entry.code === 'RULESET_OVERLAY_EXPECTED_FINGERPRINT_MISMATCH' &&
          entry.expected === 'fnv1a64:0000000000000000' &&
          entry.actual === expected,
      ),
    );
  }

  const forbidden = forbiddenOverlayPackage();
  const forbiddenResult = prepareRulesetCompilation({
    composition: composition(['sample.forbidden-overlay']),
    packages: [...sources, forbidden],
  });
  assert.equal(forbiddenResult.ok, false);
  if (!forbiddenResult.ok) {
    assert.ok(
      forbiddenResult.diagnostics.some(
        (entry) => entry.code === 'RULESET_OVERLAY_TARGET_FORBIDDEN',
      ),
    );
  }
});

test('configuration applies only an explicitly exposed typed option', () => {
  const configurable = defineSupportDefinition({
    kind: 'support',
    id: 'catalog.damage.configurable',
    visibility: 'public',
    extensionPolicy: 'configurable',
    source: { module: 'config/catalog.ts', declaration: 'damage' },
    references: [],
    semantic: { catalog: 'damageType', id: 'storm' },
    presentation: { label: 'Configurable damage' },
  });
  const source = rulesetPackageSource(defineRulesetPackage({
    identity: { id: 'sample.config', version: '1.0.0' },
    entry: { module: 'config/index.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [],
    requirements: { operations: [], capabilities: [] },
    definitions: [configurable],
    exports: [configurable.id],
    policyBindings: [],
    relationships: [defineRulesetRelationship({
      kind: 'configures',
      optionId: 'sample.damage-kind',
      target: definitionReference({ definitionId: configurable.id }),
      value: 'shadow',
      patch: {
        version: 1,
        operations: [{
          kind: 'setScalar',
          plane: 'semantic',
          path: fields('id'),
          value: 'shadow',
        }],
      },
      version: 1,
    })],
  }));
  const selected = prepareRulesetCompilation({
    composition: composeRuleset({
      identity: { id: 'sample.configured', version: '1.0.0' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      base: rulesetPackageRequest({ id: 'sample.config', version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: { 'sample.damage-kind': 'shadow' },
    }),
    packages: [source],
  });
  assert.equal(selected.ok, true, JSON.stringify(selected));
  if (!selected.ok) return;
  assert.equal(
    readPath(selected.prepared.materializedDefinitions[0]?.semantic, ['id']),
    'shadow',
  );
  assert.ok(
    selected.prepared.relationships.some(
      (relationship) => relationship.kind === 'configures',
    ),
  );

  const unavailable = prepareRulesetCompilation({
    composition: composeRuleset({
      identity: { id: 'sample.configured', version: '1.0.0' },
      language: { id: 'asha-rpg', version: '^1.0.0' },
      base: rulesetPackageRequest({ id: 'sample.config', version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: { 'sample.damage-kind': 'radiant' },
    }),
    packages: [source],
  });
  assert.equal(unavailable.ok, false);
  if (!unavailable.ok) {
    assert.ok(
      unavailable.diagnostics.some(
        (entry) => entry.code === 'RULESET_CONFIGURATION_OPTION_UNAVAILABLE',
      ),
    );
  }
});

function materializationSources(
  order: 'multiplyThenAdd' | 'addThenMultiply',
): readonly RulesetPackageSource[] {
  const damageTypeDefinition = defineSupportDefinition({
    kind: 'support',
    id: 'catalog.damage.storm',
    visibility: 'private',
    extensionPolicy: 'sealed',
    source: { module: 'foundation/catalog.ts', declaration: 'storm' },
    references: [],
    semantic: { catalog: 'damageType', id: 'storm' },
    presentation: { label: 'Storm' },
  });
  const mixinSupportDefinition = defineSupportDefinition({
    kind: 'support',
    id: 'catalog.stat.range-tuning',
    visibility: 'private',
    extensionPolicy: 'sealed',
    source: { module: 'foundation/catalog.ts', declaration: 'rangeTuning' },
    references: [],
    semantic: { catalog: 'stat', id: 'range-tuning' },
    presentation: { label: 'Range tuning' },
  });
  const baseAction = defineActionDefinition({
    kind: 'action',
    id: 'sample.arc-base',
    visibility: 'public',
    extensionPolicy: 'derivable',
    source: { module: 'foundation/actions.ts', declaration: 'arcBase' },
    references: [definitionReference({ definitionId: 'catalog.damage.storm' })],
    presentation: { label: 'Arc Base', description: 'Foundation action' },
    action: action({
      id: actionId('sample.arc-base'),
      name: 'Arc Base',
      sourcePath: 'foundation/actions.ts',
      targets: hostile({ range: 3 }),
      check: noRoll(),
      program: onCheck({
        noRoll: damage({ amount: constant(4), type: damageType('catalog.damage.storm') }),
      }),
    }),
  });
  const multiply = defineMixinDefinition({
    kind: 'mixin',
    id: 'sample.multiply-range',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'foundation/mixins.ts', declaration: 'multiplyRange' },
    references: [definitionReference({ definitionId: mixinSupportDefinition.id })],
    parameters: [{ id: 'factor', type: 'number' }],
    patch: {
      version: 1,
      operations: [{
        kind: 'adjustNumber',
        plane: 'semantic',
        path: fields('targets', 'maximumRange'),
        multiply: { parameter: 'factor' },
        add: 0,
      }],
    },
  });
  const add = defineMixinDefinition({
    kind: 'mixin',
    id: 'sample.add-range',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'foundation/mixins.ts', declaration: 'addRange' },
    references: [],
    parameters: [{ id: 'amount', type: 'number', default: 1 }],
    patch: {
      version: 1,
      operations: [{
        kind: 'adjustNumber',
        plane: 'semantic',
        path: fields('targets', 'maximumRange'),
        multiply: 1,
        add: { parameter: 'amount' },
      }],
    },
  });
  const foundation = rulesetPackageSource(defineRulesetPackage({
    identity: { id: 'sample.foundation', version: '1.0.0' },
    entry: { module: 'foundation/index.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [],
    requirements: {
      operations: [{ id: 'operation.damage', version: 1 }],
      capabilities: [{ id: 'capability.vitality', version: 1 }],
    },
    definitions: [damageTypeDefinition, mixinSupportDefinition, baseAction, multiply, add],
    exports: [
      baseAction.id,
      multiply.id,
      add.id,
    ],
    policyBindings: [],
    relationships: [],
  }));

  const mixins: readonly RulesetMixinApplication[] = order === 'multiplyThenAdd'
    ? [
        { target: definitionReference({ importAs: 'foundation', definitionId: multiply.id }), parameters: { factor: 2 } },
        { target: definitionReference({ importAs: 'foundation', definitionId: add.id }), parameters: { amount: 1 } },
      ]
    : [
        { target: definitionReference({ importAs: 'foundation', definitionId: add.id }), parameters: { amount: 1 } },
        { target: definitionReference({ importAs: 'foundation', definitionId: multiply.id }), parameters: { factor: 2 } },
      ];
  const derived = defineDerivedDefinition({
    kind: 'derived',
    id: 'sample.arc-variant',
    materializesAs: 'action',
    visibility: 'public',
    extensionPolicy: 'patchable',
    source: { module: 'core/derived.ts', declaration: 'arcVariant' },
    references: [],
    presentation: { label: 'ignored authored placeholder' },
  });
  const core = rulesetPackageSource(defineRulesetPackage({
    identity: { id: 'sample.core', version: '1.0.0' },
    entry: { module: 'core/index.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [rulesetDependency({ id: 'sample.foundation', version: '1.0.0', importAs: 'foundation' })],
    requirements: {
      operations: [{ id: 'operation.damage', version: 1 }],
      capabilities: [{ id: 'capability.vitality', version: 1 }],
    },
    definitions: [derived],
    exports: [derived.id],
    policyBindings: [],
    relationships: [defineRulesetRelationship({
      kind: 'derivesFrom',
      definitionId: derived.id,
      target: definitionReference({ importAs: 'foundation', definitionId: baseAction.id }),
      mixins,
      localPatch: {
        version: 1,
        operations: [{
          kind: 'setScalar',
          plane: 'presentation',
          path: fields('description'),
          value: 'Derived locally after ordered mixins',
        }],
      },
      version: 1,
    })],
  }));
  return [core, foundation];
}

function overlayPackage(options: {
  readonly id: string;
  readonly expectedFingerprint: string;
  readonly plane: 'semantic' | 'presentation';
  readonly path: readonly string[];
  readonly value: string | number | boolean;
}): RulesetPackageSource {
  return rulesetPackageSource(defineRulesetPackage({
    identity: { id: options.id, version: '1.0.0' },
    entry: { module: `${options.id}.ts`, declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [rulesetDependency({ id: 'sample.core', version: '1.0.0', importAs: 'core' })],
    requirements: { operations: [], capabilities: [] },
    definitions: [],
    exports: [],
    policyBindings: [],
    relationships: [defineRulesetRelationship({
      kind: 'patches',
      definitionId: `${options.id}.patch`,
      target: definitionReference({ importAs: 'core', definitionId: 'sample.arc-variant' }),
      targetPackage: { id: 'sample.core', version: '1.0.0' },
      expectedFingerprint: options.expectedFingerprint,
      patch: {
        version: 1,
        operations: [{
          kind: 'setScalar',
          plane: options.plane,
          path: fields(...options.path),
          value: options.value,
        }],
      },
      plane: options.plane,
      conflictPolicy: 'reject',
      version: 1,
    })],
  }));
}

function forbiddenOverlayPackage(): RulesetPackageSource {
  return rulesetPackageSource(defineRulesetPackage({
    identity: { id: 'sample.forbidden-overlay', version: '1.0.0' },
    entry: { module: 'forbidden.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [rulesetDependency({ id: 'sample.foundation', version: '1.0.0', importAs: 'foundation' })],
    requirements: { operations: [], capabilities: [] },
    definitions: [],
    exports: [],
    policyBindings: [],
    relationships: [defineRulesetRelationship({
      kind: 'patches',
      definitionId: 'sample.forbidden-overlay.patch',
      target: definitionReference({ importAs: 'foundation', definitionId: 'sample.arc-base' }),
      targetPackage: { id: 'sample.foundation', version: '1.0.0' },
      expectedFingerprint: 'fnv1a64:0000000000000000',
      patch: { version: 1, operations: [{ kind: 'setScalar', plane: 'presentation', path: fields('label'), value: 'Forbidden' }] },
      plane: 'presentation',
      conflictPolicy: 'reject',
      version: 1,
    })],
  }));
}

function cyclePackage(): RulesetPackageSource {
  const a = defineDerivedDefinition({
    kind: 'derived', id: 'sample.a', materializesAs: 'action', visibility: 'public', extensionPolicy: 'derivable',
    source: { module: 'cycle.ts', declaration: 'a' }, references: [],
  });
  const b = defineDerivedDefinition({
    kind: 'derived', id: 'sample.b', materializesAs: 'action', visibility: 'public', extensionPolicy: 'derivable',
    source: { module: 'cycle.ts', declaration: 'b' }, references: [],
  });
  return rulesetPackageSource(defineRulesetPackage({
    identity: { id: 'sample.cycle', version: '1.0.0' },
    entry: { module: 'cycle.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' }, dependencies: [],
    requirements: { operations: [], capabilities: [] }, definitions: [a, b], exports: [a.id], policyBindings: [],
    relationships: [a, b].map((definition, index) => defineRulesetRelationship({
      kind: 'derivesFrom', definitionId: definition.id,
      target: definitionReference({ definitionId: index === 0 ? b.id : a.id }),
      mixins: [], localPatch: { version: 1, operations: [] }, version: 1,
    })),
  }));
}

function incompatibleBasePackage(): RulesetPackageSource {
  const template = defineTemplateDefinition({
    kind: 'template',
    id: 'sample.template-base',
    visibility: 'private',
    extensionPolicy: 'derivable',
    source: { module: 'incompatible.ts', declaration: 'templateBase' },
    references: [],
  });
  const derived = defineDerivedDefinition({
    kind: 'derived',
    id: 'sample.invalid-derived',
    materializesAs: 'action',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'incompatible.ts', declaration: 'invalidDerived' },
    references: [],
  });
  return rulesetPackageSource(defineRulesetPackage({
    identity: { id: 'sample.incompatible', version: '1.0.0' },
    entry: { module: 'incompatible.ts', declaration: 'default' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    dependencies: [],
    requirements: { operations: [], capabilities: [] },
    definitions: [template, derived],
    exports: [derived.id],
    policyBindings: [],
    relationships: [defineRulesetRelationship({
      kind: 'derivesFrom',
      definitionId: derived.id,
      target: definitionReference({ definitionId: template.id }),
      mixins: [],
      localPatch: { version: 1, operations: [] },
      version: 1,
    })],
  }));
}

function composition(overlays: readonly string[]) {
  return composeRuleset({
    identity: { id: 'sample.materialized', version: '1.0.0' },
    language: { id: 'asha-rpg', version: '^1.0.0' },
    base: rulesetPackageRequest({ id: 'sample.core', version: '1.0.0' }),
    add: [],
    overlays: overlays.map((id) => rulesetPackageRequest({ id, version: '1.0.0' })),
    configure: {},
  });
}

function acceptedPrepared(
  selectedComposition: ReturnType<typeof composition>,
  packages: readonly RulesetPackageSource[],
): PreparedRulesetCompilation {
  const result = prepareRulesetCompilation({ composition: selectedComposition, packages });
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.prepared;
}

function compilePrepared(prepared: PreparedRulesetCompilation) {
  const result = compilePreparedResult(prepared);
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.artifact;
}

function compilePreparedResult(prepared: PreparedRulesetCompilation): CompilePreparedResult {
  const compilation = spawnSync(
    'cargo',
    ['run', '--quiet', '--manifest-path', join(root, 'Cargo.toml'), '-p', 'rpg-compiler', '--bin', 'compile_ruleset'],
    { cwd: root, encoding: 'utf8', input: canonicalJson(prepared) },
  );
  assert.equal(compilation.status, 0, compilation.stderr);
  return JSON.parse(compilation.stdout) as CompilePreparedResult;
}

function assertCompilationFailsWith(
  prepared: PreparedRulesetCompilation,
  diagnosticCode: string,
): void {
  const result = compilePreparedResult(prepared);
  assert.equal(result.ok, false, JSON.stringify(result));
  if (result.ok) return;
  assert.ok(
    result.diagnostics.some((diagnostic) => diagnostic.code === diagnosticCode),
    JSON.stringify(result.diagnostics),
  );
}

function fields(...names: readonly string[]) {
  return names.map((name) => ({ kind: 'field' as const, name }));
}

function readPath(value: unknown, path: readonly string[]): unknown {
  let current = value;
  for (const field of path) {
    if (current === null || typeof current !== 'object' || Array.isArray(current)) return undefined;
    current = (current as Readonly<Record<string, unknown>>)[field];
  }
  return current;
}
