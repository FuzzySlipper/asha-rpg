import assert from "node:assert/strict";
import { test } from "node:test";

import {
  action,
  applyEffect,
  activation,
  actionId,
  add,
  attack,
  constant,
  composePlayBundle,
  contentPackRequest,
  contentPackSource,
  damage,
  defineCharacterClassDefinition,
  defineCharacterFeatureDefinition,
  defineContentCatalog,
  defineEffectDefinition,
  defineContentPack,
  defineParticipantProfileData,
  defineParticipantProfileDefinition,
  defineActionDefinition,
  defineActions,
  definePackage,
  defineRuleset,
  defineScenario,
  defineScenarioTemplate,
  defineSupportDefinition,
  instantiateScenarioTemplate,
  heal,
  half,
  heterogeneousPool,
  hostile,
  onCheck,
  noRoll,
  normalizePackage,
  participantProfileVitality,
  participantProfileStat,
  onOutcome,
  preparePlayBundle,
  readStat,
  rulesetDefense,
  rulesetCalculationSelector,
  rulesetContributionStackingGroup,
  rulesetHeterogeneousPoolProfile,
  rulesetScalarTestProfile,
  rulesetActivationBudget,
  rulesetStat,
  rulesetValueId,
  scalarTest,
  sequence,
  removeEffect,
  dice,
} from "@asha-rpg/authoring";

const semanticRuleset = defineRuleset({
  schema: { identity: "asha.rpg.ruleset", major: 1 },
  identity: { id: "contract.named-values", version: "1.0.0" },
  language: { id: "asha-rpg", version: "1.0.0" },
  models: {
    checks: { id: "check.d20-roll-over", version: 1 },
    turns: { id: "turn.ordered-one-action", version: 1 },
    initiative: { id: "initiative.scenario-ordered", version: 1 },
    reactions: { id: "reaction.before-damage-choice", version: 1 },
    actionEconomy: {
      id: "action-economy.one-action-plus-reaction",
      version: 1,
    },
  },
  provides: {
    operations: [],
    capabilities: [],
    values: [
      {
        kind: "defense",
        id: "armor-class",
        label: "Armor Class",
        numericDomainId: "score",
      },
      {
        kind: "stat",
        id: "strength",
        label: "Strength",
        numericDomainId: "score",
      },
    ],
    numericDomains: [{ id: "score", minimum: 1, maximum: 30 }],
  },
});

test("Ruleset named values are owner-bound ergonomic references", () => {
  const strength = rulesetStat(semanticRuleset, "strength");
  const armorClass = rulesetDefense(semanticRuleset, "armor-class");

  assert.equal(rulesetValueId(strength), "strength");
  assert.equal(rulesetValueId(armorClass), "armor-class");
  assert.equal(strength.rulesetId, semanticRuleset.identity.id);
  assert.equal(Object.isFrozen(strength), true);
  assert.throws(() => rulesetStat(semanticRuleset, "dexterity"));
});

test("Ruleset value ownership survives action authoring and rejects a foreign owner", () => {
  const actionRuleset = defineRuleset({
    ...semanticRuleset,
    provides: {
      ...semanticRuleset.provides,
      operations: [{ id: "operation.heal", version: 1 }],
      capabilities: [
        { id: "capability.defenses", version: 1 },
        { id: "capability.random", version: 1 },
        { id: "capability.stats", version: 1 },
        { id: "capability.vitality", version: 1 },
      ],
    },
  });
  const foreignRuleset = defineRuleset({
    ...actionRuleset,
    identity: { id: "contract.foreign-values", version: "1.0.0" },
  });

  const accepted = prepareRulesetAction(
    actionRuleset,
    rulesetStat(actionRuleset, "strength"),
    rulesetDefense(actionRuleset, "armor-class"),
  );
  assert.equal(
    accepted.ok,
    true,
    accepted.ok
      ? "expected accepted Ruleset owner"
      : JSON.stringify(accepted.diagnostics),
  );

  const rejected = prepareRulesetAction(
    actionRuleset,
    rulesetStat(foreignRuleset, "strength"),
    rulesetDefense(foreignRuleset, "armor-class"),
  );
  assert.equal(rejected.ok, false);
  if (rejected.ok) return;
  assert.deepEqual(
    [...new Set(rejected.diagnostics.map((diagnostic) => diagnostic.code))],
    ["RULESET_VALUE_REFERENCE_OWNER_MISMATCH"],
  );
});

