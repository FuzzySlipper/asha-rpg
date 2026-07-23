import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

import {
  action,
  actionId,
  actionProcedureInvocation,
  actionProcedureParameterReference,
  canonicalJson,
  composePlayBundle,
  constant,
  contentPackDependency,
  contentPackRequest,
  contentPackSource,
  defineActionDefinition,
  defineActionInvocationDefinition,
  defineActionProcedureDefinition,
  defineContentCatalog,
  defineContentPack,
  hostile,
  noRoll,
  onCheck,
  preparePlayBundle,
  rulesetDefense,
  stableFingerprint,
} from '@asha-rpg/authoring';
import type {
  ContentDefinition,
  ContentPackSource,
  PreparedPlayBundle,
} from '@asha-rpg/authoring';

import { contractTestRuleset } from './test-ruleset.ts';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const foundationId = 'procedure.foundation';

const rangeParameter = {
  id: 'range',
  type: 'boundedInteger',
  minimum: 1,
  maximum: 12,
} as const;
const attackBonusParameter = { id: 'attack-bonus', type: 'formula' } as const;
const defenseParameter = {
  id: 'defense',
  type: 'rulesetValueReference',
} as const;
const damageParameter = { id: 'damage', type: 'formula' } as const;
const damageTypeParameter = {
  id: 'damage-type',
  type: 'catalogReference',
} as const;

const damageCatalog = defineContentCatalog({
  packageId: foundationId,
  sourceModule: 'foundation/damage-types.ts',
  entries: {
    force: {
      definitionId: 'damage.force',
      category: 'damageType',
      id: 'force',
      label: 'Force',
    },
  },
});

const basicAttackProcedure = defineActionProcedureDefinition({
  id: 'procedure.basic-attack',
  ownerPackageId: foundationId,
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: {
    module: 'foundation/action-procedures.ts',
    declaration: 'basicAttack',
  },
  presentation: { label: 'Basic attack procedure' },
  parameters: [
    attackBonusParameter,
    damageParameter,
    damageTypeParameter,
    defenseParameter,
    rangeParameter,
  ] as const,
  implementation: {
    kind: 'inline',
    template: {
      targets: {
        kind: 'participant',
        team: 'hostile',
        maximumRange: actionProcedureParameterReference(rangeParameter),
        maximumTargets: 1,
      },
      check: {
        kind: 'attack',
        modifier: actionProcedureParameterReference(attackBonusParameter),
        defenseId: actionProcedureParameterReference(defenseParameter),
      },
      rollScope: 'shared',
      costs: [],
      program: {
        kind: 'atomic',
        body: {
          kind: 'onCheck',
          hit: {
            kind: 'operation',
            operation: {
              kind: 'damage',
              amount: actionProcedureParameterReference(damageParameter),
              damageType:
                actionProcedureParameterReference(damageTypeParameter),
            },
          },
        },
      },
    },
  },
});

const strikeArguments = {
  'attack-bonus': constant(2),
  damage: constant(4),
  'damage-type': damageCatalog.references.force,
  defense: rulesetDefense(contractTestRuleset, 'guard'),
  range: 1,
} as const;

const invokedStrike = defineActionInvocationDefinition({
  id: 'action.procedure-strike',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: {
    module: 'consumer/actions.ts',
    declaration: 'procedureStrike',
  },
  presentation: { label: 'Procedure strike' },
  procedure: basicAttackProcedure,
  importAs: 'foundation',
  arguments: strikeArguments,
});

const forwardedProcedure = defineActionProcedureDefinition({
  ...basicAttackProcedure,
  id: 'procedure.forwarded-attack',
  ownerPackageId: 'procedure.consumer',
  source: {
    module: 'consumer/action-procedures.ts',
    declaration: 'forwardedAttack',
  },
  implementation: actionProcedureInvocation(
    basicAttackProcedure,
    {
      'attack-bonus':
        actionProcedureParameterReference(attackBonusParameter),
      damage: actionProcedureParameterReference(damageParameter),
      'damage-type':
        actionProcedureParameterReference(damageTypeParameter),
      defense: actionProcedureParameterReference(defenseParameter),
      range: actionProcedureParameterReference(rangeParameter),
    },
    'foundation',
  ),
});

const forwardedStrike = defineActionInvocationDefinition({
  id: 'action.forwarded-strike',
  visibility: 'public',
  extensionPolicy: 'sealed',
  source: {
    module: 'consumer/actions.ts',
    declaration: 'forwardedStrike',
  },
  presentation: { label: 'Forwarded strike' },
  procedure: forwardedProcedure,
  arguments: strikeArguments,
});

