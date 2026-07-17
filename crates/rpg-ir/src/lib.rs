//! Ruleset declarations and operation vocabulary.
//!
//! This crate owns the declarative vocabulary that selects and configures Rust
//! authority behavior. It does not own content catalogs, combat state, or
//! effect application.

mod normalized;

pub use normalized::*;

/// Identity and compatibility metadata for an authored ruleset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    pub modules: Vec<RuleModuleDeclaration>,
}

impl RulesetMetadata {
    pub fn validate_modules(
        &self,
    ) -> Result<ValidatedRuleModuleRegistry, RuleModuleValidationError> {
        validate_rule_modules(&self.modules)
    }

    pub fn artifact_provenance(&self) -> RulesetArtifactProvenance {
        let mut module_versions = self
            .modules
            .iter()
            .map(|module| RulesetModuleProvenance {
                module: module.module,
                version: module.version.clone(),
            })
            .collect::<Vec<_>>();
        module_versions.sort_by_key(|module| module.module.code());

        RulesetArtifactProvenance {
            ruleset_id: self.id.clone(),
            ruleset_version: self.version.clone(),
            module_versions,
            effect_operation_vocabulary_version: EffectOperationId::VOCABULARY_VERSION.to_string(),
        }
    }

    pub fn validate_artifact_provenance(
        &self,
        provenance: &RulesetArtifactProvenance,
    ) -> Result<(), RulesetCompatibilityError> {
        if provenance.ruleset_id != self.id {
            return Err(RulesetCompatibilityError::UnknownRulesetId {
                expected_id: self.id.clone(),
                actual_id: provenance.ruleset_id.clone(),
            });
        }
        validate_version("ruleset", &self.version, &provenance.ruleset_version)?;
        validate_version(
            "effect-operation vocabulary",
            EffectOperationId::VOCABULARY_VERSION,
            &provenance.effect_operation_vocabulary_version,
        )?;

        if provenance.module_versions != self.artifact_provenance().module_versions {
            return Err(RulesetCompatibilityError::IncompatibleModuleRequirements);
        }

        Ok(())
    }
}

/// Immutable compatibility identity carried by serialized consumer artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetArtifactProvenance {
    pub ruleset_id: String,
    pub ruleset_version: String,
    pub module_versions: Vec<RulesetModuleProvenance>,
    pub effect_operation_vocabulary_version: String,
}

/// The version requirement for one statically selected behavior module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetModuleProvenance {
    pub module: RuleModuleId,
    pub version: String,
}

/// One versioned authority capability exposed by a compiled ruleset provider.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RulesetProviderCapability {
    pub id: String,
    pub version: String,
}

/// Deployment-time metadata for one closed, compiled ruleset provider.
///
/// Providers are data-only registrations over Rust-owned modules and operation
/// vocabularies. They do not carry callbacks or dynamically loaded rule code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetProviderDescriptor {
    pub provider_id: String,
    pub provider_version: String,
    pub ruleset: RulesetMetadata,
    pub operation_vocabulary_version: String,
    pub effect_operation_vocabulary_version: String,
    pub capabilities: Vec<RulesetProviderCapability>,
}

impl RulesetProviderDescriptor {
    pub fn capability(&self, capability_id: &str) -> Option<&RulesetProviderCapability> {
        self.capabilities
            .iter()
            .find(|capability| capability.id == capability_id)
    }
}

/// Validated closed catalog used by a concrete Rpg runtime composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulesetProviderCatalog {
    providers: Vec<RulesetProviderDescriptor>,
}

