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
  dice,
  defineActionDefinition,
  defineActionInvocationDefinition,
  defineActionProcedureDefinition,
  defineContentCatalog,
  defineContentPack,
  defineItemDefinition,
  equippedItemAttribute,
  hostile,
  itemBoundedIntegerAttribute,
  itemCatalogReferenceAttribute,
  itemDiceAttribute,
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
    shadow: {
      definitionId: 'damage.shadow',
      category: 'damageType',
      id: 'shadow',
      label: 'Shadow',
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

test('inert item definitions materialize distinct Rust action variants without weapon-local logic', () => {
  const weaponBinding = {
    id: 'weapon',
    requiredTags: ['weapon'],
    requiredTraits: ['melee'],
    slotIds: ['hand.main', 'hand.off'],
  } as const;
  const boundAttack = defineActionInvocationDefinition({
    id: 'action.basic-attack',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: {
      module: 'consumer/actions.ts',
      declaration: 'basicAttack',
    },
    presentation: { label: 'Basic Attack' },
    procedure: basicAttackProcedure,
    importAs: 'foundation',
    binding: weaponBinding,
    arguments: {
      'attack-bonus': constant(2),
      damage: equippedItemAttribute(damageParameter, {
        bindingId: weaponBinding.id,
        attributeId: 'damage',
      }),
      'damage-type': equippedItemAttribute(damageTypeParameter, {
        bindingId: weaponBinding.id,
        attributeId: 'damage-type',
      }),
      defense: rulesetDefense(contractTestRuleset, 'guard'),
      range: equippedItemAttribute(rangeParameter, {
        bindingId: weaponBinding.id,
        attributeId: 'range',
      }),
    },
  });
  const longsword = defineItemDefinition({
    id: 'item.longsword',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: {
      module: 'consumer/items.ts',
      declaration: 'longsword',
    },
    presentation: { label: 'Longsword' },
    item: {
      tags: ['weapon'],
      traits: ['melee'],
      allowedSlots: ['hand.main', 'hand.off'],
      attributes: [
        itemDiceAttribute({ id: 'damage', count: 1, sides: 8 }),
        itemCatalogReferenceAttribute(
          'damage-type',
          damageCatalog.references.force,
        ),
        itemBoundedIntegerAttribute({
          id: 'range',
          value: 1,
          minimum: 1,
          maximum: 12,
        }),
      ],
    },
  });
  const greatsword = defineItemDefinition({
    ...longsword,
    id: 'item.greatsword',
    source: {
      module: 'consumer/items.ts',
      declaration: 'greatsword',
    },
    presentation: { label: 'Greatsword' },
    item: {
      ...longsword.item,
      attributes: [
        itemDiceAttribute({ id: 'damage', count: 2, sides: 6 }),
        itemCatalogReferenceAttribute(
          'damage-type',
          damageCatalog.references.force,
        ),
        itemBoundedIntegerAttribute({
          id: 'range',
          value: 1,
          minimum: 1,
          maximum: 12,
        }),
      ],
    },
  });
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
    requirements: {
      operations: [{ id: 'operation.damage', version: 1 }],
      capabilities: [
        { id: 'capability.defenses', version: 1 },
        { id: 'capability.random', version: 1 },
        { id: 'capability.vitality', version: 1 },
      ],
    },
    definitions: [boundAttack, greatsword, longsword],
    exports: [boundAttack.id, greatsword.id, longsword.id],
  });
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: 'procedure.item-bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({
        id: 'procedure.consumer',
        version: '1.0.0',
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
  if (!compilation.ok) return;
  const variants = compilation.compiledActions.filter(
    (candidate) => candidate.id === boundAttack.id,
  );
  assert.equal(variants.length, 2);
  assert.deepEqual(
    variants.map((variant) => ({
      itemDefinitionId: variant.binding?.itemDefinitionId,
      dice: formulaDiceRequest(variant),
    })),
    [
      {
        itemDefinitionId: 'item.greatsword',
        dice: {
          kind: 'formulaDice',
          count: 2,
          sides: 6,
          path: '$.action.program.body.hit.amount',
        },
      },
      {
        itemDefinitionId: 'item.longsword',
        dice: {
          kind: 'formulaDice',
          count: 1,
          sides: 8,
          path: '$.action.program.body.hit.amount',
        },
      },
    ],
  );
  assert.deepEqual(
    compilation.compiledItems.map((item) => item.definitionId),
    ['item.greatsword', 'item.longsword'],
  );
  assertRustCompilationFails(
    rewritePreparedDefinitionSemantic(
      result.prepared,
      longsword.id,
      (semantic) =>
        updateNestedValue(
          semantic,
          ['attributes', '1', 'value', 'packageId'],
          'procedure.consumer',
        ),
    ),
    'ITEM_CATALOG_REFERENCE_INVALID',
  );
});