test("Variable activation budgets require exact owner-bound action declarations", () => {
  const variableRuleset = defineRuleset({
    ...semanticRuleset,
    models: {
      ...semanticRuleset.models,
      actionEconomy: {
        id: "action-economy.variable-activation-budgets",
        version: 1,
        acceptedActivationCeiling: 3,
      },
    },
    provides: {
      ...semanticRuleset.provides,
      operations: [{ id: "operation.heal", version: 1 }],
      capabilities: [
        { id: "capability.activation-budgets", version: 1 },
        { id: "capability.vitality", version: 1 },
      ],
      activationBudgets: [
        {
          id: "normal",
          version: 1,
          label: "Normal activations",
          numericDomainId: "activation",
          timing: "action",
          resetBoundary: "ownerTurnStart",
          initialAmount: 3,
        },
      ],
      numericDomains: [
        { id: "activation", minimum: 0, maximum: 3 },
        ...semanticRuleset.provides.numericDomains,
      ],
    },
  });
  const normal = rulesetActivationBudget(variableRuleset, "normal");
  const prepare = (activationDeclaration?: ReturnType<typeof activation>) => {
    const authoredAction = action({
      id: actionId("contract.activation-action"),
      name: "Activation action",
      sourcePath: "contract/activation-action.ts",
      targets: hostile({ range: 1 }),
      check: noRoll(),
      ...(activationDeclaration === undefined
        ? {}
        : { activation: activationDeclaration }),
      program: onCheck({ noRoll: heal({ amount: constant(1) }) }),
    });
    const definition = defineActionDefinition({
      id: authoredAction.id,
      visibility: "public",
      extensionPolicy: "sealed",
      source: {
        module: "contract/activation-action.ts",
        declaration: "action",
      },
      action: authoredAction,
    });
    const contentPack = defineContentPack({
      identity: { id: "contract.activation-content", version: "1.0.0" },
      entry: {
        module: "contract/activation-action.ts",
        declaration: "content",
      },
      definitions: [definition],
    });
    return preparePlayBundle({
      bundle: composePlayBundle({
        identity: { id: "contract.activation-bundle", version: "1.0.0" },
        ruleset: variableRuleset,
        base: contentPackRequest({
          id: contentPack.identity.id,
          version: "1.0.0",
        }),
        add: [],
        overlays: [],
        configure: {},
      }),
      contentPacks: [contentPackSource(contentPack)],
    });
  };

  const accepted = prepare(
    activation({
      timing: "action",
      costs: [{ budget: normal, amount: 2 }],
    }),
  );
  assert.equal(
    accepted.ok,
    true,
    accepted.ok ? "expected activation contract" : JSON.stringify(accepted.diagnostics),
  );
  if (accepted.ok) {
    assert.deepEqual(
      accepted.prepared.ruleset.provides.activationBudgets.map(
        (budget) => budget.id,
      ),
      ["normal"],
    );
  }

  const missing = prepare();
  assert.equal(missing.ok, false);
  if (!missing.ok) {
    assert.ok(
      missing.diagnostics.some(
        (diagnostic) =>
          diagnostic.code === "ACTION_ACTIVATION_MODEL_MISMATCH",
      ),
    );
  }
});

test("Ruleset scalar profiles author canonical ordered outcomes and reject gaps", () => {
  const scalarRuleset = defineRuleset({
    ...semanticRuleset,
    provides: {
      ...semanticRuleset.provides,
      operations: [{ id: "operation.heal", version: 1 }],
      capabilities: [
        { id: "capability.defenses", version: 1 },
        { id: "capability.random", version: 1 },
        { id: "capability.stats", version: 1 },
        { id: "capability.vitality", version: 1 },
      ],
      scalarTestProfiles: [
        {
          id: "graded",
          version: 1,
          label: "Graded test",
          numericDomainId: "score",
          dieSides: 20,
          contributionSelectorId: null,
          bands: [
            { id: "failure", label: "Failure" },
            { id: "success", label: "Success" },
          ],
          marginRules: [
            { minimum: null, maximum: -1, bandId: "failure" },
            { minimum: 0, maximum: null, bandId: "success" },
          ],
          naturalDieRules: [
            {
              id: "natural-high",
              minimum: 20,
              maximum: 20,
              effect: { kind: "setBand", bandId: "success" },
            },
          ],
        },
      ],
    },
  });
  const result = prepareScalarRulesetAction(scalarRuleset);
  assert.equal(
    result.ok,
    true,
    result.ok ? "expected scalar profile" : JSON.stringify(result.diagnostics),
  );
  if (!result.ok) return;
  assert.equal(result.prepared.schema.major, 12);
  assert.deepEqual(
    result.prepared.ruleset.provides.scalarTestProfiles.map(
      (profile) => profile.id,
    ),
    ["graded"],
  );
  const actionDefinition = result.prepared.materializedDefinitions.find(
    (definition) => definition.id === "contract.scalar-action",
  );
  assert.equal(
    (actionDefinition?.semantic as { action?: { check?: { kind?: string } } })
      .action?.check?.kind,
    "scalarTest",
  );

  const validScalarCheck = scalarTest({
    profile: rulesetScalarTestProfile(scalarRuleset, "graded"),
    base: readStat("actor", rulesetStat(scalarRuleset, "strength")),
    difficulty: {
      kind: "targetDefense",
      defense: rulesetDefense(scalarRuleset, "armor-class"),
    },
  });
  if (false) {
    scalarTest({
      profile: rulesetScalarTestProfile(scalarRuleset, "graded"),
      // @ts-expect-error scalar-test base expressions cannot contain dice
      base: dice({ count: 1, sides: 6 }),
      difficulty: {
        kind: "targetDefense",
        defense: rulesetDefense(scalarRuleset, "armor-class"),
      },
    });
    scalarTest({
      profile: rulesetScalarTestProfile(scalarRuleset, "graded"),
      base: constant(1),
      difficulty: {
        kind: "explicit",
        // @ts-expect-error nested dice remain outside scalar-test expressions
        value: add(constant(10), half(dice({ count: 1, sides: 4 }))),
      },
    });
  }
  const invalidChecks = [
    {
      ...validScalarCheck,
      base: dice({ count: 1, sides: 6 }),
    },
    {
      ...validScalarCheck,
      difficulty: {
        kind: "explicit",
        value: add(constant(10), half(dice({ count: 1, sides: 4 }))),
      },
    },
  ] as unknown as readonly ReturnType<typeof scalarTest>[];
  for (const invalidCheck of invalidChecks) {
    const invalidAction = scalarRulesetAction(scalarRuleset, invalidCheck);
    const normalized = normalizePackage(
      definePackage({
        id: "contract.invalid-scalar",
        version: "1.0.0",
        sources: [defineActions("invalid-scalar", [invalidAction])],
      }),
    );
    assert.equal(normalized.ok, false);
    if (!normalized.ok) {
      assert.ok(
        normalized.diagnostics.some(
          (diagnostic) =>
            diagnostic.code ===
            "normalization.scalarTestRandomFormulaInvalid",
        ),
      );
    }
    const prepared = prepareScalarRulesetAction(scalarRuleset, invalidCheck);
    assert.equal(prepared.ok, false);
    if (!prepared.ok) {
      assert.ok(
        prepared.diagnostics.some(
          (diagnostic) =>
            diagnostic.code ===
            "normalization.scalarTestRandomFormulaInvalid",
        ),
      );
    }
  }

  const scalarProfile = scalarRuleset.provides.scalarTestProfiles[0];
  if (scalarProfile === undefined) {
    throw new Error("expected scalar profile fixture");
  }
  const malformedRuleset = defineRuleset({
    ...scalarRuleset,
    provides: {
      ...scalarRuleset.provides,
      scalarTestProfiles: [
        {
          ...scalarProfile,
          marginRules: [
            { minimum: null, maximum: -1, bandId: "failure" },
            { minimum: 1, maximum: null, bandId: "success" },
          ],
        },
      ],
    },
  });
  const malformed = prepareScalarRulesetAction(malformedRuleset);
  assert.equal(malformed.ok, false);
  if (malformed.ok) return;
  assert.ok(
    malformed.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "RULESET_SCALAR_TEST_MARGIN_RULE_INVALID",
    ),
  );

  const overflowRuleset = defineRuleset({
    ...scalarRuleset,
    provides: {
      ...scalarRuleset.provides,
      numericDomains: [
        { id: "score", minimum: -1, maximum: 2_147_483_647 },
      ],
    },
  });
  const overflow = prepareScalarRulesetAction(overflowRuleset);
  assert.equal(overflow.ok, false);
  if (overflow.ok) return;
  assert.ok(
    overflow.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "RULESET_SCALAR_TEST_DOMAIN_OVERFLOW",
    ),
  );
});