impl RulesetProviderCatalog {
    pub fn new(
        mut providers: Vec<RulesetProviderDescriptor>,
    ) -> Result<Self, Vec<RulesetProviderCatalogError>> {
        providers.sort_by(|left, right| {
            left.provider_id
                .cmp(&right.provider_id)
                .then_with(|| left.provider_version.cmp(&right.provider_version))
        });

        let mut errors = Vec::new();
        let mut provider_identities = std::collections::BTreeSet::new();
        let mut ruleset_identities = std::collections::BTreeSet::new();
        for provider in &mut providers {
            provider.capabilities.sort();
            let provider_identity =
                format!("{}@{}", provider.provider_id, provider.provider_version);
            if !provider_identities.insert(provider_identity.clone()) {
                errors.push(RulesetProviderCatalogError::DuplicateProviderIdentity {
                    identity: provider_identity,
                });
            }
            let ruleset_identity = format!("{}@{}", provider.ruleset.id, provider.ruleset.version);
            if !ruleset_identities.insert(ruleset_identity.clone()) {
                errors.push(RulesetProviderCatalogError::RulesetProviderCollision {
                    identity: ruleset_identity,
                });
            }
            if provider.provider_id.is_empty()
                || provider.provider_version.is_empty()
                || provider.ruleset.id.is_empty()
                || provider.ruleset.version.is_empty()
            {
                errors.push(RulesetProviderCatalogError::EmptyProviderIdentity);
            }
            if provider.ruleset.validate_modules().is_err() {
                errors.push(RulesetProviderCatalogError::InvalidRulesetModules {
                    ruleset_id: provider.ruleset.id.clone(),
                });
            }
            if provider.operation_vocabulary_version != OperationPipelineV2::VOCABULARY_VERSION {
                errors.push(
                    RulesetProviderCatalogError::IncompatibleOperationVocabulary {
                        provider_id: provider.provider_id.clone(),
                        expected_version: OperationPipelineV2::VOCABULARY_VERSION.to_string(),
                        actual_version: provider.operation_vocabulary_version.clone(),
                    },
                );
            }
            if provider.effect_operation_vocabulary_version != EffectOperationId::VOCABULARY_VERSION
            {
                errors.push(
                    RulesetProviderCatalogError::IncompatibleEffectOperationVocabulary {
                        provider_id: provider.provider_id.clone(),
                        expected_version: EffectOperationId::VOCABULARY_VERSION.to_string(),
                        actual_version: provider.effect_operation_vocabulary_version.clone(),
                    },
                );
            }
            let mut capability_identities = std::collections::BTreeSet::new();
            for capability in &provider.capabilities {
                let identity = format!("{}@{}", capability.id, capability.version);
                if capability.id.is_empty() || capability.version.is_empty() {
                    errors.push(RulesetProviderCatalogError::EmptyCapabilityIdentity {
                        provider_id: provider.provider_id.clone(),
                    });
                } else if !capability_identities.insert(identity.clone()) {
                    errors.push(RulesetProviderCatalogError::DuplicateCapabilityIdentity {
                        provider_id: provider.provider_id.clone(),
                        identity,
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(Self { providers })
        } else {
            Err(errors)
        }
    }

    pub fn providers(&self) -> &[RulesetProviderDescriptor] {
        &self.providers
    }

    pub fn select_ruleset(
        &self,
        ruleset_id: &str,
        ruleset_version: &str,
    ) -> Result<&RulesetProviderDescriptor, RulesetProviderCompatibilityError> {
        if let Some(provider) = self.providers.iter().find(|provider| {
            provider.ruleset.id == ruleset_id && provider.ruleset.version == ruleset_version
        }) {
            return Ok(provider);
        }
        if let Some(provider) = self
            .providers
            .iter()
            .find(|provider| provider.ruleset.id == ruleset_id)
        {
            return Err(
                RulesetProviderCompatibilityError::IncompatibleRulesetVersion {
                    ruleset_id: ruleset_id.to_string(),
                    expected_version: provider.ruleset.version.clone(),
                    actual_version: ruleset_version.to_string(),
                },
            );
        }
        Err(RulesetProviderCompatibilityError::ProviderUnavailable {
            ruleset_id: ruleset_id.to_string(),
            ruleset_version: ruleset_version.to_string(),
        })
    }

    pub fn validate_ruleset(
        &self,
        ruleset: &RulesetMetadata,
    ) -> Result<&RulesetProviderDescriptor, RulesetProviderCompatibilityError> {
        let provider = self.select_ruleset(&ruleset.id, &ruleset.version)?;
        if provider.ruleset != *ruleset {
            return Err(
                RulesetProviderCompatibilityError::RulesetDefinitionMismatch {
                    ruleset_id: ruleset.id.clone(),
                    ruleset_version: ruleset.version.clone(),
                },
            );
        }
        Ok(provider)
    }

    pub fn validate_artifact_provenance(
        &self,
        provenance: &RulesetArtifactProvenance,
    ) -> Result<&RulesetProviderDescriptor, RulesetProviderCompatibilityError> {
        let provider = self.select_ruleset(&provenance.ruleset_id, &provenance.ruleset_version)?;
        provider
            .ruleset
            .validate_artifact_provenance(provenance)
            .map_err(
                |error| RulesetProviderCompatibilityError::ArtifactIncompatible {
                    code: error.code().to_string(),
                },
            )?;
        Ok(provider)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesetProviderCatalogError {
    EmptyProviderIdentity,
    DuplicateProviderIdentity {
        identity: String,
    },
    RulesetProviderCollision {
        identity: String,
    },
    InvalidRulesetModules {
        ruleset_id: String,
    },
    IncompatibleOperationVocabulary {
        provider_id: String,
        expected_version: String,
        actual_version: String,
    },
    IncompatibleEffectOperationVocabulary {
        provider_id: String,
        expected_version: String,
        actual_version: String,
    },
    EmptyCapabilityIdentity {
        provider_id: String,
    },
    DuplicateCapabilityIdentity {
        provider_id: String,
        identity: String,
    },
}

impl RulesetProviderCatalogError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyProviderIdentity => "emptyRulesetProviderIdentity",
            Self::DuplicateProviderIdentity { .. } => "duplicateRulesetProviderIdentity",
            Self::RulesetProviderCollision { .. } => "rulesetProviderCollision",
            Self::InvalidRulesetModules { .. } => "invalidRulesetProviderModules",
            Self::IncompatibleOperationVocabulary { .. } => {
                "incompatibleProviderOperationVocabulary"
            }
            Self::IncompatibleEffectOperationVocabulary { .. } => {
                "incompatibleProviderEffectOperationVocabulary"
            }
            Self::EmptyCapabilityIdentity { .. } => "emptyRulesetProviderCapability",
            Self::DuplicateCapabilityIdentity { .. } => "duplicateRulesetProviderCapability",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesetProviderCompatibilityError {
    ProviderUnavailable {
        ruleset_id: String,
        ruleset_version: String,
    },
    IncompatibleRulesetVersion {
        ruleset_id: String,
        expected_version: String,
        actual_version: String,
    },
    RulesetDefinitionMismatch {
        ruleset_id: String,
        ruleset_version: String,
    },
    MissingCapability {
        provider_id: String,
        capability_id: String,
        capability_version: String,
    },
    ArtifactIncompatible {
        code: String,
    },
}

impl RulesetProviderCompatibilityError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ProviderUnavailable { .. } => "rulesetProviderUnavailable",
            Self::IncompatibleRulesetVersion { .. } => "incompatibleProviderRulesetVersion",
            Self::RulesetDefinitionMismatch { .. } => "rulesetProviderDefinitionMismatch",
            Self::MissingCapability { .. } => "rulesetProviderCapabilityMissing",
            Self::ArtifactIncompatible { .. } => "rulesetProviderArtifactIncompatible",
        }
    }
}

/// Stable compatibility failures for loading previously authored artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesetCompatibilityError {
    UnknownRulesetId {
        expected_id: String,
        actual_id: String,
    },
    NewerVersion {
        surface: &'static str,
        expected: String,
        actual: String,
    },
    IncompatibleVersion {
        surface: &'static str,
        expected: String,
        actual: String,
    },
    IncompatibleModuleRequirements,
}

impl RulesetCompatibilityError {
    pub const fn code(&self) -> &'static str {
        match self {
            RulesetCompatibilityError::UnknownRulesetId { .. } => "unknownRulesetId",
            RulesetCompatibilityError::NewerVersion { .. } => "newerRulesetVersion",
            RulesetCompatibilityError::IncompatibleVersion { .. } => "incompatibleRulesetVersion",
            RulesetCompatibilityError::IncompatibleModuleRequirements => {
                "incompatibleRulesetModules"
            }
        }
    }
}

fn validate_version(
    surface: &'static str,
    expected: &str,
    actual: &str,
) -> Result<(), RulesetCompatibilityError> {
    if actual == expected {
        return Ok(());
    }

    let error = if version_is_newer(actual, expected) {
        RulesetCompatibilityError::NewerVersion {
            surface,
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    } else {
        RulesetCompatibilityError::IncompatibleVersion {
            surface,
            expected: expected.to_string(),
            actual: actual.to_string(),
        }
    };
    Err(error)
}

fn version_is_newer(actual: &str, expected: &str) -> bool {
    let actual_segments = actual.split('.').collect::<Vec<_>>();
    let expected_segments = expected.split('.').collect::<Vec<_>>();
    let segment_count = actual_segments.len().max(expected_segments.len());

    for index in 0..segment_count {
        let actual_segment = actual_segments
            .get(index)
            .and_then(|segment| segment.parse::<u32>().ok())
            .unwrap_or_default();
        let expected_segment = expected_segments
            .get(index)
            .and_then(|segment| segment.parse::<u32>().ok())
            .unwrap_or_default();
        if actual_segment != expected_segment {
            return actual_segment > expected_segment;
        }
    }

    false
}

/// A closed identifier for a Rust behavior module selected by a ruleset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleModuleId {
    ActionResolution,
    TurnControl,
}

impl RuleModuleId {
    pub const fn code(self) -> &'static str {
        match self {
            RuleModuleId::ActionResolution => "actionResolution",
            RuleModuleId::TurnControl => "turnControl",
        }
    }

