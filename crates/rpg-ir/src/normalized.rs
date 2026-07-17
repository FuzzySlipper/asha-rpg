use serde::{Deserialize, Serialize};

pub const RPG_IR_IDENTITY: &str = "asha.rpg.ir";
pub const RPG_IR_MAJOR: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NormalizedRpgIr {
    pub schema: RpgIrSchema,
    pub package: RpgIrPackage,
    pub catalogs: RpgIrCatalogs,
    pub requirements: Vec<RpgIrRequirement>,
    pub actions: Vec<RpgIrAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIrSchema {
    pub identity: String,
    pub major: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIrPackage {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIrCatalogs {
    #[serde(default)]
    pub stats: Vec<String>,
    #[serde(default)]
    pub defenses: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgIrRequirementKind {
    Operation,
    Capability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIrRequirement {
    pub kind: RpgIrRequirementKind,
    pub id: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIrAction {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub targets: RpgIrTargetSelector,
    pub check: RpgIrCheck,
    pub roll_scope: RpgIrRollScope,
    #[serde(default)]
    pub costs: Vec<RpgIrResourceCost>,
    pub program: RpgIrProgram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgIrTeamConstraint {
    Hostile,
    Ally,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIrTargetSelector {
    pub team: RpgIrTeamConstraint,
    pub maximum_range: u32,
    pub maximum_targets: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgIrSubject {
    Actor,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RpgIrResourceCost {
    pub resource_id: String,
    pub amount: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgIrRollScope {
    Shared,
    PerTarget,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgIrCheck {
    NoRoll,
    Attack {
        modifier: RpgIrFormula,
        defense_id: String,
    },
    SavingThrow {
        difficulty: RpgIrFormula,
        defense_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgIrFormula {
    Constant {
        value: i32,
    },
    ReadStat {
        subject: RpgIrSubject,
        stat_id: String,
    },
    Add {
        terms: Vec<RpgIrFormula>,
    },
    Dice {
        count: u32,
        sides: u32,
        bonus: i32,
    },
    Half {
        value: Box<RpgIrFormula>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgIrComparison {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RpgIrStackingPolicy {
    Replace,
    Refresh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgIrPredicate {
    Always,
    Compare {
        left: RpgIrFormula,
        comparison: RpgIrComparison,
        right: RpgIrFormula,
    },
    Not {
        predicate: Box<RpgIrPredicate>,
    },
    All {
        predicates: Vec<RpgIrPredicate>,
    },
    Any {
        predicates: Vec<RpgIrPredicate>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgIrOperation {
    Damage {
        amount: RpgIrFormula,
        damage_type: String,
    },
    Heal {
        amount: RpgIrFormula,
    },
    ChangeResource {
        subject: RpgIrSubject,
        resource_id: String,
        delta: RpgIrFormula,
    },
    ApplyModifier {
        modifier_id: String,
        stacking_group: String,
        stacking: RpgIrStackingPolicy,
        value: RpgIrFormula,
        duration_turns: u32,
    },
    Move {
        subject: RpgIrSubject,
        delta_x: RpgIrFormula,
        delta_y: RpgIrFormula,
        maximum_distance: u32,
        provokes: bool,
    },
}

impl RpgIrOperation {
    pub const fn registration_id(&self) -> &'static str {
        match self {
            Self::Damage { .. } => "operation.damage",
            Self::Heal { .. } => "operation.heal",
            Self::ChangeResource { .. } => "operation.changeResource",
            Self::ApplyModifier { .. } => "operation.applyModifier",
            Self::Move { .. } => "operation.move",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RpgIrProgram {
    Operation {
        operation: RpgIrOperation,
    },
    Sequence {
        steps: Vec<RpgIrProgram>,
    },
    When {
        predicate: RpgIrPredicate,
        then: Box<RpgIrProgram>,
        #[serde(default)]
        otherwise: Option<Box<RpgIrProgram>>,
    },
    Repeat {
        count: u32,
        body: Box<RpgIrProgram>,
    },
    ForEachTarget {
        maximum: u32,
        body: Box<RpgIrProgram>,
    },
    OnCheck {
        #[serde(default)]
        hit: Option<Box<RpgIrProgram>>,
        #[serde(default)]
        miss: Option<Box<RpgIrProgram>>,
        #[serde(default)]
        saved: Option<Box<RpgIrProgram>>,
        #[serde(default)]
        failed: Option<Box<RpgIrProgram>>,
        #[serde(default)]
        no_roll: Option<Box<RpgIrProgram>>,
    },
    Atomic {
        body: Box<RpgIrProgram>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_decode_rejects_unknown_semantic_fields() {
        let source = br#"{
          "schema":{"identity":"asha.rpg.ir","major":1},
          "package":{"id":"consumer","version":"1.0.0","callback":"forbidden"},
          "catalogs":{},"requirements":[],"actions":[]
        }"#;
        let error = serde_json::from_slice::<NormalizedRpgIr>(source).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn operation_ids_are_closed_and_stable() {
        let damage = RpgIrOperation::Damage {
            amount: RpgIrFormula::Constant { value: 4 },
            damage_type: "arcane".to_owned(),
        };
        assert_eq!(damage.registration_id(), "operation.damage");
    }
}