test("Heterogeneous pool profiles and content contributions retain typed owner-bound contracts", () => {
  const poolRuleset = defineRuleset({
    ...semanticRuleset,
    provides: {
      ...semanticRuleset.provides,
      operations: [{ id: "operation.heal", version: 1 }],
      capabilities: [
        { id: "capability.random", version: 1 },
        { id: "capability.vitality", version: 1 },
      ],
      contributionStackingGroups: [
        {
          id: "pool-sum",
          version: 1,
          label: "Pool sum",
          policy: "sum",
        },
      ],
      heterogeneousPoolProfiles: [
        {
          id: "story",
          version: 1,
          label: "Story pool",
          dieTypes: [
            {
              id: "boost",
              label: "Boost",
              sides: 4,
              faces: [
                { value: 1, vector: [] },
                {
                  value: 2,
                  vector: [{ axisId: "success", value: 1 }],
                },
                {
                  value: 3,
                  vector: [{ axisId: "success", value: 1 }],
                },
                {
                  value: 4,
                  vector: [{ axisId: "success", value: 2 }],
                },
              ],
            },
          ],
          axes: [{ id: "success", label: "Success" }],
          cancellations: [],
          bands: [
            { id: "failure", label: "Failure" },
            { id: "success", label: "Success" },
          ],
          outcomeRules: [
            {
              id: "success",
              bandId: "success",
              requirements: [
                { axisId: "success", minimum: 1, maximum: null },
              ],
            },
          ],
          defaultBandId: "failure",
        },
      ],
    },
  });
  const profile = rulesetHeterogeneousPoolProfile(poolRuleset, "story");
  const group = rulesetContributionStackingGroup(poolRuleset, "pool-sum");
  const authoredAction = action({
    id: actionId("contract.pool-action"),
    name: "Pool action",
    sourcePath: "contract/pool.ts#poolAction",
    targets: hostile({ range: 1 }),
    check: heterogeneousPool({
      profile,
      baseDice: [{ dieTypeId: "boost", count: 1 }],
      automaticAxes: [],
    }),
    rollScope: "shared",
    program: onOutcome({
      branches: { success: heal({ amount: constant(2) }) },
      default: heal({ amount: constant(1) }),
    }),
  });
  const actionDefinition = defineActionDefinition({
    id: authoredAction.id,
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/pool.ts", declaration: "poolAction" },
    action: authoredAction,
  });
  const poolFeature = defineCharacterFeatureDefinition({
    id: "feature.pool",
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/pool.ts", declaration: "poolFeature" },
    presentation: { label: "Pool feature" },
    characterFeature: {
      poolContributions: [
        {
          id: "z-add-success",
          profile,
          stackingGroup: group,
          effect: { kind: "addAxis", axisId: "success", value: 1 },
          predicate: { kind: "always" },
        },
        {
          id: "add-boost",
          profile,
          stackingGroup: group,
          effect: { kind: "addDice", dieTypeId: "boost", delta: 1 },
          predicate: { kind: "always" },
        },
      ],
    },
  });
  const contentPack = defineContentPack({
    identity: { id: "contract.pool-content", version: "1.0.0" },
    entry: { module: "contract/pool.ts", declaration: "content" },
    definitions: [actionDefinition, poolFeature],
  });
  const prepare = (ruleset: typeof poolRuleset) =>
    preparePlayBundle({
      bundle: composePlayBundle({
        identity: { id: "contract.pool-bundle", version: "1.0.0" },
        ruleset,
        base: contentPackRequest({
          id: contentPack.identity.id,
          version: contentPack.identity.version,
        }),
        add: [],
        overlays: [],
        configure: {},
      }),
      contentPacks: [contentPackSource(contentPack)],
    });
  const accepted = prepare(poolRuleset);
  assert.equal(
    accepted.ok,
    true,
    accepted.ok
      ? "expected heterogeneous pool contract"
      : JSON.stringify(accepted.diagnostics),
  );
  if (!accepted.ok) return;
  const materializedFeature = accepted.prepared.materializedDefinitions.find(
    (definition) => definition.id === poolFeature.id,
  );
  assert.deepEqual(
    (
      materializedFeature?.semantic as {
        poolContributions?: readonly {
          profile: { rulesetId: string; id: string };
          effect: { kind: string; dieTypeId: string; delta: number };
        }[];
      }
    ).poolContributions,
    [
      {
        schema: { identity: "asha.rpg.pool-contribution", version: 2 },
        id: "add-boost",
        subject: "actor",
        profile: { rulesetId: poolRuleset.identity.id, id: "story" },
        stackingGroup: {
          rulesetId: poolRuleset.identity.id,
          id: "pool-sum",
        },
        effect: { kind: "addDice", dieTypeId: "boost", delta: 1 },
        predicate: { kind: "always" },
      },
      {
        schema: { identity: "asha.rpg.pool-contribution", version: 2 },
        id: "z-add-success",
        subject: "actor",
        profile: { rulesetId: poolRuleset.identity.id, id: "story" },
        stackingGroup: {
          rulesetId: poolRuleset.identity.id,
          id: "pool-sum",
        },
        effect: { kind: "addAxis", axisId: "success", value: 1 },
        predicate: { kind: "always" },
      },
    ],
  );

  const profileDefinition =
    poolRuleset.provides.heterogeneousPoolProfiles[0];
  if (profileDefinition === undefined) {
    throw new Error("expected heterogeneous pool profile fixture");
  }
  const dieType = profileDefinition.dieTypes[0];
  if (dieType === undefined) {
    throw new Error("expected heterogeneous pool die fixture");
  }
  const malformed = prepare(
    defineRuleset({
      ...poolRuleset,
      provides: {
        ...poolRuleset.provides,
        heterogeneousPoolProfiles: [
          {
            ...profileDefinition,
            dieTypes: [
              {
                ...dieType,
                faces: dieType.faces.slice(0, 3),
              },
            ],
          },
        ],
      },
    }),
  );
  assert.equal(malformed.ok, false);
  if (!malformed.ok) {
    assert.ok(
      malformed.diagnostics.some(
        (diagnostic) =>
          diagnostic.code === "RULESET_POOL_DIE_TYPE_INVALID",
      ),
    );
  }
});