    pub const fn supported_version(self) -> &'static str {
        match self {
            RuleModuleId::ActionResolution => "1",
            RuleModuleId::TurnControl => "1",
        }
    }

    pub fn from_code(code: &str) -> Result<Self, RuleModuleValidationError> {
        match code {
            "actionResolution" => Ok(RuleModuleId::ActionResolution),
            "turnControl" => Ok(RuleModuleId::TurnControl),
            _ => Err(RuleModuleValidationError::UnknownModuleCode {
                code: code.to_string(),
            }),
        }
    }
}

/// A versioned declaration of a Rust behavior module used by a ruleset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleModuleDeclaration {
    pub module: RuleModuleId,
    pub version: String,
    pub configuration: RuleModuleConfiguration,
}

impl RuleModuleDeclaration {
    pub fn action_resolution(configuration: ActionResolutionModuleConfiguration) -> Self {
        Self {
            module: RuleModuleId::ActionResolution,
            version: RuleModuleId::ActionResolution
                .supported_version()
                .to_string(),
            configuration: RuleModuleConfiguration::ActionResolution(configuration),
        }
    }

    pub fn turn_control(configuration: TurnControlModuleConfiguration) -> Self {
        Self {
            module: RuleModuleId::TurnControl,
            version: RuleModuleId::TurnControl.supported_version().to_string(),
            configuration: RuleModuleConfiguration::TurnControl(configuration),
        }
    }
}

/// Closed configuration schemas for supported behavior modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleModuleConfiguration {
    ActionResolution(ActionResolutionModuleConfiguration),
    TurnControl(TurnControlModuleConfiguration),
}

impl RuleModuleConfiguration {
    pub const fn module(&self) -> RuleModuleId {
        match self {
            RuleModuleConfiguration::ActionResolution(_) => RuleModuleId::ActionResolution,
            RuleModuleConfiguration::TurnControl(_) => RuleModuleId::TurnControl,
        }
    }
}

/// Static action-resolution options supported by the current authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResolutionModuleConfiguration {
    pub targeting_policy: ActionResolutionTargetingPolicy,
    pub supported_check_handlers: Vec<CheckHandlerKind>,
}

impl ActionResolutionModuleConfiguration {
    pub fn declared_targets_and_line_of_sight() -> Self {
        Self {
            targeting_policy: ActionResolutionTargetingPolicy::DeclaredTargetsAndLineOfSight,
            supported_check_handlers: vec![CheckHandlerKind::AttackVsDefense],
        }
    }

    pub fn with_supported_check_handlers(
        targeting_policy: ActionResolutionTargetingPolicy,
        supported_check_handlers: Vec<CheckHandlerKind>,
    ) -> Self {
        Self {
            targeting_policy,
            supported_check_handlers,
        }
    }

    pub fn supports_check(&self, check: &CheckDeclaration) -> bool {
        self.supported_check_handlers
            .iter()
            .any(|handler| *handler == CheckHandlerKind::for_declaration(check))
    }
}

/// Closed Rust handler families a ruleset may authorize for action checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckHandlerKind {
    AttackVsDefense,
    SavingThrow,
    Contested,
}