test('two distinct actions share one Rust-expanded procedure without copying its body', () => {
  const closeStrike = defineActionInvocationDefinition({
    id: 'action.close-strike',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'consumer/actions.ts', declaration: 'closeStrike' },
    presentation: { label: 'Close strike' },
    procedure: basicAttackProcedure,
    importAs: 'foundation',
    arguments: {
      'attack-bonus': constant(2),
      damage: dice({ count: 1, sides: 6 }),
      'damage-type': damageCatalog.references.force,
      defense: rulesetDefense(contractTestRuleset, 'guard'),
      range: 1,
    },
  });
  const distantStrike = defineActionInvocationDefinition({
    id: 'action.distant-strike',
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: { module: 'consumer/actions.ts', declaration: 'distantStrike' },
    presentation: { label: 'Distant strike' },
    procedure: basicAttackProcedure,
    importAs: 'foundation',
    arguments: {
      'attack-bonus': constant(4),
      damage: dice({ count: 2, sides: 8, bonus: 1 }),
      'damage-type': damageCatalog.references.shadow,
      defense: rulesetDefense(contractTestRuleset, 'resolve'),
      range: 8,
    },
  });
  const prepared = acceptedPreparedActions([closeStrike, distantStrike]);
  const invocations = prepared.materializedDefinitions
    .filter(
      (definition) =>
        definition.id === closeStrike.id ||
        definition.id === distantStrike.id,
    )
    .map((definition) => definition.semantic);
  assert.equal(invocations.length, 2);
  assert.ok(
    invocations.every(
      (semantic) =>
        readPath(semantic, ['procedureId']) === basicAttackProcedure.id,
    ),
  );
  assert.notDeepEqual(
    readPath(invocations[0], ['arguments']),
    readPath(invocations[1], ['arguments']),
  );
  const closeArguments = readPath(invocations[0], ['arguments']);
  const distantArguments = readPath(invocations[1], ['arguments']);
  assert.equal(readPath(closeArguments, ['damage', 'count']), 1);
  assert.equal(readPath(distantArguments, ['damage', 'count']), 2);
  assert.equal(readPath(closeArguments, ['range']), 1);
  assert.equal(readPath(distantArguments, ['range']), 8);
  assert.equal(
    readPath(closeArguments, ['damage-type', 'definitionId']),
    damageCatalog.references.force.definitionId,
  );
  assert.equal(
    readPath(distantArguments, ['damage-type', 'definitionId']),
    damageCatalog.references.shadow.definitionId,
  );
  assert.equal(readPath(closeArguments, ['defense', 'id']), 'guard');
  assert.equal(readPath(distantArguments, ['defense', 'id']), 'resolve');
  const compilation = compilePrepared(prepared);
  assert.equal(compilation.ok, true, JSON.stringify(compilation));
  if (!compilation.ok) return;
  const closeCompiled = compiledAction(compilation, closeStrike.id);
  const distantCompiled = compiledAction(compilation, distantStrike.id);
  assert.equal(closeCompiled.targets.maximumRange, 1);
  assert.equal(distantCompiled.targets.maximumRange, 8);
  assert.equal(closeCompiled.check.defenseId, 'guard');
  assert.equal(distantCompiled.check.defenseId, 'resolve');
  const closeDamageRequest = formulaDiceRequest(closeCompiled);
  const distantDamageRequest = formulaDiceRequest(distantCompiled);
  assert.deepEqual(
    [closeDamageRequest.count, closeDamageRequest.sides],
    [1, 6],
  );
  assert.deepEqual(
    [distantDamageRequest.count, distantDamageRequest.sides],
    [2, 8],
  );

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
          ...basicAttackProcedure.implementation.template.targets,
          team: 'any' as const,
        },
      },
    },
  };
  for (const actionDefinition of [closeStrike, distantStrike]) {
    const baseline = compilePrepared(acceptedPrepared(actionDefinition));
    const changed = compilePrepared(
      acceptedPrepared(actionDefinition, true, changedProcedure),
    );
    assert.equal(baseline.ok, true, JSON.stringify(baseline));
    assert.equal(changed.ok, true, JSON.stringify(changed));
    if (!baseline.ok || !changed.ok) continue;
    assert.notEqual(baseline.artifact.artifactId, changed.artifact.artifactId);
    assert.equal(
      compiledAction(baseline, actionDefinition.id).targets.team,
      'hostile',
    );
    assert.equal(
      compiledAction(changed, actionDefinition.id).targets.team,
      'any',
    );
  }
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