test("typed damage packets and responses retain owned catalog references and canonical order", () => {
  const damageRuleset = defineRuleset({
    ...semanticRuleset,
    provides: {
      ...semanticRuleset.provides,
      operations: [{ id: "operation.damage", version: 2 }],
      capabilities: [{ id: "capability.vitality", version: 1 }],
    },
  });
  const catalogs = defineContentCatalog({
    packageId: "contract.damage-content",
    sourceModule: "contract/damage-catalog.ts",
    entries: {
      cold: {
        definitionId: "catalog.damage.cold",
        category: "damageType",
        id: "cold",
        label: "Cold",
      },
      fire: {
        definitionId: "catalog.damage.fire",
        category: "damageType",
        id: "fire",
        label: "Fire",
      },
    },
  });
  const authoredAction = action({
    id: actionId("action.damage-packet"),
    name: "Damage packet",
    sourcePath: "contract/damage.ts#packet",
    targets: hostile({ range: 3 }),
    check: noRoll(),
    program: onCheck({
      noRoll: damage({
        parts: [
          {
            id: "fire",
            amount: constant(7),
            type: catalogs.references.fire,
            tags: ["weapon", "magic"],
          },
          {
            id: "cold",
            amount: constant(3),
            type: catalogs.references.cold,
            tags: ["magic"],
          },
        ],
      }),
    }),
  });
  const actionDefinition = defineActionDefinition({
    id: authoredAction.id,
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/damage.ts", declaration: "packet" },
    action: authoredAction,
  });
  const responseFeature = defineCharacterFeatureDefinition({
    id: "feature.damage-responses",
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/damage.ts", declaration: "responses" },
    presentation: { label: "Damage responses" },
    characterFeature: {
      damageResponses: [
        {
          id: "fire-half",
          damageType: catalogs.references.fire,
          requiredTags: ["weapon", "magic"],
          bypassTags: ["penetrating", "adamantine"],
          effect: { kind: "scale", numerator: 1, denominator: 2 },
        },
        {
          id: "cold-immune",
          damageType: catalogs.references.cold,
          requiredTags: [],
          bypassTags: [],
          effect: { kind: "immune" },
        },
      ],
    },
  });
  const contentPack = defineContentPack({
    identity: { id: "contract.damage-content", version: "1.0.0" },
    entry: { module: "contract/damage.ts", declaration: "content" },
    requirements: {
      operations: [{ id: "operation.damage", version: 2 }],
      capabilities: [{ id: "capability.vitality", version: 1 }],
    },
    definitions: [
      ...catalogs.definitions,
      actionDefinition,
      responseFeature,
    ],
  });
  const prepare = (
    feature: typeof responseFeature = responseFeature,
  ) => {
    const source = defineContentPack({
      ...contentPack,
      definitions: [
        ...catalogs.definitions,
        actionDefinition,
        feature,
      ],
    });
    return preparePlayBundle({
      bundle: composePlayBundle({
        identity: { id: "contract.damage-bundle", version: "1.0.0" },
        ruleset: damageRuleset,
        base: contentPackRequest({
          id: source.identity.id,
          version: source.identity.version,
        }),
        add: [],
        overlays: [],
        configure: {},
      }),
      contentPacks: [contentPackSource(source)],
    });
  };

  const accepted = prepare();
  assert.equal(
    accepted.ok,
    true,
    accepted.ok ? "expected damage contract" : JSON.stringify(accepted.diagnostics),
  );
  if (!accepted.ok) return;
  const materializedAction = accepted.prepared.materializedDefinitions.find(
    (definition) => definition.id === actionDefinition.id,
  );
  const materializedFeature = accepted.prepared.materializedDefinitions.find(
    (definition) => definition.id === responseFeature.id,
  );
  assert.deepEqual(
    (
      materializedAction?.semantic as {
        action: {
          program: {
            body: {
              noRoll: {
                operation: {
                  parts: readonly {
                    id: string;
                    damageType: string;
                    tags: readonly string[];
                  }[];
                };
              };
            };
          };
        };
      }
    ).action.program.body.noRoll.operation.parts,
    [
      {
        id: "cold",
        amount: { kind: "constant", value: 3 },
        damageType: "catalog.damage.cold",
        tags: ["magic"],
      },
      {
        id: "fire",
        amount: { kind: "constant", value: 7 },
        damageType: "catalog.damage.fire",
        tags: ["magic", "weapon"],
      },
    ],
  );
  assert.deepEqual(
    (
      materializedFeature?.semantic as {
        damageResponses: readonly unknown[];
      }
    ).damageResponses,
    [
      {
        schema: { identity: "asha.rpg.damage-response", version: 1 },
        id: "cold-immune",
        damageTypeId: "catalog.damage.cold",
        requiredTags: [],
        bypassTags: [],
        effect: { kind: "immune" },
      },
      {
        schema: { identity: "asha.rpg.damage-response", version: 1 },
        id: "fire-half",
        damageTypeId: "catalog.damage.fire",
        requiredTags: ["magic", "weapon"],
        bypassTags: ["adamantine", "penetrating"],
        effect: { kind: "scale", numerator: 1, denominator: 2 },
      },
    ],
  );
  assert.deepEqual(
    materializedAction?.references,
    ["catalog.damage.cold", "catalog.damage.fire"],
  );
  assert.deepEqual(
    materializedFeature?.references,
    ["catalog.damage.cold", "catalog.damage.fire"],
  );

  const duplicate = prepare(defineCharacterFeatureDefinition({
    ...responseFeature,
    characterFeature: {
      damageResponses: [
        {
          id: "duplicate",
          damageType: catalogs.references.fire,
          requiredTags: [],
          bypassTags: [],
          effect: { kind: "flat", value: -1 },
        },
        {
          id: "duplicate",
          damageType: catalogs.references.fire,
          requiredTags: [],
          bypassTags: [],
          effect: { kind: "flat", value: -2 },
        },
      ],
    },
  }));
  assert.equal(duplicate.ok, false);
  if (!duplicate.ok) {
    assert.ok(
      duplicate.diagnostics.some(
        (diagnostic) =>
          diagnostic.code === "DAMAGE_RESPONSES_NOT_CANONICAL",
      ),
    );
  }

  const invalidScale = prepare(defineCharacterFeatureDefinition({
    ...responseFeature,
    characterFeature: {
      damageResponses: [{
        id: "invalid-scale",
        damageType: catalogs.references.fire,
        requiredTags: [],
        bypassTags: [],
        effect: { kind: "scale", numerator: 1_001, denominator: 1 },
      }],
    },
  }));
  assert.equal(invalidScale.ok, false);
  if (!invalidScale.ok) {
    assert.ok(
      invalidScale.diagnostics.some(
        (diagnostic) =>
          diagnostic.code === "DAMAGE_RESPONSE_SCALE_INVALID",
      ),
    );
  }
});

