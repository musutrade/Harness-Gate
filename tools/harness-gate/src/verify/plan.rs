use crate::config::StepConfig;
use crate::project::Project;
use crate::scope::ScopeResult;
use std::collections::{BTreeSet, HashMap, HashSet};

pub(super) fn selected_steps<'a>(
    project: &'a Project,
    scope: &ScopeResult,
    profile: &str,
    only_step: Option<&str>,
) -> Vec<&'a StepConfig> {
    let mut selected = project
        .config
        .steps
        .iter()
        .filter(|step| {
            scope.components.contains(&step.component)
                && only_step
                    .map(|id| step.id == id)
                    .unwrap_or_else(|| step.profiles.contains(profile))
        })
        .map(|step| step.id.clone())
        .collect::<BTreeSet<_>>();
    loop {
        let previous = selected.len();
        for step in &project.config.steps {
            if selected.contains(&step.id) {
                selected.extend(step.depends_on.iter().cloned());
            }
        }
        if selected.len() == previous {
            break;
        }
    }
    let candidates = project
        .config
        .steps
        .iter()
        .filter(|step| selected.contains(&step.id))
        .collect::<Vec<_>>();
    topological_order(candidates)
}

#[allow(clippy::needless_lifetimes)]
fn topological_order<'a>(candidates: Vec<&'a StepConfig>) -> Vec<&'a StepConfig> {
    let ids = candidates
        .iter()
        .map(|step| step.id.as_str())
        .collect::<HashSet<_>>();
    let mut by_id = candidates
        .iter()
        .map(|step| (step.id.as_str(), *step))
        .collect::<HashMap<_, _>>();
    let mut emitted = BTreeSet::new();
    let mut ordered = Vec::with_capacity(candidates.len());
    fn emit<'a>(
        id: &str,
        ids: &HashSet<&str>,
        by_id: &mut HashMap<&str, &'a StepConfig>,
        emitted: &mut BTreeSet<String>,
        ordered: &mut Vec<&'a StepConfig>,
    ) {
        if emitted.contains(id) {
            return;
        }
        let Some(step) = by_id.get(id).copied() else {
            return;
        };
        for dependency in &step.depends_on {
            if ids.contains(dependency.as_str()) {
                emit(dependency, ids, by_id, emitted, ordered);
            }
        }
        emitted.insert(id.to_string());
        ordered.push(step);
    }
    for step in &candidates {
        emit(&step.id, &ids, &mut by_id, &mut emitted, &mut ordered);
    }
    ordered
}

#[cfg(test)]
mod tests {
    use super::topological_order;
    use crate::config::FlowConfig;

    #[test]
    fn dependencies_are_stably_topologically_sorted() {
        let source = include_str!("../../presets/generic.flow.toml");
        let mut config: FlowConfig = toml::from_str(source).expect("preset");
        config.steps[0].depends_on = vec![config.steps[1].id.clone()];
        config.validate().expect("valid DAG");
        let ordered = topological_order(config.steps.iter().collect());
        assert_eq!(ordered[0].id, config.steps[1].id);
        assert_eq!(ordered[1].id, config.steps[0].id);
    }
}