test('a dependent package invokes an owner-bound exported procedure and Rust expands it', () => {
  const prepared = acceptedPrepared(invokedStrike);
  const actionDefinition = prepared.materializedDefinitions.find(
    (definition) => definition.id === invokedStrike.id,
  );
  const procedureDefinition = prepared.materializedDefinitions.find(
    (definition) => definition.id === basicAttackProcedure.id,
  );
  assert.equal(actionDefinition?.kind, 'action');
  assert.equal(
    readPath(actionDefinition?.semantic, ['kind']),
    'invocation',
  );
  assert.equal(procedureDefinition?.kind, 'actionProcedure');

  const compilation = compilePrepared(prepared);
  assert.equal(compilation.ok, true, JSON.stringify(compilation));
  if (!compilation.ok) return;
  assert.equal(
    compilation.artifact.materializedDefinitions.some(
      (definition) => definition.id === basicAttackProcedure.id,
    ),
    true,
  );
});

test('inline actions remain an explicit alternative to procedure invocation', () => {
  const inlineAction = action({
    id: actionId('action.inline'),
    name: 'Inline',
    sourcePath: 'consumer/inline.ts#inline',
    targets: hostile({ range: 1 }),
    check: noRoll(),
    program: onCheck({
      noRoll: {
        kind: 'operation',
        operation: {
          kind: 'heal',
          amount: constant(1),
        },
        timing: { kind: 'immediate' },
      },
    }),
  });
  const inlineDefinition = defineActionDefinition({
    id: inlineAction.id,
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'consumer/inline.ts', declaration: 'inline' },
    action: inlineAction,
  });
  const prepared = acceptedPrepared(inlineDefinition, false);
  assert.equal(
    readPath(prepared.materializedDefinitions[0]?.semantic, ['kind']),
    'inline',
  );
  assert.equal(compilePrepared(prepared).ok, true);
});

test('procedure composition forwards typed parameters without a TypeScript callback', () => {
  const foundation = defineContentPack({
    identity: { id: foundationId, version: '1.0.0' },
    entry: { module: 'foundation/index.ts', declaration: 'content' },
    definitions: [...damageCatalog.definitions, basicAttackProcedure],
    exports: [
      ...damageCatalog.definitions.map((definition) => definition.id),
      basicAttackProcedure.id,
    ],
  });
  const consumer = defineContentPack({
    identity: { id: 'procedure.consumer', version: '1.0.0' },
    entry: { module: 'consumer/index.ts', declaration: 'content' },
    dependencies: [
      contentPackDependency({
        id: foundationId,
        version: '1.0.0',
        importAs: 'foundation',
      }),
    ],
    definitions: [forwardedProcedure, forwardedStrike],
    exports: [forwardedStrike.id],
  });
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: 'procedure.forwarded-bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({
        id: consumer.identity.id,
        version: consumer.identity.version,
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [
      contentPackSource(consumer),
      contentPackSource(foundation),
    ],
  });
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  const compilation = compilePrepared(result.prepared);
  assert.equal(compilation.ok, true, JSON.stringify(compilation));
});

test('missing, extra, wrong-typed, and wrong-owner invocation arguments fail preparation', () => {
  const cases = [
    {
      expected: 'ACTION_PROCEDURE_ARGUMENT_MISSING',
      arguments: withoutArgument(invokedStrike.invocation.arguments, 'range'),
    },
    {
      expected: 'ACTION_PROCEDURE_ARGUMENT_EXTRA',
      arguments: { ...invokedStrike.invocation.arguments, unexpected: 1 },
    },
    {
      expected: 'ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH',
      arguments: { ...invokedStrike.invocation.arguments, range: 'near' },
    },
  ] as const;
  for (const entry of cases) {
    const malformed = {
      ...invokedStrike,
      invocation: {
        ...invokedStrike.invocation,
        arguments: entry.arguments,
      },
    } as unknown as ContentDefinition;
    assertPreparationFails(malformed, entry.expected);
  }

  const wrongOwner = {
    ...invokedStrike,
    invocation: {
      ...invokedStrike.invocation,
      procedureOwnerPackageId: 'procedure.impostor',
    },
  } as unknown as ContentDefinition;
  assertPreparationFails(
    wrongOwner,
    'ACTION_PROCEDURE_REFERENCE_OWNER_MISMATCH',
  );
});