test("Content Pack requirements are checked directly against Ruleset provisions", () => {
  const contentPack = defineContentPack({
    identity: { id: "contract.incompatible-content", version: "1.0.0" },
    entry: { module: "contract/content.ts", declaration: "content" },
    requirements: {
      values: [{ kind: "stat", id: "dexterity" }],
    },
    definitions: [],
  });
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.incompatible-bundle", version: "1.0.0" },
      ruleset: semanticRuleset,
      base: contentPackRequest({
        id: contentPack.identity.id,
        version: "1.0.0",
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(contentPack)],
  });

  assert.equal(result.ok, false);
  if (result.ok) return;
  assert.deepEqual(
    result.diagnostics.map((diagnostic) => diagnostic.code),
    ["CONTENT_PACK_VALUE_REQUIREMENT_MISSING"],
  );
});

test("Content Packs carry typed class, feature, and participant setup definitions", () => {
  const profileRuleset = defineRuleset({
    ...semanticRuleset,
    provides: {
      ...semanticRuleset.provides,
      operations: [{ id: "operation.heal", version: 1 }],
      capabilities: [
        { id: "capability.defenses", version: 1 },
        { id: "capability.random", version: 1 },
        { id: "capability.stats", version: 1 },
        { id: "capability.vitality", version: 1 },
      ],
      calculationSelectors: [
        {
          id: "attack-total",
          version: 1,
          label: "Attack total",
          numericDomainId: "score",
        },
      ],
      contributionStackingGroups: [
        {
          id: "circumstance",
          version: 1,
          label: "Circumstance",
          policy: "sum",
        },
      ],
    },
  });
  const attackSelector = rulesetCalculationSelector(
    profileRuleset,
    "attack-total",
  );
  const circumstanceGroup = rulesetContributionStackingGroup(
    profileRuleset,
    "circumstance",
  );
  const authoredAction = action({
    id: actionId("action.profile-heal"),
    name: "Profile heal",
    sourcePath: "contract/profiles.ts#profileHeal",
    tags: ["attack"],
    targets: hostile({ range: 1 }),
    check: attack({
      modifier: constant(1),
      defense: rulesetDefense(profileRuleset, "armor-class"),
      contributionSelector: attackSelector,
    }),
    rollScope: "perTarget",
    program: onCheck({ hit: heal({ amount: constant(1) }) }),
  });
  const actionDefinition = defineActionDefinition({
    id: authoredAction.id,
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/profiles.ts", declaration: "profileHeal" },
    action: authoredAction,
  });
  const highGround = defineSupportDefinition({
    id: "terrain.high-ground",
    visibility: "private",
    extensionPolicy: "sealed",
    source: {
      module: "contract/profiles.ts",
      declaration: "highGround",
    },
    presentation: { label: "High ground" },
    semantic: {
      catalog: "cell-capability",
      id: "terrain.high-ground",
      data: { kind: "flag" },
    },
  });
  const flankingFeature = defineCharacterFeatureDefinition({
    id: "feature.flanking",
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/profiles.ts", declaration: "flanking" },
    presentation: { label: "Flanking Discipline" },
    characterFeature: {
      contributions: [
        {
          id: "cell",
          selector: attackSelector,
          stackingGroup: circumstanceGroup,
          value: { kind: "constant", value: 1 },
          predicate: {
            kind: "cellCapability",
            subject: "actor",
            capability: { definitionId: highGround.id },
          },
        },
        {
          id: "flanking",
          selector: attackSelector,
          stackingGroup: circumstanceGroup,
          value: { kind: "constant", value: 2 },
          predicate: { kind: "actorFlanksTarget" },
        },
      ],
    },
  });
  const vanguardClass = defineCharacterClassDefinition({
    id: "class.vanguard",
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/profiles.ts", declaration: "vanguardClass" },
    presentation: { label: "Vanguard" },
    characterClass: {
      featureDefinitions: [{ definitionId: flankingFeature.id }],
    },
  });
  const profile = defineParticipantProfileDefinition({
    id: "profile.vanguard",
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/profiles.ts", declaration: "vanguard" },
    presentation: { label: "Vanguard" },
    profileId: "vanguard",
    profile: defineParticipantProfileData({
      role: "player",
      definitionReferences: [{ definitionId: actionDefinition.id }],
      classDefinition: { definitionId: vanguardClass.id },
      featureDefinitions: [{ definitionId: flankingFeature.id }],
      capabilities: [
        participantProfileVitality({ current: 10, max: 10 }),
        participantProfileStat(rulesetStat(profileRuleset, "strength"), 16),
      ],
    }),
  });
  const contentPack = defineContentPack({
    identity: { id: "contract.profile-content", version: "1.0.0" },
    entry: { module: "contract/profiles.ts", declaration: "content" },
    definitions: [
      actionDefinition,
      flankingFeature,
      highGround,
      vanguardClass,
      profile,
    ],
  });
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.profile-bundle", version: "1.0.0" },
      ruleset: profileRuleset,
      base: contentPackRequest({
        id: contentPack.identity.id,
        version: "1.0.0",
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(contentPack)],
  });

  assert.equal(result.ok, true);
  if (!result.ok) return;
  const materializedProfile = result.prepared.materializedDefinitions.find(
    (definition) => definition.id === profile.id,
  );
  const materializedSemantic = materializedProfile?.semantic as
    | { readonly data: unknown }
    | undefined;
  assert.deepEqual(materializedSemantic?.data, {
    schema: { identity: "asha.rpg.participant-profile", version: 2 },
    role: "player",
    definitionIds: [actionDefinition.id],
    classDefinitionId: vanguardClass.id,
    featureDefinitionIds: [flankingFeature.id],
    items: [],
    equipment: [],
    capabilities: [
      { owner: "vitality", value: { current: 10, max: 10 } },
      { owner: "stat", id: "strength", value: 16 },
    ],
  });
  const materializedFeature = result.prepared.materializedDefinitions.find(
    (definition) => definition.id === flankingFeature.id,
  );
  assert.deepEqual(materializedFeature?.references, [highGround.id]);
  assert.equal(
    (
      materializedFeature?.semantic as {
        readonly contributions: readonly [
          { readonly predicate: { readonly capabilityId: string } },
        ];
      }
    ).contributions[0].predicate.capabilityId,
    highGround.id,
  );
  assert.deepEqual(result.prepared.contentRequirements.values, [
    { kind: "defense", id: "armor-class" },
    { kind: "stat", id: "strength" },
  ]);
  assert.deepEqual(result.prepared.contentRequirements.numericDomains, ["score"]);

  const invalidFeature = defineCharacterFeatureDefinition({
    ...flankingFeature,
    id: "feature.invalid-surrounded",
    extensionPolicy: "patchable",
    source: {
      module: "contract/profiles.ts",
      declaration: "invalidSurrounded",
    },
    characterFeature: {
      contributions: [
        {
          id: "duplicate",
          selector: attackSelector,
          stackingGroup: circumstanceGroup,
          value: { kind: "constant", value: 1 },
          predicate: { kind: "always" },
        },
        {
          id: "duplicate",
          selector: attackSelector,
          stackingGroup: circumstanceGroup,
          value: { kind: "constant", value: 1 },
          predicate: {
            kind: "actorSurrounded",
            minimumHostiles: 5,
          },
        },
      ],
    },
  });
  const invalidContent = defineContentPack({
    identity: { id: "contract.invalid-feature-content", version: "1.0.0" },
    entry: {
      module: "contract/profiles.ts",
      declaration: "invalidFeatureContent",
    },
    definitions: [invalidFeature],
  });
  const invalidResult = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.invalid-feature-bundle", version: "1.0.0" },
      ruleset: profileRuleset,
      base: contentPackRequest({
        id: invalidContent.identity.id,
        version: "1.0.0",
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(invalidContent)],
  });
  assert.equal(invalidResult.ok, false);
  if (!invalidResult.ok) {
    assert.ok(invalidResult.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "CHARACTER_FEATURE_EXTENSION_POLICY_UNSUPPORTED",
    ));
    assert.ok(invalidResult.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "SCALAR_CONTRIBUTION_SURROUNDED_THRESHOLD_INVALID",
    ));
    assert.ok(invalidResult.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "SCALAR_CONTRIBUTIONS_NOT_CANONICAL",
    ));
  }

  const invalidContractFeature = defineCharacterFeatureDefinition({
    id: "feature.invalid-contract",
    visibility: "public",
    extensionPolicy: "sealed",
    source: {
      module: "contract/profiles.ts",
      declaration: "invalidContract",
    },
    presentation: { label: "Invalid contract" },
    characterFeature: {
      contributions: [
        {
          id: "invalid-contract",
          selector: {
            rulesetId: profileRuleset.identity.id,
            id: "missing-selector",
          },
          stackingGroup: {
            rulesetId: profileRuleset.identity.id,
            id: "missing-group",
          },
          value: { kind: "constant", value: 1 },
          predicate: { kind: "always" },
        },
      ],
    },
  });
  const invalidContractContent = defineContentPack({
    identity: { id: "contract.invalid-contract-content", version: "1.0.0" },
    entry: {
      module: "contract/profiles.ts",
      declaration: "invalidContractContent",
    },
    definitions: [invalidContractFeature],
  });
  const invalidContractResult = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.invalid-contract-bundle", version: "1.0.0" },
      ruleset: profileRuleset,
      base: contentPackRequest({
        id: invalidContractContent.identity.id,
        version: "1.0.0",
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(invalidContractContent)],
  });
  assert.equal(invalidContractResult.ok, false);
  if (!invalidContractResult.ok) {
    assert.ok(invalidContractResult.diagnostics.some(
      (diagnostic) => diagnostic.code === "SCALAR_CONTRIBUTION_SELECTOR_INVALID",
    ));
    assert.ok(invalidContractResult.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "SCALAR_CONTRIBUTION_STACKING_GROUP_INVALID",
    ));
  }

  const invalidPolicyRuleset = defineRuleset({
    ...profileRuleset,
    provides: {
      ...profileRuleset.provides,
      contributionStackingGroups: [
        {
          id: "circumstance",
          version: 1,
          label: "Circumstance",
          policy: "median" as "sum",
        },
      ],
    },
  });
  const invalidPolicyResult = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.invalid-policy-bundle", version: "1.0.0" },
      ruleset: invalidPolicyRuleset,
      base: contentPackRequest({
        id: contentPack.identity.id,
        version: "1.0.0",
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(contentPack)],
  });
  assert.equal(invalidPolicyResult.ok, false);
  if (!invalidPolicyResult.ok) {
    assert.ok(invalidPolicyResult.diagnostics.some(
      (diagnostic) =>
        diagnostic.code === "RULESET_CONTRIBUTION_STACKING_GROUP_INVALID",
    ));
  }

  const foreignRuleset = defineRuleset({
    ...profileRuleset,
    identity: { id: "contract.foreign-profile", version: "1.0.0" },
  });
  const foreignProfile = defineParticipantProfileDefinition({
    ...profile,
    profileId: "foreign-vanguard",
    profile: defineParticipantProfileData({
      role: "player",
      definitionReferences: [{ definitionId: actionDefinition.id }],
      capabilities: [
        participantProfileVitality({ current: 10, max: 10 }),
        participantProfileStat(rulesetStat(foreignRuleset, "strength"), 16),
      ],
    }),
  });
  const foreignContent = defineContentPack({
    ...contentPack,
    identity: { id: "contract.foreign-profile-content", version: "1.0.0" },
    definitions: [actionDefinition, foreignProfile],
  });
  const foreignResult = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.foreign-profile-bundle", version: "1.0.0" },
      ruleset: profileRuleset,
      base: contentPackRequest({ id: foreignContent.identity.id, version: "1.0.0" }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(foreignContent)],
  });
  assert.equal(foreignResult.ok, false);
  if (!foreignResult.ok) {
    assert.ok(foreignResult.diagnostics.some(
      (diagnostic) => diagnostic.code === "RULESET_VALUE_REFERENCE_OWNER_MISMATCH",
    ));
  }
});

