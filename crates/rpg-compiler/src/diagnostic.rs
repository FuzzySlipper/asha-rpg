use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpgDiagnosticStage {
    Decode,
    Compatibility,
    Requirements,
    References,
    Semantics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpgDiagnosticSeverity {
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgDiagnostic {
    pub stage: RpgDiagnosticStage,
    pub severity: RpgDiagnosticSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
    pub requirement: Option<String>,
}

impl RpgDiagnostic {
    pub(crate) fn error(
        stage: RpgDiagnosticStage,
        code: &str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            severity: RpgDiagnosticSeverity::Error,
            code: code.to_owned(),
            path: path.into(),
            message: message.into(),
            requirement: None,
        }
    }

    pub(crate) fn with_requirement(mut self, requirement: impl Into<String>) -> Self {
        self.requirement = Some(requirement.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpgCompileFailure {
    pub diagnostics: Vec<RpgDiagnostic>,
}

impl Display for RpgCompileFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "RPG IR compilation failed with {} diagnostic(s)",
            self.diagnostics.len()
        )
    }
}

impl std::error::Error for RpgCompileFailure {}