test('cyclic procedure composition is rejected by the closed definition graph', () => {
  const first = cyclicProcedure('procedure.first', 'procedure.second');
  const second = cyclicProcedure('procedure.second', 'procedure.first');
  const manifest = defineContentPack({
    identity: { id: 'procedure.cycle', version: '1.0.0' },
    entry: { module: 'cycle/index.ts', declaration: 'content' },
    definitions: [first, second],
    exports: [first.id],
  });
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: 'procedure.cycle-bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({
        id: manifest.identity.id,
        version: manifest.identity.version,
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(manifest)],
  });
  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.ok(
    result.diagnostics.some(
      (diagnostic) => diagnostic.code === 'CONTENT_PACK_DEFINITION_CYCLE',
    ),
    JSON.stringify(result.diagnostics),
  );
});

test('Rust rejects tampered procedure arguments during compiled artifact reload', () => {
  const prepared = acceptedPrepared(invokedStrike);
  const compilation = compilePrepared(prepared);
  assert.equal(compilation.ok, true, JSON.stringify(compilation));
  if (!compilation.ok) return;

  const materializedDefinitions = compilation.artifact.materializedDefinitions.map(
    (definition) => {
      if (definition.id !== invokedStrike.id) return definition;
      const semantic = replaceArgument(definition.semantic, 'range', 99);
      const { fingerprint: _fingerprint, ...identity } = definition;
      return {
        ...definition,
        semantic,
        fingerprint: stableFingerprint({ ...identity, semantic }),
      };
    },
  );
  const definitionCommitments = compilation.artifact.definitionCommitments.map(
    (commitment) => {
      if (
        commitment.kind !== 'concrete' ||
        commitment.definitionId !== invokedStrike.id
      ) {
        return commitment;
      }
      const semantic = replaceArgument(
        commitment.stage.value.semantic,
        'range',
        99,
      );
      const stage = {
        ...commitment.stage,
        value: { ...commitment.stage.value, semantic },
      };
      return { ...commitment, stage, fingerprint: stableFingerprint(stage) };
    },
  );
  const tampered = {
    ...compilation.artifact,
    materializedDefinitions,
    definitionCommitments,
  };
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
    { cwd: root, encoding: 'utf8', input: canonicalJson(tampered) },
  );
  assert.notEqual(validation.status, 0);
  assert.match(
    validation.stderr,
    /ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH/,
  );
});

test('procedure semantics participate in the artifact identity used by persistence', () => {
  const baselinePrepared = acceptedPrepared(invokedStrike);
  if (basicAttackProcedure.implementation.kind !== 'inline') {
    assert.fail('fixture procedure must be inline');
  }
  const changedProcedure = {
    ...basicAttackProcedure,
    implementation: {
      kind: 'inline' as const,
      template: {
        ...basicAttackProcedure.implementation.template,
        targets: {
          kind: 'participant' as const,
          team: 'any' as const,
          maximumRange: actionProcedureParameterReference(rangeParameter),
          maximumTargets: 1,
        },
      },
    },
  };
  const changedResult = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: 'procedure.consumer.bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({
        id: 'procedure.consumer',
        version: '1.0.0',
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: procedureSources(invokedStrike, changedProcedure),
  });
  if (!changedResult.ok) assert.fail(JSON.stringify(changedResult.diagnostics));
  const baseline = compilePrepared(baselinePrepared);
  const changed = compilePrepared(changedResult.prepared);
  assert.equal(baseline.ok, true, JSON.stringify(baseline));
  assert.equal(changed.ok, true, JSON.stringify(changed));
  if (!baseline.ok || !changed.ok) return;
  assert.notEqual(baseline.artifact.artifactId, changed.artifact.artifactId);
});

function acceptedPrepared(
  actionDefinition: ContentDefinition,
  includeFoundation = true,
): PreparedPlayBundle {
  const sources = includeFoundation
    ? procedureSources(actionDefinition)
    : [singleActionSource(actionDefinition)];
  const consumerId = includeFoundation
    ? 'procedure.consumer'
    : 'procedure.inline-consumer';
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: `${consumerId}.bundle`, version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({ id: consumerId, version: '1.0.0' }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: sources,
  });
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.prepared;
}