test("named effects materialize closed typed lifecycle definitions and operations", () => {
  const ruleset = defineRuleset({
    ...semanticRuleset,
    provides: {
      ...semanticRuleset.provides,
      operations: [
        { id: "operation.applyEffect", version: 1 },
        { id: "operation.removeEffect", version: 1 },
      ],
      capabilities: [{ id: "capability.effects", version: 1 }],
      calculationSelectors: [
        {
          id: "effect-total",
          version: 1,
          label: "Effect total",
          numericDomainId: "score",
        },
      ],
      contributionStackingGroups: [
        {
          id: "effect-stack",
          version: 1,
          label: "Effect stack",
          policy: "sum",
        },
      ],
    },
  });
  const selector = rulesetCalculationSelector(ruleset, "effect-total");
  const stackingGroup = rulesetContributionStackingGroup(
    ruleset,
    "effect-stack",
  );
  const effect = defineEffectDefinition({
    id: "effect.focused",
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/effects.ts", declaration: "focused" },
    presentation: { label: "Focused" },
    effect: {
      rankMinimum: 1,
      rankMaximum: 4,
      stackingId: "focus",
      stacking: "refresh",
      tenure: { kind: "targetTurnEndSave" },
      condition: {
        clauses: [
          { kind: "forbidMovement" },
          { kind: "requireActionTag", actionTag: "focus" },
          { kind: "forbidActionTag", actionTag: "restricted" },
        ],
      },
      contributions: [
        {
          id: "focused-bonus",
          subject: "target",
          selector,
          stackingGroup,
          value: { kind: "constant", value: 1 },
          predicate: {
            kind: "effectActive",
            subject: "actor",
            definition: { definitionId: "effect.focused" },
          },
        },
      ],
    },
  });
  const authored = action({
    id: actionId("action.focus"),
    name: "Focus",
    sourcePath: "contract/effects.ts#focus",
    targets: hostile({ range: 1 }),
    check: noRoll(),
    tags: ["focus", "restricted"],
    program: onCheck({
      noRoll: sequence(
        applyEffect({
          effect: { definitionId: effect.id },
          rank: constant(2),
        }),
        removeEffect({ effect: { definitionId: effect.id } }),
      ),
    }),
  });
  const actionDefinition = defineActionDefinition({
    id: authored.id,
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/effects.ts", declaration: "focus" },
    action: authored,
  });
  const content = defineContentPack({
    identity: { id: "contract.effects", version: "1.0.0" },
    entry: { module: "contract/effects.ts", declaration: "content" },
    requirements: {
      operations: [
        { id: "operation.applyEffect", version: 1 },
        { id: "operation.removeEffect", version: 1 },
      ],
      capabilities: [{ id: "capability.effects", version: 1 }],
      numericDomains: ["score"],
    },
    definitions: [actionDefinition, effect],
    exports: [actionDefinition.id, effect.id],
  });
  const result = preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.effects", version: "1.0.0" },
      ruleset,
      base: contentPackRequest({ id: content.identity.id, version: "1.0.0" }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(content)],
  });
  assert.equal(
    result.ok,
    true,
    result.ok ? "expected effect bundle" : JSON.stringify(result.diagnostics),
  );
  if (!result.ok) return;
  const materialized = result.prepared.materializedDefinitions.find(
    (definition) => definition.id === effect.id,
  );
  assert.equal(materialized?.kind, "effect");
  assert.deepEqual(materialized?.references, []);
  assert.deepEqual(
    (materialized?.semantic as {
      contributions: { predicate: unknown; subject: string }[];
    }).contributions[0]?.predicate,
    {
      kind: "effectActive",
      subject: "actor",
      definitionId: effect.id,
    },
  );
  assert.equal(
    (materialized?.semantic as {
      contributions: { subject: string }[];
    }).contributions[0]?.subject,
    "target",
  );
  assert.deepEqual(
    (materialized?.semantic as {
      tenure: unknown;
      condition: unknown;
    }).tenure,
    { kind: "targetTurnEndSave" },
  );
  assert.deepEqual(
    (materialized?.semantic as {
      condition: unknown;
    }).condition,
    {
      clauses: [
        { kind: "forbidActionTag", actionTag: "restricted" },
        { kind: "requireActionTag", actionTag: "focus" },
        { kind: "forbidMovement" },
      ],
    },
  );
});