test('direct and composed procedures reject declared parameters they never consume', () => {
  const unusedParameter = { id: 'unused', type: 'boolean' } as const;
  const direct = {
    ...basicAttackProcedure,
    parameters: [...basicAttackProcedure.parameters, unusedParameter],
  } as unknown as ContentDefinition;
  assertProcedurePreparationFails(direct, 'ACTION_PROCEDURE_PARAMETER_UNUSED');

  const composed = {
    ...forwardedProcedure,
    parameters: [...forwardedProcedure.parameters, unusedParameter],
  } as unknown as ContentDefinition;
  assertProcedurePreparationFails(
    composed,
    'ACTION_PROCEDURE_PARAMETER_UNUSED',
  );

  for (const procedureDefinition of [
    basicAttackProcedure,
    forwardedProcedure,
  ]) {
    const result = prepareUninvokedProcedure(procedureDefinition);
    if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
    assertRustCompilationFails(
      rewritePreparedDefinitionSemantic(
        result.prepared,
        procedureDefinition.id,
        (semantic) => appendProcedureParameter(semantic, unusedParameter),
      ),
      'ACTION_PROCEDURE_PARAMETER_UNUSED',
    );
  }
});

test('uninvoked exported procedures are fully validated by TypeScript and Rust', () => {
  const concreteProcedureInput = {
    kind: 'actionProcedure',
    id: 'procedure.concrete-export',
    ownerPackageId: foundationId,
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: {
      module: 'foundation/action-procedures.ts',
      declaration: 'concreteExport',
    },
    parameters: [damageTypeParameter],
    implementation: {
      kind: 'inline',
      template: {
        targets: {
          kind: 'participant',
          team: 'hostile',
          maximumRange: 1,
          maximumTargets: 1,
        },
        check: { kind: 'noRoll' },
        rollScope: 'none',
        costs: [],
        program: {
          kind: 'atomic',
          body: {
            kind: 'onCheck',
            noRoll: {
              kind: 'operation',
              operation: {
                kind: 'damage',
                amount: { kind: 'constant', value: 1 },
                damageType:
                  actionProcedureParameterReference(damageTypeParameter),
              },
            },
          },
        },
      },
    },
  } as const;
  const concreteProcedure =
    concreteProcedureInput as unknown as ContentDefinition;
  const malformedInline = {
    ...concreteProcedureInput,
    implementation: {
      kind: 'inline' as const,
      template: {
        ...concreteProcedureInput.implementation.template,
        targets: {
          ...concreteProcedureInput.implementation.template.targets,
          maximumRange: 'near',
        },
      },
    },
  } as unknown as ContentDefinition;
  assertProcedurePreparationFails(
    malformedInline,
    'ACTION_PROCEDURE_TEMPLATE_INVALID',
  );

  const directResult = prepareUninvokedProcedure(concreteProcedure);
  if (!directResult.ok) assert.fail(JSON.stringify(directResult.diagnostics));
  assertRustCompilationFails(
    rewritePreparedDefinitionSemantic(
      directResult.prepared,
      concreteProcedure.id,
      (semantic) =>
        updateNestedValue(
          semantic,
          ['implementation', 'template', 'targets', 'maximumRange'],
          'near',
        ),
    ),
    'ACTION_PROCEDURE_TEMPLATE_INVALID',
  );

  const semanticallyInvalidProcedure = {
    kind: 'actionProcedure',
    id: 'procedure.semantic-invalid',
    ownerPackageId: foundationId,
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: {
      module: 'foundation/action-procedures.ts',
      declaration: 'semanticInvalid',
    },
    parameters: [],
    implementation: {
      kind: 'inline',
      template: {
        targets: {
          kind: 'participant',
          team: 'hostile',
          maximumRange: 1,
          maximumTargets: 0,
        },
        check: { kind: 'noRoll' },
        rollScope: 'none',
        costs: [],
        program: { kind: 'sequence', steps: [] },
      },
    },
  } as unknown as ContentDefinition;
  const semanticResult = prepareUninvokedProcedure(
    semanticallyInvalidProcedure,
  );
  if (!semanticResult.ok) {
    assert.fail(JSON.stringify(semanticResult.diagnostics));
  }
  const semanticCompilation = compilePrepared(semanticResult.prepared);
  assert.equal(
    semanticCompilation.ok,
    false,
    JSON.stringify(semanticCompilation),
  );
  if (semanticCompilation.ok) return;
  for (const code of [
    'RPG_IR_TARGET_BOUND_INVALID',
    'RPG_IR_EMPTY_SEQUENCE',
    'RPG_IR_ATOMIC_ROOT_REQUIRED',
  ]) {
    assert.ok(
      semanticCompilation.diagnostics.some(
        (diagnostic) => diagnostic.code === code,
      ),
      JSON.stringify(semanticCompilation.diagnostics),
    );
  }

  if (forwardedProcedure.implementation.kind !== 'invocation') {
    assert.fail('fixture procedure must compose through an invocation');
  }
  const missingNestedArgument = {
    ...forwardedProcedure,
    implementation: {
      ...forwardedProcedure.implementation,
      invocation: {
        ...forwardedProcedure.implementation.invocation,
        arguments: withoutArgument(
          forwardedProcedure.implementation.invocation.arguments,
          'range',
        ),
      },
    },
  } as unknown as ContentDefinition;
  assertProcedurePreparationFails(
    missingNestedArgument,
    'ACTION_PROCEDURE_ARGUMENT_MISSING',
  );
  const wrongNestedArgument = {
    ...forwardedProcedure,
    implementation: {
      ...forwardedProcedure.implementation,
      invocation: {
        ...forwardedProcedure.implementation.invocation,
        arguments: {
          ...forwardedProcedure.implementation.invocation.arguments,
          range: 'near',
        },
      },
    },
  } as unknown as ContentDefinition;
  assertProcedurePreparationFails(
    wrongNestedArgument,
    'ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH',
  );
  const wrongNestedOwner = {
    ...forwardedProcedure,
    implementation: {
      ...forwardedProcedure.implementation,
      invocation: {
        ...forwardedProcedure.implementation.invocation,
        procedureOwnerPackageId: 'procedure.impostor',
      },
    },
  } as unknown as ContentDefinition;
  assertProcedurePreparationFails(
    wrongNestedOwner,
    'ACTION_PROCEDURE_REFERENCE_OWNER_MISMATCH',
  );
  const widerRangeParameter = { ...rangeParameter, maximum: 20 } as const;
  const incompatibleForwarding = {
    ...forwardedProcedure,
    parameters: forwardedProcedure.parameters.map((parameter) =>
      parameter.id === rangeParameter.id ? widerRangeParameter : parameter,
    ),
    implementation: actionProcedureInvocation(
      basicAttackProcedure,
      {
        'attack-bonus':
          actionProcedureParameterReference(attackBonusParameter),
        damage: actionProcedureParameterReference(damageParameter),
        'damage-type':
          actionProcedureParameterReference(damageTypeParameter),
        defense: actionProcedureParameterReference(defenseParameter),
        range: actionProcedureParameterReference(widerRangeParameter),
      },
      'foundation',
    ),
  } as unknown as ContentDefinition;
  assertProcedurePreparationFails(
    incompatibleForwarding,
    'ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH',
  );

  const composedResult = prepareUninvokedProcedure(forwardedProcedure);
  if (!composedResult.ok) {
    assert.fail(JSON.stringify(composedResult.diagnostics));
  }
  const rustCases = [
    {
      code: 'ACTION_PROCEDURE_ARGUMENT_MISSING',
      prepared: rewritePreparedDefinitionSemantic(
        composedResult.prepared,
        forwardedProcedure.id,
        (semantic) =>
          removeNestedProperty(semantic, [
            'implementation',
            'arguments',
            'range',
          ]),
      ),
    },
    {
      code: 'ACTION_PROCEDURE_REFERENCE_OWNER_MISMATCH',
      prepared: rewritePreparedDefinitionSemantic(
        composedResult.prepared,
        forwardedProcedure.id,
        (semantic) =>
          updateNestedValue(
            semantic,
            ['implementation', 'procedureOwnerPackageId'],
            'procedure.impostor',
          ),
      ),
    },
    {
      code: 'ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH',
      prepared: rewritePreparedDefinitionSemantic(
        composedResult.prepared,
        forwardedProcedure.id,
        (semantic) =>
          updateNestedValue(
            semantic,
            ['implementation', 'arguments', 'range'],
            'near',
          ),
      ),
    },
    {
      code: 'ACTION_PROCEDURE_ARGUMENT_TYPE_MISMATCH',
      prepared: rewritePreparedDefinitionSemantic(
        composedResult.prepared,
        forwardedProcedure.id,
        (semantic) =>
          updateProcedureParameterMaximum(semantic, rangeParameter.id, 20),
      ),
    },
  ];
  for (const entry of rustCases) {
    assertRustCompilationFails(entry.prepared, entry.code);
  }
});

