use anyhow::Result;

pub trait Transform {
    /// Human-readable name for this transform, used in log messages and error context.
    ///
    /// Concrete transforms should override this to return their type name so that
    /// diagnostics such as `"Transform failed: VariableSubstitutionTransform"` are
    /// immediately actionable.  The default falls back to `"Transform"`.
    fn name(&self) -> &str {
        "Transform"
    }

    fn apply(&self, input: String) -> Result<String>;
}
