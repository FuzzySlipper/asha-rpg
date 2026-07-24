import assert from "node:assert/strict";
import { test } from "node:test";

import {
  action,
  actionId,
  attack,
  constant,
  composePlayBundle,
  contentPackRequest,
  contentPackSource,
  defineContentPack,
  defineParticipantProfileData,
  defineParticipantProfileDefinition,
  defineActionDefinition,
  defineRuleset,
  defineScenario,
  defineScenarioTemplate,
  instantiateScenarioTemplate,
  heal,
  hostile,
  onCheck,
  noRoll,
  participantProfileVitality,
  participantProfileStat,
  preparePlayBundle,
  readStat,
  rulesetDefense,
  rulesetStat,
  rulesetValueId,
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

test("Content Packs may carry inert consumer setup data without extending Rust catalogs", () => {
  const profileRuleset = defineRuleset({
    ...semanticRuleset,
    provides: {
      ...semanticRuleset.provides,
      operations: [{ id: "operation.heal", version: 1 }],
      capabilities: [
        { id: "capability.stats", version: 1 },
        { id: "capability.vitality", version: 1 },
      ],
    },
  });
  const authoredAction = action({
    id: actionId("action.profile-heal"),
    name: "Profile heal",
    sourcePath: "contract/profiles.ts#profileHeal",
    targets: hostile({ range: 1 }),
    check: noRoll(),
    program: onCheck({ noRoll: heal({ amount: constant(1) }) }),
  });
  const actionDefinition = defineActionDefinition({
    id: authoredAction.id,
    visibility: "public",
    extensionPolicy: "sealed",
    source: { module: "contract/profiles.ts", declaration: "profileHeal" },
    action: authoredAction,
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
      capabilities: [
        participantProfileVitality({ current: 10, max: 10 }),
        participantProfileStat(rulesetStat(profileRuleset, "strength"), 16),
      ],
    }),
  });
  const contentPack = defineContentPack({
    identity: { id: "contract.profile-content", version: "1.0.0" },
    entry: { module: "contract/profiles.ts", declaration: "content" },
    definitions: [actionDefinition, profile],
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
    schema: { identity: "asha.rpg.participant-profile", version: 1 },
    role: "player",
    definitionIds: [actionDefinition.id],
    items: [],
    equipment: [],
    capabilities: [
      { owner: "vitality", value: { current: 10, max: 10 } },
      { owner: "stat", id: "strength", value: 16 },
    ],
  });
  assert.deepEqual(result.prepared.contentRequirements.values, [
    { kind: "stat", id: "strength" },
  ]);
  assert.deepEqual(result.prepared.contentRequirements.numericDomains, ["score"]);

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

  assert.deepEqual(scenario.schema, { id: "asha.rpg.scenario", version: 1 });
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