impl CheckHandlerKind {
    pub const fn code(self) -> &'static str {
        match self {
            CheckHandlerKind::AttackVsDefense => "attackVsDefense",
            CheckHandlerKind::SavingThrow => "savingThrow",
            CheckHandlerKind::Contested => "contested",
        }
    }

    pub const fn for_declaration(check: &CheckDeclaration) -> Self {
        match check {
            CheckDeclaration::Attack(_) => CheckHandlerKind::AttackVsDefense,
            CheckDeclaration::SavingThrow(_) => CheckHandlerKind::SavingThrow,
            CheckDeclaration::Contested(_) => CheckHandlerKind::Contested,
        }
    }
}

/// The closed targeting policy vocabulary for action resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionResolutionTargetingPolicy {
    DeclaredTargetsAndLineOfSight,
}

impl ActionResolutionTargetingPolicy {
    pub const fn code(self) -> &'static str {
        match self {
            ActionResolutionTargetingPolicy::DeclaredTargetsAndLineOfSight => {
                "declaredTargetsAndLineOfSight"
            }
        }
    }
}

/// Static turn-control options recognized by the current authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnControlModuleConfiguration {
    pub turn_order_policy: TurnOrderPolicy,
    pub combat_end_policy: CombatEndPolicy,
}

impl TurnControlModuleConfiguration {
    pub const fn explicit_turn_order() -> Self {
        Self {
            turn_order_policy: TurnOrderPolicy::Explicit,
            combat_end_policy: CombatEndPolicy::LastSideStanding,
        }
    }

    pub const fn explicit_turn_order_with_end_policy(combat_end_policy: CombatEndPolicy) -> Self {
        Self {
            turn_order_policy: TurnOrderPolicy::Explicit,
            combat_end_policy,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CombatEndPolicy {
    LastSideStanding,
    ObjectiveSideVictory { side_id: String },
    ExplicitOnly,
}

impl CombatEndPolicy {
    pub const fn code(&self) -> &'static str {
        match self {
            CombatEndPolicy::LastSideStanding => "lastSideStanding",
            CombatEndPolicy::ObjectiveSideVictory { .. } => "objectiveSideVictory",
            CombatEndPolicy::ExplicitOnly => "explicitOnly",
        }
    }

    pub fn objective_side_id(&self) -> Option<&str> {
        match self {
            CombatEndPolicy::ObjectiveSideVictory { side_id } => Some(side_id),
            CombatEndPolicy::LastSideStanding | CombatEndPolicy::ExplicitOnly => None,
        }
    }
}

/// The closed turn-order vocabulary for turn control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnOrderPolicy {
    Explicit,
}

impl TurnOrderPolicy {
    pub const fn code(self) -> &'static str {
        match self {
            TurnOrderPolicy::Explicit => "explicit",
        }
    }
}

/// A validated, static ruleset module registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuleModuleRegistry {
    declarations: Vec<ValidatedRuleModuleDeclaration>,
    action_resolution: ActionResolutionModuleConfiguration,
    turn_control: Option<TurnControlModuleConfiguration>,
}

impl ValidatedRuleModuleRegistry {
    pub fn declarations(&self) -> &[ValidatedRuleModuleDeclaration] {
        &self.declarations
    }

    pub const fn action_resolution(&self) -> &ActionResolutionModuleConfiguration {
        &self.action_resolution
    }

    pub const fn turn_control(&self) -> Option<&TurnControlModuleConfiguration> {
        self.turn_control.as_ref()
    }
}

/// A module declaration that passed version and configuration validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedRuleModuleDeclaration {
    pub module: RuleModuleId,
    pub version: String,
}

/// Stable errors emitted while validating ruleset behavior declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleModuleValidationError {
    UnknownModuleCode {
        code: String,
    },
    MissingRequiredModule {
        module: RuleModuleId,
    },
    DuplicateModuleDeclaration {
        module: RuleModuleId,
    },
    IncompatibleModuleVersion {
        module: RuleModuleId,
        expected_version: String,
        actual_version: String,
    },
    ConfigurationDoesNotMatchModule {
        module: RuleModuleId,
        configuration_module: RuleModuleId,
    },
}

impl RuleModuleValidationError {
    pub const fn code(&self) -> &'static str {
        match self {
            RuleModuleValidationError::UnknownModuleCode { .. } => "unknownRuleModule",
            RuleModuleValidationError::MissingRequiredModule { .. } => "missingRequiredRuleModule",
            RuleModuleValidationError::DuplicateModuleDeclaration { .. } => {
                "duplicateRuleModuleDeclaration"
            }
            RuleModuleValidationError::IncompatibleModuleVersion { .. } => {
                "incompatibleRuleModuleVersion"
            }
            RuleModuleValidationError::ConfigurationDoesNotMatchModule { .. } => {
                "ruleModuleConfigurationMismatch"
            }
        }
    }
}