function procedureSources(
  actionDefinition: ContentDefinition,
  procedureDefinition = basicAttackProcedure,
): readonly ContentPackSource[] {
  const foundation = defineContentPack({
    identity: { id: foundationId, version: '1.0.0' },
    entry: { module: 'foundation/index.ts', declaration: 'content' },
    definitions: [...damageCatalog.definitions, procedureDefinition],
    exports: [
      ...damageCatalog.definitions.map((definition) => definition.id),
      procedureDefinition.id,
    ],
  });
  const consumer = defineContentPack({
    identity: { id: 'procedure.consumer', version: '1.0.0' },
    entry: { module: 'consumer/index.ts', declaration: 'content' },
    dependencies: [
      contentPackDependency({
        id: foundationId,
        version: '1.0.0',
        importAs: 'foundation',
      }),
    ],
    requirements: {
      operations: [{ id: 'operation.damage', version: 1 }],
      capabilities: [
        { id: 'capability.defenses', version: 1 },
        { id: 'capability.random', version: 1 },
        { id: 'capability.vitality', version: 1 },
      ],
    },
    definitions: [actionDefinition],
    exports: [actionDefinition.id],
  });
  return [contentPackSource(consumer), contentPackSource(foundation)];
}

function singleActionSource(
  actionDefinition: ContentDefinition,
): ContentPackSource {
  return contentPackSource(
    defineContentPack({
      identity: {
        id: 'procedure.inline-consumer',
        version: '1.0.0',
      },
      entry: { module: 'consumer/inline.ts', declaration: 'content' },
      requirements: {
        operations: [{ id: 'operation.heal', version: 1 }],
        capabilities: [{ id: 'capability.vitality', version: 1 }],
      },
      definitions: [actionDefinition],
      exports: [actionDefinition.id],
    }),
  );
}

function assertPreparationFails(
  actionDefinition: ContentDefinition,
  code: string,
): void {
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: 'procedure.invalid-case', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({
        id: 'procedure.consumer',
        version: '1.0.0',
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: procedureSources(actionDefinition),
  });
  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.ok(
    result.diagnostics.some((diagnostic) => diagnostic.code === code),
    JSON.stringify(result.diagnostics),
  );
}

function cyclicProcedure(id: string, target: string): ContentDefinition {
  return {
    kind: 'actionProcedure',
    id,
    ownerPackageId: 'procedure.cycle',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'cycle/procedures.ts', declaration: id },
    parameters: [],
    implementation: {
      kind: 'invocation',
      invocation: {
        procedure: { definitionId: target },
        procedureOwnerPackageId: 'procedure.cycle',
        arguments: {},
      },
    },
  };
}

function withoutArgument(
  values: Readonly<Record<string, unknown>>,
  removed: string,
): Readonly<Record<string, unknown>> {
  return Object.fromEntries(
    Object.entries(values).filter(([key]) => key !== removed),
  );
}

function replaceArgument(
  semantic: unknown,
  argumentId: string,
  value: unknown,
): unknown {
  if (semantic === null || typeof semantic !== 'object' || Array.isArray(semantic)) {
    return semantic;
  }
  const record = semantic as Readonly<Record<string, unknown>>;
  const argumentsValue = record['arguments'];
  if (
    argumentsValue === null ||
    typeof argumentsValue !== 'object' ||
    Array.isArray(argumentsValue)
  ) {
    return semantic;
  }
  return {
    ...record,
    arguments: {
      ...(argumentsValue as Readonly<Record<string, unknown>>),
      [argumentId]: value,
    },
  };
}

function readPath(value: unknown, path: readonly string[]): unknown {
  let current = value;
  for (const field of path) {
    if (
      current === null ||
      typeof current !== 'object' ||
      Array.isArray(current)
    ) {
      return undefined;
    }
    current = (current as Readonly<Record<string, unknown>>)[field];
  }
  return current;
}

type CompilationResult =
  | {
      readonly ok: true;
      readonly artifact: {
        readonly artifactId: string;
        readonly materializedDefinitions: readonly {
          readonly id: string;
          readonly semantic: unknown;
          readonly fingerprint: string;
          readonly [key: string]: unknown;
        }[];
        readonly definitionCommitments: readonly {
          readonly kind: string;
          readonly definitionId: string;
          readonly fingerprint: string;
          readonly stage: {
            readonly value: { readonly semantic: unknown };
            readonly [key: string]: unknown;
          };
          readonly [key: string]: unknown;
        }[];
        readonly [key: string]: unknown;
      };
    }
  | {
      readonly ok: false;
      readonly diagnostics: readonly { readonly code: string }[];
    };

function compilePrepared(prepared: PreparedPlayBundle): CompilationResult {
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
      'compile_play_bundle',
    ],
    { cwd: root, encoding: 'utf8', input: canonicalJson(prepared) },
  );
  assert.equal(compilation.status, 0, compilation.stderr);
  return JSON.parse(compilation.stdout) as CompilationResult;
}
