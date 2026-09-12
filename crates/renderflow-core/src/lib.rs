//! Renderflow library crate.
//!
//! Exposes the core subsystems for use by benchmarks, tests, and external
//! integrations.  The binary entrypoint lives in `main.rs`.

#![recursion_limit = "256"]

pub mod adapters;
pub mod ai;
pub mod app;
pub mod artifact;
mod assets;
mod audio;
pub mod cache;
pub mod checkpoint;
pub mod cli;
mod commands;
mod compat;
mod config;
pub mod detect;
pub mod dna;
pub mod ebook;
pub mod error;
pub mod evidence;
pub mod font;
pub mod graph;
pub mod hygiene;
mod image;
mod input_format;
pub mod intake;
pub mod optimization;
mod pipeline;
pub mod planning;
pub mod process;
pub mod publication;
mod sdk;
pub mod spec;
pub mod strategies;
pub mod super_resolution;
pub mod toolchain;
pub mod transforms;
pub mod validation;
pub mod video;

pub use dna::{
    ArtifactDna, ArtifactDnaComparison, ArtifactDnaEngine, ArtifactDnaExtractor,
    BuiltinArtifactDnaExtractor, DnaDeterminism, DnaEvidenceOrigin, DnaExtractionBatch,
    DnaExtractionOutcome, DnaExtractionPolicy, DnaExtractionStatus, DnaHygieneAction, DnaModality,
    DnaProtectedReferenceRule, DnaProviderLocality, DnaReviewState,
    ARTIFACT_DNA_COMPARISON_SCHEMA_V1, ARTIFACT_DNA_SCHEMA_V1,
};
pub use evidence::{ArtifactManifest, RunManifest};
pub use font::{
    FontAsset, FontDiagnostic, FontDiagnosticSeverity, FontEmbeddingPermission, FontFormat,
    FontLicense, FontProvenance, FontRedistributionStatus, FontRegistry, FontResolutionReport,
    FontRole, FontRoleBinding, FontTarget, FontValidationReport, LoadedFontRegistry,
    FONT_REGISTRY_SCHEMA_V1, FONT_REGISTRY_VARIABLE, FONT_RESOLUTION_SCHEMA_V1,
};
pub use hygiene::{
    ContentRedactionProvider, HygieneEngine, HygieneEvidence, HygieneFinding, HygieneFindingKind,
    HygieneOutcome, HygieneStatus, RedactionEvidence, RedactionRequest, RedactionResult,
    HYGIENE_EVIDENCE_SCHEMA_V1,
};
pub use intake::{
    ArtifactIntakeProvider, DetectionConfidence, DiscoveredArtifact, EvidenceOrigin,
    InspectionContext, IntakeBudgetUsage, IntakeBudgets, IntakeConflict, IntakeDiagnostic,
    IntakeDiagnosticSeverity, IntakeEngine, IntakeReport, IntakeRequest, IntakeSignal,
    IntakeSignalKind, ProvenanceValue, ProviderInspection, ResolvedArtifactProfile,
    INTAKE_SCHEMA_V1,
};
pub use publication::magazine::{
    build_magazine_candidates, create_magazine_ai_request, MagazineAiCandidate, MagazineAiStatus,
    MagazineCandidateEnvelope, MagazineCandidatePolicy, MAGAZINE_CANDIDATE_SCHEMA_V1,
    MAGAZINE_CANDIDATE_SKILL_ID_V1,
};
pub use sdk::{
    ArtifactProfile, CancellationToken, DiagnosticReport, Engine, EngineBuilder, ExecutionRequest,
    ExecutionResult, InspectionRequest, PlanRequest, ProgressEvent, ProgressReporter,
    ProgressStage, ProviderCapabilities, ProviderPlan, RenderflowError, RenderflowProvider,
    SavedRunAssessment,
};
pub use video::{
    execute_handbrake, execute_handbrake_with_progress, plan_handbrake,
    HandBrakeCapabilityContract, HandBrakeLimits, HandBrakePreset, HandBrakePresetContract,
    HandBrakeTransformPlan, HandBrakeTransformReport, HandBrakeTransformRequest,
    HandBrakeValidation, ANIFLOW_RECONSTRUCT_CAPABILITY_ID_V1, ANIFLOW_SEGMENT_CAPABILITY_ID_V1,
    HANDBRAKE_CAPABILITY_CONTRACT_SCHEMA_V1, HANDBRAKE_PROVIDER_ID, HANDBRAKE_TOOL_ID,
    HANDBRAKE_TRANSFORM_CAPABILITY_ID_V1, HANDBRAKE_TRANSFORM_SCHEMA_V1,
};