/// Validate a ruleset's module declarations into the static registry consumed by authority code.
pub fn validate_rule_modules(
    declarations: &[RuleModuleDeclaration],
) -> Result<ValidatedRuleModuleRegistry, RuleModuleValidationError> {
    let mut validated_declarations = Vec::with_capacity(declarations.len());
    let mut action_resolution = None;
    let mut turn_control = None;

    for declaration in declarations {
        let configuration_module = declaration.configuration.module();
        if declaration.module != configuration_module {
            return Err(RuleModuleValidationError::ConfigurationDoesNotMatchModule {
                module: declaration.module,
                configuration_module,
            });
        }

        let expected_version = declaration.module.supported_version();
        if declaration.version != expected_version {
            return Err(RuleModuleValidationError::IncompatibleModuleVersion {
                module: declaration.module,
                expected_version: expected_version.to_string(),
                actual_version: declaration.version.clone(),
            });
        }

        match (&declaration.module, &declaration.configuration) {
            (RuleModuleId::ActionResolution, RuleModuleConfiguration::ActionResolution(config)) => {
                if action_resolution.replace(config.clone()).is_some() {
                    return Err(RuleModuleValidationError::DuplicateModuleDeclaration {
                        module: RuleModuleId::ActionResolution,
                    });
                }
            }
            (RuleModuleId::TurnControl, RuleModuleConfiguration::TurnControl(config)) => {
                if turn_control.replace(config.clone()).is_some() {
                    return Err(RuleModuleValidationError::DuplicateModuleDeclaration {
                        module: RuleModuleId::TurnControl,
                    });
                }
            }
            _ => {
                return Err(RuleModuleValidationError::ConfigurationDoesNotMatchModule {
                    module: declaration.module,
                    configuration_module,
                });
            }
        }

        validated_declarations.push(ValidatedRuleModuleDeclaration {
            module: declaration.module,
            version: declaration.version.clone(),
        });
    }

    let Some(action_resolution) = action_resolution else {
        return Err(RuleModuleValidationError::MissingRequiredModule {
            module: RuleModuleId::ActionResolution,
        });
    };

    Ok(ValidatedRuleModuleRegistry {
        declarations: validated_declarations,
        action_resolution,
        turn_control,
    })
}

/// The authored category of an ability definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityDefinitionKind {
    Ability,
    Spell,
}

impl AbilityDefinitionKind {
    pub const fn code(self) -> &'static str {
        match self {
            AbilityDefinitionKind::Ability => "ability",
            AbilityDefinitionKind::Spell => "spell",
        }
    }
}

/// A named ability or spell declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbilityDefinition {
    pub id: String,
    pub name: String,
    pub kind: AbilityDefinitionKind,
    pub summary: String,
    pub tags: Vec<String>,
}

/// A closed action-economy resource selected by authored action costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionResourceKind {
    StandardAction,
    SpellSlot,
    Charge,
    Cooldown,
}

impl ActionResourceKind {
    pub const fn code(self) -> &'static str {
        match self {
            ActionResourceKind::StandardAction => "standardAction",
            ActionResourceKind::SpellSlot => "spellSlot",
            ActionResourceKind::Charge => "charge",
            ActionResourceKind::Cooldown => "cooldown",
        }
    }
}

/// The deterministic point at which a depleted resource pool may refresh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionResourceRefreshPolicy {
    Never,
    CombatStart,
    TurnStart,
    Turns(u32),
}

impl ActionResourceRefreshPolicy {
    pub const fn code(&self) -> &'static str {
        match self {
            ActionResourceRefreshPolicy::Never => "never",
            ActionResourceRefreshPolicy::CombatStart => "combatStart",
            ActionResourceRefreshPolicy::TurnStart => "turnStart",
            ActionResourceRefreshPolicy::Turns(_) => "turns",
        }
    }
}

/// An authored resource pool owned by one combatant at combat creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResourcePool {
    pub id: String,
    pub kind: ActionResourceKind,
    pub initial: u32,
    pub maximum: u32,
    pub refresh_policy: ActionResourceRefreshPolicy,
}

impl ActionResourcePool {
    pub fn standard_action() -> Self {
        Self {
            id: "standard-action".to_string(),
            kind: ActionResourceKind::StandardAction,
            initial: 1,
            maximum: 1,
            refresh_policy: ActionResourceRefreshPolicy::TurnStart,
        }
    }
}

/// One authoritative resource cost required to admit an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionResourceCost {
    pub resource_id: String,
    pub amount: u32,
}

impl ActionResourceCost {
    pub fn standard_action() -> Self {
        Self {
            resource_id: "standard-action".to_string(),
            amount: 1,
        }
    }
}

/// A declared action with targeting, check, and effect configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionDefinition {
    pub id: String,
    pub ruleset_id: String,
    pub ability_id: String,
    pub name: String,
    pub actor_id: String,
    pub targeting: TargetingDeclaration,
    pub check: CheckDeclaration,
    pub hit: HitEffect,
    pub resource_costs: Vec<ActionResourceCost>,
    pub movement: Option<MovementActionDeclaration>,
    pub action_text: String,
    pub effect_text: String,
}

/// A content-declared movement behavior. The initial implementation uses
/// direct orthogonal Manhattan cost and does not claim route finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementActionDeclaration {
    pub allowance: u32,
    pub topology: MovementTopology,
    pub blocking_terrain_tags: Vec<String>,
    pub difficult_terrain_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementTopology {
    OrthogonalManhattan,
}

impl ActionDefinition {
    pub fn attack_check(&self) -> Option<&AttackCheckDeclaration> {
        match &self.check {
            CheckDeclaration::Attack(attack) => Some(attack),
            CheckDeclaration::SavingThrow(_) | CheckDeclaration::Contested(_) => None,
        }
    }

    pub fn attack_check_mut(&mut self) -> Option<&mut AttackCheckDeclaration> {
        match &mut self.check {
            CheckDeclaration::Attack(attack) => Some(attack),
            CheckDeclaration::SavingThrow(_) | CheckDeclaration::Contested(_) => None,
        }
    }
}

