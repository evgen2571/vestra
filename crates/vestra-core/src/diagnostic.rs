use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Usage,
    Project,
    Semantic,
    Asset,
    Media,
    Backend,
    Render,
    Output,
    Cancellation,
    Internal,
}

impl Category {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Project => "project",
            Self::Semantic => "semantic",
            Self::Asset => "asset",
            Self::Media => "media",
            Self::Backend => "backend",
            Self::Render => "render",
            Self::Output => "output",
            Self::Cancellation => "cancellation",
            Self::Internal => "internal",
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Fatal,
    Warning,
}

impl Severity {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Fatal => "fatal",
            Self::Warning => "warning",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub category: Category,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(
        code: &str,
        category: Category,
        message: impl Into<String>,
        pointer: impl Into<String>,
    ) -> Self {
        Self {
            code: code.to_owned(),
            category,
            severity: Severity::Fatal,
            message: message.into(),
            pointer: Some(pointer.into()),
            related_id: None,
            hint: None,
        }
    }

    #[must_use]
    pub fn warning(code: &str, message: impl Into<String>, pointer: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            category: Category::Semantic,
            severity: Severity::Warning,
            message: message.into(),
            pointer: Some(pointer.into()),
            related_id: None,
            hint: None,
        }
    }

    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    #[must_use]
    pub fn with_related_id(mut self, related_id: impl Into<String>) -> Self {
        self.related_id = Some(related_id.into());
        self
    }
}
