use super::{ScopeError, ScopeResult};
use crate::project::Project;
use crate::utils::fs as output_fs;

impl ScopeResult {
    pub fn write_reports(&self, project: &Project) -> std::result::Result<(), ScopeError> {
        let changed = if self.changed_files.is_empty() {
            String::new()
        } else {
            format!("{}\n", self.changed_files.join("\n"))
        };
        output_fs::write(&project.reports.join("changed_files.txt"), changed)
            .map_err(ScopeError::report)?;
        output_fs::write_json(&project.reports.join("scope.json"), self)
            .map_err(ScopeError::report)?;
        Ok(())
    }
}