/// The target legality inputs declared by an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetingDeclaration {
    pub target_kind: TargetKind,
    pub selection: TargetSelection,
    pub team_constraint: TargetTeamConstraint,
    pub maximum_range: u32,
    pub visibility_requirement: VisibilityRequirement,
    pub target_ids: Vec<String>,
    pub visible_target_ids: Vec<String>,
    /// Present only for operation-pipeline v2 actions. Existing v1 actions
    /// retain their exact declaration and artifact fingerprints with `None`.
    pub operation_pipeline: Option<OperationPipelineV2>,
}

/// Bounded target and effect execution contract for operation pipeline v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationPipelineV2 {
    pub maximum_targets: u32,
    pub area: Option<AreaTargetingDeclaration>,
    pub roll_policy: ActionRollPolicy,
    pub failure_policy: TargetFailurePolicy,
    pub target_order: TargetOrderPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetingOperationId {
    SingleCombatant,
    MultipleCombatants,
    ManhattanBurstArea,
    CellMovement,
}

impl TargetingOperationId {
    pub const ALL: &'static [Self] = &[
        Self::SingleCombatant,
        Self::MultipleCombatants,
        Self::ManhattanBurstArea,
        Self::CellMovement,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            Self::SingleCombatant => "singleCombatant",
            Self::MultipleCombatants => "multipleCombatants",
            Self::ManhattanBurstArea => "manhattanBurstArea",
            Self::CellMovement => "cellMovement",
        }
    }

    pub const fn validation_supported(self) -> bool {
        matches!(
            self,
            Self::SingleCombatant
                | Self::MultipleCombatants
                | Self::ManhattanBurstArea
                | Self::CellMovement
        )
    }
}

impl OperationPipelineV2 {
    pub const VOCABULARY_VERSION: &'static str = "2";
    pub const MAXIMUM_TARGET_LIMIT: u32 = 8;
    pub const MAXIMUM_AREA_RADIUS: u32 = 4;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaTargetingDeclaration {
    pub shape: AreaShape,
    pub radius: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaShape {
    ManhattanBurst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRollPolicy {
    Shared,
    PerTarget,
    NoRoll,
}

impl ActionRollPolicy {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::PerTarget => "perTarget",
            Self::NoRoll => "noRoll",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetFailurePolicy {
    Atomic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOrderPolicy {
    CanonicalId,
}

/// Declared target shapes. Area targeting uses a bounded Rust-projected cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Combatant,
    Area,
}

/// Declared target cardinality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSelection {
    Single,
    Multiple,
}

/// The team relationship required for a target to be legal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetTeamConstraint {
    Hostile,
    Ally,
    Any,
}

/// Whether a target must be listed as visible before resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityRequirement {
    Required,
    Ignored,
}

/// The check declaration used to resolve an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckDeclaration {
    Attack(AttackCheckDeclaration),
    SavingThrow(SavingThrowCheckDeclaration),
    Contested(ContestedCheckDeclaration),
}

/// The currently supported attack-versus-defense check declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackCheckDeclaration {
    pub modifier: i32,
    pub modifier_stat_id: String,
    pub defense: DefenseReference,
}

/// A named defense referenced by a check declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefenseReference {
    pub id: String,
    pub label: String,
}

/// A declared saving throw. Its evaluation is intentionally not implemented yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavingThrowCheckDeclaration {
    pub save_stat_id: String,
    pub difficulty_class: i32,
}

/// A declared contested check. Its evaluation is intentionally not implemented yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContestedCheckDeclaration {
    pub actor_stat_id: String,
    pub target_stat_id: String,
}

/// The operation set applied after an accepted hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitEffect {
    pub damage_bonus: i32,
    pub damage_type: String,
    pub modifier_id: String,
    pub modifier_label: String,
    pub modifier_duration: String,
    pub operations: Vec<HitEffectOperation>,
}

impl HitEffect {
    pub fn damage_operation(&self) -> Option<&DamageEffectOperation> {
        self.operations
            .iter()
            .find_map(|operation| match operation {
                HitEffectOperation::Damage(damage) => Some(damage),
                HitEffectOperation::Heal(_)
                | HitEffectOperation::GrantTemporaryVitality(_)
                | HitEffectOperation::ApplyModifier(_)
                | HitEffectOperation::Move(_)
                | HitEffectOperation::ChangeResource(_)
                | HitEffectOperation::OpenReactionWindow(_) => None,
            })
    }

    pub fn modifier_operation(&self) -> Option<&ModifierEffectOperation> {
        self.operations
            .iter()
            .find_map(|operation| match operation {
                HitEffectOperation::Damage(_)
                | HitEffectOperation::Heal(_)
                | HitEffectOperation::GrantTemporaryVitality(_)
                | HitEffectOperation::Move(_)
                | HitEffectOperation::ChangeResource(_)
                | HitEffectOperation::OpenReactionWindow(_) => None,
                HitEffectOperation::ApplyModifier(modifier) => Some(modifier),
            })
    }

    pub fn reaction_hook_operation(&self) -> Option<&ReactionHookEffectOperation> {
        self.operations
            .iter()
            .find_map(|operation| match operation {
                HitEffectOperation::OpenReactionWindow(hook) => Some(hook),
                _ => None,
            })
    }
}

/// A typed effect operation selected by an action declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitEffectOperation {
    Damage(DamageEffectOperation),
    Heal(HealingEffectOperation),
    GrantTemporaryVitality(TemporaryVitalityEffectOperation),
    ApplyModifier(ModifierEffectOperation),
    Move(MovementEffectOperation),
    ChangeResource(ResourceChangeEffectOperation),
    OpenReactionWindow(ReactionHookEffectOperation),
}

