use serde::Serialize;
use specman::{
    ResolvedTemplate, TemplateCatalog as LibraryTemplateCatalog, TemplateScenario, WorkspacePaths,
};
use std::fmt;

use crate::error::CliError;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    Specification,
    Implementation,
    Scratch,
}

pub struct TemplateCatalog {
    catalog: LibraryTemplateCatalog,
}

impl TemplateCatalog {
    pub fn new(workspace: WorkspacePaths) -> Self {
        Self {
            catalog: LibraryTemplateCatalog::new(workspace),
        }
    }

    pub fn set_pointer(
        &self,
        kind: TemplateKind,
        locator: impl AsRef<str>,
    ) -> Result<ResolvedTemplate, CliError> {
        self.catalog
            .set_pointer(kind.scenario(), locator.as_ref())
            .map_err(CliError::from)
    }

    pub fn remove_pointer(&self, kind: TemplateKind) -> Result<ResolvedTemplate, CliError> {
        self.catalog
            .remove_pointer(kind.scenario())
            .map_err(CliError::from)
    }
}

impl TemplateKind {
    fn scenario(self) -> TemplateScenario {
        match self {
            TemplateKind::Specification => TemplateScenario::Specification,
            TemplateKind::Implementation => TemplateScenario::Implementation,
            TemplateKind::Scratch => TemplateScenario::ScratchPad,
        }
    }
}

impl fmt::Display for TemplateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            TemplateKind::Specification => "spec",
            TemplateKind::Implementation => "impl",
            TemplateKind::Scratch => "scratch",
        };
        write!(f, "{label}")
    }
}
