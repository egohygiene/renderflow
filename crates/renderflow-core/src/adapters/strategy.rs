use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::artifact::{
    Artifact, ArtifactDescriptor, ArtifactStorageClass, ArtifactStore, ArtifactTransform,
};
use crate::assets::normalize_asset_paths;
use crate::config::OutputType;
use crate::graph::Format;
use crate::input_format::InputFormat;
use crate::pipeline::Pipeline;
use crate::strategies::{select_strategy, RenderContext};

/// Artifact-native compatibility adapter for the mature document/image/audio
/// output strategies. The application planner registers this adapter as a graph
/// capability; callers never dispatch to a family-specific top-level pipeline.
pub struct StrategyArtifactTransform {
    from: Format,
    to: Format,
    output_type: OutputType,
    template: Option<String>,
    profile: Option<String>,
    variables: HashMap<String, String>,
    source_asset_root: Option<PathBuf>,
    cache_identity: String,
}

impl StrategyArtifactTransform {
    pub fn new(
        from: Format,
        to: Format,
        template: Option<String>,
        profile: Option<String>,
        variables: BTreeMap<String, String>,
        source_asset_root: Option<PathBuf>,
    ) -> Result<Self> {
        let output_type = output_type_for_format(to).ok_or_else(|| {
            anyhow::anyhow!(
                "format '{}' is not implemented by a built-in output strategy",
                to
            )
        })?;
        let variables: HashMap<String, String> = variables.into_iter().collect();
        let mut identity_variables: Vec<_> = variables.iter().collect();
        identity_variables.sort_by(|left, right| left.0.cmp(right.0));
        let cache_identity = format!(
            "renderflow.strategy-adapter/v1;from={from};to={to};template={template:?};profile={profile:?};source_root={:?};variables={identity_variables:?}",
            source_asset_root
        );
        Ok(Self {
            from,
            to,
            output_type,
            template,
            profile,
            variables,
            source_asset_root,
            cache_identity,
        })
    }

    fn prepare_document_input(
        &self,
        input: &Artifact,
        store: &ArtifactStore,
        work_dir: &Path,
    ) -> Result<PathBuf> {
        let text = store.read_text(input).with_context(|| {
            format!(
                "built-in document adapter requires UTF-8 input for '{}'",
                self.from
            )
        })?;
        let normalized = if let Some(root) = &self.source_asset_root {
            normalize_asset_paths(&text, root)?.into_owned()
        } else {
            text
        };
        let pipeline = Pipeline::with_standard_transforms(&self.variables, &self.output_type);
        let transformed = pipeline
            .run_transforms(normalized)
            .context("built-in document transform phase failed")?;
        let path = work_dir.join(format!("input.{}", self.from));
        fs::write(&path, transformed).with_context(|| {
            format!(
                "failed to stage built-in document input '{}'",
                path.display()
            )
        })?;
        Ok(path)
    }
}

impl ArtifactTransform for StrategyArtifactTransform {
    fn name(&self) -> &str {
        "renderflow.strategy-adapter"
    }

    fn cache_identity(&self) -> String {
        self.cache_identity.clone()
    }

    fn apply(
        &self,
        input: &Artifact,
        output_format: Format,
        store: &ArtifactStore,
    ) -> Result<Artifact> {
        if output_format != self.to {
            anyhow::bail!(
                "strategy adapter planned '{}' but executor requested '{}'",
                self.to,
                output_format
            );
        }

        let work_dir = tempfile::tempdir_in(store.temporary_directory())
            .context("failed to create strategy adapter work directory")?;
        let document_input = document_input_format(self.from);
        let input_path = if document_input.is_some() {
            self.prepare_document_input(input, store, work_dir.path())?
        } else {
            store.payload_path(input)?
        };
        let output_path = work_dir.path().join(format!("output.{}", self.to));
        let strategy = select_strategy(
            &self.output_type,
            self.template.as_deref(),
            "templates",
            self.profile.as_deref(),
        )?;
        let input_path_string = input_path
            .to_str()
            .context("strategy input path contains non-UTF8 characters")?;
        let output_path_string = output_path
            .to_str()
            .context("strategy output path contains non-UTF8 characters")?;
        let context = RenderContext {
            input_path: input_path_string,
            input_format: document_input.unwrap_or_default(),
            output_path: output_path_string,
            variables: &self.variables,
            dry_run: false,
        };
        strategy.render(&context).with_context(|| {
            format!(
                "built-in strategy adapter failed for '{}' -> '{}'",
                self.from, self.to
            )
        })?;
        if !output_path.is_file() {
            anyhow::bail!(
                "built-in strategy '{}' -> '{}' completed without producing '{}'",
                self.from,
                self.to,
                output_path.display()
            );
        }
        store.import_path(
            &output_path,
            ArtifactDescriptor::for_format(output_format, ArtifactStorageClass::Intermediate)
                .with_source(input.id().clone())
                .with_metadata("renderflow.adapter", "builtin.strategy")
                .with_metadata("renderflow.from", self.from.to_string())
                .with_metadata("renderflow.to", self.to.to_string()),
        )
    }
}

pub fn document_input_format(format: Format) -> Option<InputFormat> {
    match format {
        Format::Markdown => Some(InputFormat::Markdown),
        Format::Docx => Some(InputFormat::Docx),
        Format::Html => Some(InputFormat::Html),
        Format::Epub => Some(InputFormat::Epub),
        Format::Rst => Some(InputFormat::Rst),
        Format::Latex => Some(InputFormat::Latex),
        _ => None,
    }
}

pub fn output_type_for_format(format: Format) -> Option<OutputType> {
    match format {
        Format::Html => Some(OutputType::Html),
        Format::Pdf => Some(OutputType::Pdf),
        Format::Docx => Some(OutputType::Docx),
        _ => {
            let value = format.to_string();
            if let Ok(audio) = value.parse::<crate::audio::AudioFormat>() {
                if audio.supports_encoding() {
                    return Some(OutputType::Audio(audio));
                }
            }
            if let Ok(image) = value.parse::<crate::image::ImageFormat>() {
                if image.supports_encoding() {
                    return Some(OutputType::Image(image));
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_builtin_document_and_media_outputs() {
        assert_eq!(output_type_for_format(Format::Html), Some(OutputType::Html));
        assert!(matches!(
            output_type_for_format(Format::Png),
            Some(OutputType::Image(_))
        ));
        assert!(matches!(
            output_type_for_format(Format::Flac),
            Some(OutputType::Audio(_))
        ));
        assert!(output_type_for_format(Format::Svg).is_none());
    }

    #[test]
    fn document_input_mapping_is_explicit() {
        assert_eq!(
            document_input_format(Format::Markdown),
            Some(InputFormat::Markdown)
        );
        assert_eq!(document_input_format(Format::Png), None);
    }
}