test("Scenario builder emits setup-only immutable data", () => {
  const scenario = defineScenario({
    playBundleId: "contract.bundle@1.0.0:fnv1a64:test",
    board: { width: 2, height: 2, cells: [] },
    participants: [],
    turn: {
      initiativeOrder: [],
      currentActorId: "",
      round: 1,
      turn: 1,
    },
    randomSource: {
      policyId: "random.automatic",
      policyVersion: 1,
      sourceId: "random.system",
      sourceVersion: 1,
    },
  });

  assert.deepEqual(scenario.schema, { id: "asha.rpg.scenario", version: 3 });
  assert.equal(Object.isFrozen(scenario.board), true);
  assert.equal("commands" in scenario, false);
  assert.equal("rolls" in scenario, false);
  assert.equal("tester" in scenario, false);
});

test("Scenario templates stay artifact-independent until explicit instantiation", () => {
  const template = defineScenarioTemplate({
    identity: { id: "scenario.duel", version: "1.0.0" },
    playBundle: { id: "play.starter", version: "1.0.0" },
    presentation: { label: "Starter duel" },
    board: { width: 3, height: 3, cells: [] },
    participants: [],
    turn: {
      initiativeOrder: [],
      currentActorId: "",
      round: 1,
      turn: 1,
    },
    randomSource: {
      policyId: "random.automatic",
      policyVersion: 1,
      sourceId: "random.system",
      sourceVersion: 1,
    },
  });

  assert.equal(template.schema.id, "asha.rpg.scenario-template");
  assert.equal(Object.isFrozen(template), true);
  assert.equal("playBundleId" in template, false);

  const scenario = instantiateScenarioTemplate(
    template,
    "play.starter@1.0.0:fnv1a64:artifact",
  );
  assert.equal(scenario.playBundleId, "play.starter@1.0.0:fnv1a64:artifact");
  assert.deepEqual(scenario.board, template.board);
});