test('uninvoked procedures reject interacting bounded domains that exceed semantic limits', () => {
  const repeatParameters = ['count-a', 'count-b', 'count-c', 'count-d'].map(
    (id) => ({
      id,
      type: 'boundedInteger',
      minimum: 1,
      maximum: 16,
    }),
  );
  const repeatedDamage = repeatParameters.reduceRight<unknown>(
    (body, parameter) => ({
      kind: 'repeat',
      count: {
        kind: 'parameter',
        parameterId: parameter.id,
        parameterType: parameter.type,
      },
      body,
    }),
    {
      kind: 'operation',
      operation: {
        kind: 'damage',
        amount: { kind: 'constant', value: 1 },
        damageType: actionProcedureParameterReference(damageTypeParameter),
      },
    },
  );
  const interactingProcedure = {
    kind: 'actionProcedure',
    id: 'procedure.interacting-bounds',
    ownerPackageId: foundationId,
    visibility: 'public',
    extensionPolicy: 'sealed',
    source: {
      module: 'foundation/action-procedures.ts',
      declaration: 'interactingBounds',
    },
    parameters: [...repeatParameters, damageTypeParameter],
    implementation: {
      kind: 'inline',
      template: {
        targets: {
          kind: 'participant',
          team: 'hostile',
          maximumRange: 1,
          maximumTargets: 1,
        },
        check: { kind: 'noRoll' },
        rollScope: 'none',
        costs: [],
        program: {
          kind: 'atomic',
          body: {
            kind: 'onCheck',
            noRoll: repeatedDamage,
          },
        },
      },
    },
  } as unknown as ContentDefinition;
  const result = prepareUninvokedProcedure(interactingProcedure);
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  assertRustCompilationFails(
    result.prepared,
    'RPG_IR_PROGRAM_EXPANSION_EXCEEDED',
  );
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
  procedureDefinition = basicAttackProcedure,
): PreparedPlayBundle {
  const sources = includeFoundation
    ? procedureSources(actionDefinition, procedureDefinition)
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

function acceptedPreparedActions(
  actionDefinitions: readonly ContentDefinition[],
  procedureDefinition = basicAttackProcedure,
): PreparedPlayBundle {
  const result = preparePlayBundle({
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
    contentPacks: procedureSourcesForActions(
      actionDefinitions,
      procedureDefinition,
    ),
  });
  if (!result.ok) assert.fail(JSON.stringify(result.diagnostics));
  return result.prepared;
}

function prepareUninvokedProcedure(
  procedureDefinition: ContentDefinition,
): ReturnType<typeof preparePlayBundle> {
  const ownerPackageId =
    procedureDefinition.kind === 'actionProcedure'
      ? procedureDefinition.ownerPackageId
      : foundationId;
  const foundationDefinitions =
    ownerPackageId === foundationId
      ? [...damageCatalog.definitions, procedureDefinition]
      : [...damageCatalog.definitions, basicAttackProcedure];
  const requirements = {
    operations: [{ id: 'operation.damage' as const, version: 1 }],
    capabilities: [
      { id: 'capability.defenses' as const, version: 1 },
      { id: 'capability.random' as const, version: 1 },
      { id: 'capability.vitality' as const, version: 1 },
    ],
  };
  const foundation = defineContentPack({
    identity: { id: foundationId, version: '1.0.0' },
    entry: { module: 'foundation/index.ts', declaration: 'content' },
    requirements,
    definitions: foundationDefinitions,
    exports: foundationDefinitions.map((definition) => definition.id),
  });
  const consumer =
    ownerPackageId === foundationId
      ? undefined
      : defineContentPack({
          identity: { id: ownerPackageId, version: '1.0.0' },
          entry: { module: 'consumer/index.ts', declaration: 'content' },
          dependencies: [
            contentPackDependency({
              id: foundationId,
              version: '1.0.0',
              importAs: 'foundation',
            }),
          ],
          requirements,
          definitions: [procedureDefinition],
          exports: [procedureDefinition.id],
        });
  return preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: 'procedure.uninvoked.bundle', version: '1.0.0' },
      ruleset: contractTestRuleset,
      base: contentPackRequest({
        id: ownerPackageId,
        version: '1.0.0',
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks:
      consumer === undefined
        ? [contentPackSource(foundation)]
        : [contentPackSource(consumer), contentPackSource(foundation)],
  });
}

function procedureSources(
  actionDefinition: ContentDefinition,
  procedureDefinition = basicAttackProcedure,
): readonly ContentPackSource[] {
  return procedureSourcesForActions([actionDefinition], procedureDefinition);
}

function procedureSourcesForActions(
  actionDefinitions: readonly ContentDefinition[],
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
    definitions: actionDefinitions,
    exports: actionDefinitions.map((definition) => definition.id),
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

function assertProcedurePreparationFails(
  procedureDefinition: ContentDefinition,
  code: string,
): void {
  const result = prepareUninvokedProcedure(procedureDefinition);
  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.ok(
    result.diagnostics.some((diagnostic) => diagnostic.code === code),
    JSON.stringify(result.diagnostics),
  );
}

function assertRustCompilationFails(
  prepared: PreparedPlayBundle,
  code: string,
): void {
  const compilation = compilePrepared(prepared);
  assert.equal(compilation.ok, false, JSON.stringify(compilation));
  if (compilation.ok) return;
  assert.ok(
    compilation.diagnostics.some((diagnostic) => diagnostic.code === code),
    JSON.stringify(compilation.diagnostics),
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

function rewritePreparedDefinitionSemantic(
  prepared: PreparedPlayBundle,
  definitionId: string,
  rewrite: (semantic: unknown) => unknown,
): PreparedPlayBundle {
  return {
    ...prepared,
    materializedDefinitions: prepared.materializedDefinitions.map(
      (definition) => {
        if (definition.id !== definitionId) return definition;
        const semantic = rewrite(definition.semantic);
        const { fingerprint: _fingerprint, ...identity } = definition;
        return {
          ...definition,
          semantic,
          fingerprint: stableFingerprint({ ...identity, semantic }),
        };
      },
    ),
    definitionCommitments: prepared.definitionCommitments.map((commitment) => {
      if (
        commitment.kind !== 'concrete' ||
        commitment.definitionId !== definitionId
      ) {
        return commitment;
      }
      const stage = {
        ...commitment.stage,
        value: {
          ...commitment.stage.value,
          semantic: rewrite(commitment.stage.value.semantic),
        },
      };
      return {
        ...commitment,
        stage,
        fingerprint: stableFingerprint(stage),
      };
    }),
  };
}

function updateNestedValue(
  value: unknown,
  path: readonly string[],
  replacement: unknown,
): unknown {
  if (path.length === 0) return replacement;
  if (Array.isArray(value)) {
    const [field, ...remaining] = path;
    const index = Number(field);
    if (!Number.isInteger(index) || index < 0 || index >= value.length) {
      return value;
    }
    return value.map((entry, candidateIndex) =>
      candidateIndex === index
        ? updateNestedValue(entry, remaining, replacement)
        : entry,
    );
  }
  if (!isObjectRecord(value)) return value;
  const [field, ...remaining] = path;
  if (field === undefined) return replacement;
  return {
    ...value,
    [field]: updateNestedValue(value[field], remaining, replacement),
  };
}

function removeNestedProperty(
  value: unknown,
  path: readonly string[],
): unknown {
  if (!isObjectRecord(value) || path.length === 0) return value;
  const [field, ...remaining] = path;
  if (field === undefined) return value;
  if (remaining.length === 0) {
    return Object.fromEntries(
      Object.entries(value).filter(([candidate]) => candidate !== field),
    );
  }
  return {
    ...value,
    [field]: removeNestedProperty(value[field], remaining),
  };
}

function updateProcedureParameterMaximum(
  semantic: unknown,
  parameterId: string,
  maximum: number,
): unknown {
  if (!isObjectRecord(semantic) || !Array.isArray(semantic['parameters'])) {
    return semantic;
  }
  return {
    ...semantic,
    parameters: semantic['parameters'].map((parameter) =>
      isObjectRecord(parameter) && parameter['id'] === parameterId
        ? { ...parameter, maximum }
        : parameter,
    ),
  };
}

function appendProcedureParameter(
  semantic: unknown,
  parameter: unknown,
): unknown {
  if (!isObjectRecord(semantic) || !Array.isArray(semantic['parameters'])) {
    return semantic;
  }
  return {
    ...semantic,
    parameters: [...semantic['parameters'], parameter].sort((left, right) => {
      const leftId = isObjectRecord(left) ? left['id'] : '';
      const rightId = isObjectRecord(right) ? right['id'] : '';
      return String(leftId).localeCompare(String(rightId));
    }),
  };
}

function isObjectRecord(
  value: unknown,
): value is Readonly<Record<string, unknown>> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
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
      readonly compiledActions: readonly CompiledActionProjection[];
      readonly compiledItems: readonly {
        readonly definitionId: string;
      }[];
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

type CompiledActionProjection = {
  readonly id: string;
  readonly targets: {
    readonly team: string;
    readonly maximumRange: number;
    readonly maximumTargets: number;
  };
  readonly check: {
    readonly kind: string;
    readonly defenseId?: string;
  };
  readonly randomPlan: readonly {
    readonly request: {
      readonly kind: string;
      readonly count: number;
      readonly sides: number;
      readonly path: string;
    };
  }[];
  readonly binding?: {
    readonly itemDefinitionId: string;
  };
};

function compiledAction(
  compilation: Extract<CompilationResult, { readonly ok: true }>,
  actionId: string,
): CompiledActionProjection {
  const result = compilation.compiledActions.find(
    (action) => action.id === actionId,
  );
  assert.ok(result, `missing compiled action ${actionId}`);
  return result;
}

function formulaDiceRequest(
  action: CompiledActionProjection,
): CompiledActionProjection['randomPlan'][number]['request'] {
  const result = action.randomPlan.find(
    (entry) => entry.request.kind === 'formulaDice',
  );
  assert.ok(result, `missing formula-dice request for ${action.id}`);
  return result.request;
}

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