impl HitEffectOperation {
    pub const fn id(&self) -> EffectOperationId {
        match self {
            HitEffectOperation::Damage(_) => EffectOperationId::Damage,
            HitEffectOperation::Heal(_) => EffectOperationId::Heal,
            HitEffectOperation::GrantTemporaryVitality(_) => {
                EffectOperationId::GrantTemporaryVitality
            }
            HitEffectOperation::ApplyModifier(_) => EffectOperationId::ApplyModifier,
            HitEffectOperation::Move(_) => EffectOperationId::Move,
            HitEffectOperation::ChangeResource(_) => EffectOperationId::ChangeResource,
            HitEffectOperation::OpenReactionWindow(_) => EffectOperationId::OpenReactionWindow,
        }
    }

    pub const fn is_currently_supported(&self) -> bool {
        matches!(
            self,
            HitEffectOperation::Damage(_)
                | HitEffectOperation::Heal(_)
                | HitEffectOperation::GrantTemporaryVitality(_)
                | HitEffectOperation::ApplyModifier(_)
                | HitEffectOperation::Move(_)
                | HitEffectOperation::ChangeResource(_)
                | HitEffectOperation::OpenReactionWindow(_)
        )
    }
}

/// Stable identifiers for effect-operation declarations and future trace entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectOperationId {
    Damage,
    Heal,
    GrantTemporaryVitality,
    ApplyModifier,
    Move,
    ChangeResource,
    OpenReactionWindow,
}

impl EffectOperationId {
    pub const VOCABULARY_VERSION: &'static str = "1";
    pub const ALL: &'static [Self] = &[
        Self::Damage,
        Self::Heal,
        Self::GrantTemporaryVitality,
        Self::ApplyModifier,
        Self::Move,
        Self::ChangeResource,
        Self::OpenReactionWindow,
    ];

    pub const fn code(self) -> &'static str {
        match self {
            EffectOperationId::Damage => "damage",
            EffectOperationId::Heal => "heal",
            EffectOperationId::GrantTemporaryVitality => "grantTemporaryVitality",
            EffectOperationId::ApplyModifier => "applyModifier",
            EffectOperationId::Move => "move",
            EffectOperationId::ChangeResource => "changeResource",
            EffectOperationId::OpenReactionWindow => "openReactionWindow",
        }
    }

    pub const fn validation_supported(self) -> bool {
        matches!(
            self,
            Self::Damage
                | Self::Heal
                | Self::GrantTemporaryVitality
                | Self::ApplyModifier
                | Self::Move
                | Self::ChangeResource
                | Self::OpenReactionWindow
        )
    }
}

/// A damage operation declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageEffectOperation {
    pub damage_bonus: i32,
    pub damage_type: String,
}

/// A capped healing operation declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealingEffectOperation {
    pub healing_bonus: i32,
    pub healing_type: String,
}

/// A temporary vitality operation declaration. Higher values replace lower
/// current temporary vitality rather than stacking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporaryVitalityEffectOperation {
    pub vitality_bonus: i32,
}

/// A modifier application operation declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierEffectOperation {
    pub modifier_id: String,
    pub modifier_label: String,
    pub modifier_duration: String,
}

/// A bounded movement operation declaration interpreted by Rust authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementEffectOperation {
    pub maximum_distance: u32,
    pub movement_kind: MovementKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementKind {
    Push,
    Pull,
    Shift,
}

impl MovementKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Pull => "pull",
            Self::Shift => "shift",
        }
    }
}

/// A bounded resource mutation declaration interpreted by Rust authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceChangeEffectOperation {
    pub resource_id: String,
    pub delta: i32,
}

/// A closed reaction-window declaration interpreted by Rust authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionHookEffectOperation {
    pub hook_id: String,
    pub window: ReactionWindow,
    pub eligible_reactor_ids: Vec<String>,
    pub options: Vec<ReactionOptionDeclaration>,
    pub maximum_nested_depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionOptionDeclaration {
    pub id: String,
    pub reactor_id: String,
    pub opens_nested_window: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactionWindow {
    BeforeEffect,
    AfterEffect,
}

impl ReactionWindow {
    pub const fn code(self) -> &'static str {
        match self {
            ReactionWindow::BeforeEffect => "beforeEffect",
            ReactionWindow::AfterEffect => "afterEffect",
        }
    }
}

/// Whether a modifier declaration survives beyond a temporary combat window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierTenure {
    Temporary,
    Permanent,
}

impl ModifierTenure {
    pub const fn code(self) -> &'static str {
        match self {
            ModifierTenure::Temporary => "temporary",
            ModifierTenure::Permanent => "permanent",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_rule_modules, AbilityDefinitionKind, ActionResolutionModuleConfiguration,
        CombatEndPolicy, DamageEffectOperation, EffectOperationId, HealingEffectOperation,
        HitEffect, HitEffectOperation, ModifierEffectOperation, ModifierTenure,
        RuleModuleConfiguration, RuleModuleDeclaration, RuleModuleId,
        TurnControlModuleConfiguration,
    };

    #[test]
    fn hit_effect_operation_accessors_preserve_typed_operation_selection() {
        let hit = HitEffect {
            damage_bonus: 2,
            damage_type: "psychic".to_string(),
            modifier_id: "rattled".to_string(),
            modifier_label: "Rattled".to_string(),
            modifier_duration: "until end of next turn".to_string(),
            operations: vec![
                HitEffectOperation::Damage(DamageEffectOperation {
                    damage_bonus: 2,
                    damage_type: "psychic".to_string(),
                }),
                HitEffectOperation::ApplyModifier(ModifierEffectOperation {
                    modifier_id: "rattled".to_string(),
                    modifier_label: "Rattled".to_string(),
                    modifier_duration: "until end of next turn".to_string(),
                }),
            ],
        };

        assert_eq!(
            hit.damage_operation()
                .map(|operation| operation.damage_bonus),
            Some(2)
        );
        assert_eq!(
            hit.modifier_operation()
                .map(|operation| operation.modifier_id.as_str()),
            Some("rattled")
        );
    }