function prepareRulesetAction(
  ruleset: typeof semanticRuleset,
  stat: ReturnType<typeof rulesetStat>,
  defense: ReturnType<typeof rulesetDefense>,
) {
  const authoredAction = action({
    id: actionId("contract.ruleset-owned-action"),
    name: "Ruleset-owned action",
    sourcePath: "contract/ruleset-owned-action.ts",
    targets: hostile({ range: 1 }),
    check: attack({ modifier: readStat("actor", stat), defense }),
    rollScope: "perTarget",
    program: onCheck({ hit: heal({ amount: constant(1) }) }),
  });
  const definition = defineActionDefinition({
    id: authoredAction.id,
    visibility: "public",
    extensionPolicy: "sealed",
    source: {
      module: "contract/ruleset-owned-action.ts",
      declaration: "action",
    },
    action: authoredAction,
  });
  const contentPack = defineContentPack({
    identity: { id: "contract.ruleset-owned-content", version: "1.0.0" },
    entry: {
      module: "contract/ruleset-owned-action.ts",
      declaration: "content",
    },
    definitions: [definition],
  });
  return preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.ruleset-owned-bundle", version: "1.0.0" },
      ruleset,
      base: contentPackRequest({
        id: contentPack.identity.id,
        version: "1.0.0",
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(contentPack)],
  });
}

function scalarRulesetAction(
  ruleset: typeof semanticRuleset,
  check = scalarTest({
    profile: rulesetScalarTestProfile(ruleset, "graded"),
    base: readStat("actor", rulesetStat(ruleset, "strength")),
    difficulty: {
      kind: "targetDefense" as const,
      defense: rulesetDefense(ruleset, "armor-class"),
    },
  }),
) {
  return action({
    id: actionId("contract.scalar-action"),
    name: "Scalar action",
    sourcePath: "contract/scalar-action.ts",
    targets: hostile({ range: 1 }),
    check,
    rollScope: "perTarget",
    program: onOutcome({
      branches: {
        success: heal({ amount: constant(2) }),
      },
      default: heal({ amount: constant(1) }),
    }),
  });
}

function prepareScalarRulesetAction(
  ruleset: typeof semanticRuleset,
  check?: ReturnType<typeof scalarTest>,
) {
  const authoredAction = scalarRulesetAction(ruleset, check);
  const definition = defineActionDefinition({
    id: authoredAction.id,
    visibility: "public",
    extensionPolicy: "sealed",
    source: {
      module: "contract/scalar-action.ts",
      declaration: "action",
    },
    action: authoredAction,
  });
  const contentPack = defineContentPack({
    identity: { id: "contract.scalar-content", version: "1.0.0" },
    entry: {
      module: "contract/scalar-action.ts",
      declaration: "content",
    },
    definitions: [definition],
  });
  return preparePlayBundle({
    bundle: composePlayBundle({
      identity: { id: "contract.scalar-bundle", version: "1.0.0" },
      ruleset,
      base: contentPackRequest({
        id: contentPack.identity.id,
        version: "1.0.0",
      }),
      add: [],
      overlays: [],
      configure: {},
    }),
    contentPacks: [contentPackSource(contentPack)],
  });
}
