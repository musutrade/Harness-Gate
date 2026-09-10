# GH-206 shared-service remediation

The production loader previously rejected shared consumers even when the scheduler
was configured to dispatch one step at a time. Resource diagnostics now honor
that serial execution contract (`parallel = false`, or a single configured worker).
Parallel unordered consumers remain rejected. Dependency ordering, duplicate logs,
service injection conflicts and all quality thresholds remain enforced.

No imported step dependencies, commands, profiles or requiredness were changed.
Regression coverage loads both serial modes, retains parallel rejection and
executes two shared-service consumers exactly once in declaration order.
Resource validation is decomposed by responsibility and added to the versioned
base/head risk inventory; both revisions use the same measurement selection.

Arc-Admin native collector provisioning and authenticated baseline inputs are
separate integration prerequisites. Measurements must retain their actual failed
outcomes; neither diagnostic execution nor receipt collection authorizes authority
transfer or constitutes complete generic-quality acceptance.
