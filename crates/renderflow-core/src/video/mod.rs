//! Whole-file video delivery transforms.
//!
//! Temporal decomposition and reconstruction intentionally remain outside this
//! module and are owned by Aniflow.

mod handbrake;

pub use handbrake::{
    execute_handbrake, execute_handbrake_with_progress, plan_handbrake,
    HandBrakeCapabilityContract, HandBrakeLimits, HandBrakePreset, HandBrakePresetContract,
    HandBrakeTransformPlan, HandBrakeTransformReport, HandBrakeTransformRequest,
    HandBrakeValidation, ANIFLOW_RECONSTRUCT_CAPABILITY_ID_V1, ANIFLOW_SEGMENT_CAPABILITY_ID_V1,
    HANDBRAKE_CAPABILITY_CONTRACT_SCHEMA_V1, HANDBRAKE_PROVIDER_ID, HANDBRAKE_TOOL_ID,
    HANDBRAKE_TRANSFORM_CAPABILITY_ID_V1, HANDBRAKE_TRANSFORM_SCHEMA_V1,
};
