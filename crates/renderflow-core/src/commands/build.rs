use anyhow::Result;
use tracing::info;

use crate::optimization::OptimizationMode;
use crate::planning::{execute, resolve, PlanningRequest};

/// Explicit CLI override for the target intent declared by the configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuildTargetOverride<'a> {
    Configured,
    Target(&'a str),
    Profile(&'a str),
    AllReachable,
}

/// Typed options for one canonical build selection.
#[derive(Debug)]
pub(crate) struct BuildOptions<'a> {
    pub config_path: &'a str,
    pub dry_run: bool,
    pub resume: bool,
    pub optimization: Option<OptimizationMode>,
    pub target_override: BuildTargetOverride<'a>,
    pub exclude: &'a [String],
}

impl BuildOptions<'_> {
    fn planning_request(&self) -> Result<PlanningRequest> {
        let mut request = PlanningRequest::from_path(self.config_path);
        if let Some(optimization) = self.optimization {
            request = request.with_optimization(optimization);
        }
        request = match self.target_override {
            BuildTargetOverride::Configured => request,
            BuildTargetOverride::Target(target) => request.with_target(target),
            BuildTargetOverride::Profile(profile) => request.with_profile(profile),
            BuildTargetOverride::AllReachable => request.with_all_reachable(),
        };
        for selector in self.exclude {
            request = request.with_exclude(selector)?;
        }
        Ok(request)
    }
}

/// Run the canonical Renderflow execution lifecycle using the target intent
/// declared in the v1/v2 configuration.
pub fn run(config_path: &str, dry_run: bool, optimization: Option<OptimizationMode>) -> Result<()> {
    run_selection(BuildOptions {
        config_path,
        dry_run,
        resume: false,
        optimization,
        target_override: BuildTargetOverride::Configured,
        exclude: &[],
    })
}

/// Compatibility entrypoint for watch mode.
///
/// Watch mode itself owns resilience by keeping the watcher alive after an
/// execution error; individual builds still use the exact same canonical
/// planner/executor and fail atomically.
pub fn run_resilient(config_path: &str) -> Result<()> {
    run(config_path, false, None)
}

/// Run the canonical lifecycle with optional CLI target overrides.
pub(crate) fn run_selection(options: BuildOptions<'_>) -> Result<()> {
    if options.dry_run {
        info!(
            "Dry-run mode enabled — planning and bounded provider probes may run, but transforms and output writes are disabled"
        );
    }

    let resolved = resolve(options.planning_request()?)?.with_resume(options.resume);
    info!(
        source = %resolved.source_format(),
        targets = %resolved
            .target_formats()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        depth = resolved.plan().metadata.execution_depth,
        waves = resolved.plan().metadata.execution_waves,
        "Resolved canonical execution plan"
    );

    let result = execute(resolved, options.dry_run)?;
    if options.dry_run {
        // stdout is reserved for machine-readable plan evidence; tracing remains on stderr.
        println!("{}", serde_json::to_string_pretty(&result.plan)?);
    }
    for output in &result.run_manifest.artifact_manifest.outputs {
        if options.dry_run {
            info!("[DRY RUN] Planned output: {}", output);
        } else {
            info!("✔ Output written to: {}", output);
        }
    }
    if let Some(manifest_path) = &result.manifest_path {
        info!(
            run_id = %result.run_manifest.run_id,
            state = ?result.run_manifest.state,
            "Run evidence written to: {}",
            manifest_path
        );
    }
    if !result.is_success() {
        anyhow::bail!(
            "renderflow execution finished with state {:?}; inspect '{}' for structured evidence",
            result.run_manifest.state,
            result
                .manifest_path
                .as_deref()
                .unwrap_or("renderflow-run.json")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn build_options_map_profile_and_exclusions_into_planning_request() {
        let exclusions = vec![
            "family:video".to_string(),
            "provider:tool.ffmpeg".to_string(),
        ];
        let request = BuildOptions {
            config_path: "custom.yaml",
            dry_run: true,
            resume: true,
            optimization: Some(OptimizationMode::Quality),
            target_override: BuildTargetOverride::Profile("everything"),
            exclude: &exclusions,
        }
        .planning_request()
        .unwrap();

        assert_eq!(request.config_path, PathBuf::from("custom.yaml"));
        assert_eq!(request.optimization, Some(OptimizationMode::Quality));
        assert_eq!(request.profile.as_deref(), Some("everything"));
        assert!(request.target.is_none());
        assert!(!request.all_reachable);
        assert_eq!(request.exclude.families, ["video"]);
        assert_eq!(request.exclude.providers, ["tool.ffmpeg"]);
    }

    #[test]
    fn build_options_reject_invalid_exclusion_syntax() {
        let exclusions = vec!["video".to_string()];
        let result = BuildOptions {
            config_path: "renderflow.yaml",
            dry_run: false,
            resume: false,
            optimization: None,
            target_override: BuildTargetOverride::AllReachable,
            exclude: &exclusions,
        }
        .planning_request();

        assert!(result.is_err());
    }
}