    #[test]
    fn ruleset_enum_codes_are_stable() {
        assert_eq!(AbilityDefinitionKind::Spell.code(), "spell");
        assert_eq!(ModifierTenure::Permanent.code(), "permanent");
        assert_eq!(RuleModuleId::ActionResolution.code(), "actionResolution");
        assert_eq!(EffectOperationId::VOCABULARY_VERSION, "1");
        assert_eq!(
            EffectOperationId::OpenReactionWindow.code(),
            "openReactionWindow"
        );
    }

    #[test]
    fn effect_operations_expose_stable_identity_and_handler_status() {
        let damage = HitEffectOperation::Damage(DamageEffectOperation {
            damage_bonus: 1,
            damage_type: "fire".to_string(),
        });
        let healing = HitEffectOperation::Heal(HealingEffectOperation {
            healing_bonus: 1,
            healing_type: "vitality".to_string(),
        });

        assert_eq!(damage.id(), EffectOperationId::Damage);
        assert!(damage.is_currently_supported());
        assert_eq!(healing.id(), EffectOperationId::Heal);
        assert!(healing.is_currently_supported());
    }

    #[test]
    fn artifact_provenance_fails_closed_for_unknown_newer_and_incompatible_versions() {
        let ruleset = super::RulesetMetadata {
            id: "test.ruleset".to_string(),
            name: "Test Ruleset".to_string(),
            version: "1.2.0".to_string(),
            summary: "Compatibility fixture.".to_string(),
            modules: vec![RuleModuleDeclaration::action_resolution(
                ActionResolutionModuleConfiguration::declared_targets_and_line_of_sight(),
            )],
        };
        let provenance = ruleset.artifact_provenance();

        assert_eq!(ruleset.validate_artifact_provenance(&provenance), Ok(()));

        let mut unknown = provenance.clone();
        unknown.ruleset_id = "other.ruleset".to_string();
        assert_eq!(
            ruleset
                .validate_artifact_provenance(&unknown)
                .unwrap_err()
                .code(),
            "unknownRulesetId"
        );

        let mut newer = provenance.clone();
        newer.ruleset_version = "2.0.0".to_string();
        assert_eq!(
            ruleset
                .validate_artifact_provenance(&newer)
                .unwrap_err()
                .code(),
            "newerRulesetVersion"
        );

        let mut incompatible = provenance.clone();
        incompatible.effect_operation_vocabulary_version = "0".to_string();
        assert_eq!(
            ruleset
                .validate_artifact_provenance(&incompatible)
                .unwrap_err()
                .code(),
            "incompatibleRulesetVersion"
        );
    }

    #[test]
    fn module_registry_accepts_static_supported_declarations() {
        let declarations = vec![
            RuleModuleDeclaration::action_resolution(
                ActionResolutionModuleConfiguration::declared_targets_and_line_of_sight(),
            ),
            RuleModuleDeclaration::turn_control(
                TurnControlModuleConfiguration::explicit_turn_order(),
            ),
        ];

        let registry = validate_rule_modules(&declarations).expect("supported modules validate");

        assert_eq!(registry.declarations().len(), 2);
        assert!(registry.turn_control().is_some());
        assert_eq!(
            registry.action_resolution().targeting_policy.code(),
            "declaredTargetsAndLineOfSight"
        );
    }

    #[test]
    fn module_registry_reports_stable_invalid_declaration_codes() {
        assert_eq!(
            RuleModuleId::from_code("not-installed").unwrap_err().code(),
            "unknownRuleModule"
        );

        let duplicate = RuleModuleDeclaration::action_resolution(
            ActionResolutionModuleConfiguration::declared_targets_and_line_of_sight(),
        );
        assert_eq!(
            validate_rule_modules(&[duplicate.clone(), duplicate])
                .unwrap_err()
                .code(),
            "duplicateRuleModuleDeclaration"
        );

        assert_eq!(
            validate_rule_modules(&[]).unwrap_err().code(),
            "missingRequiredRuleModule"
        );

        let incompatible_version = RuleModuleDeclaration {
            module: RuleModuleId::ActionResolution,
            version: "2".to_string(),
            configuration: RuleModuleConfiguration::ActionResolution(
                ActionResolutionModuleConfiguration::declared_targets_and_line_of_sight(),
            ),
        };
        assert_eq!(
            validate_rule_modules(&[incompatible_version])
                .unwrap_err()
                .code(),
            "incompatibleRuleModuleVersion"
        );

        let mismatched_configuration = RuleModuleDeclaration {
            module: RuleModuleId::ActionResolution,
            version: "1".to_string(),
            configuration: RuleModuleConfiguration::TurnControl(
                TurnControlModuleConfiguration::explicit_turn_order(),
            ),
        };
        assert_eq!(
            validate_rule_modules(&[mismatched_configuration])
                .unwrap_err()
                .code(),
            "ruleModuleConfigurationMismatch"
        );
    }

    #[test]
    fn combat_end_policy_codes_are_stable() {
        assert_eq!(CombatEndPolicy::LastSideStanding.code(), "lastSideStanding");
        assert_eq!(CombatEndPolicy::ExplicitOnly.code(), "explicitOnly");
        assert_eq!(
            CombatEndPolicy::ObjectiveSideVictory {
                side_id: "heroes".to_string(),
            }
            .code(),
            "objectiveSideVictory"
        );
    }
}
