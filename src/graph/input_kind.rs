/// Describes how many inputs a transformation consumes.
///
/// Most transformations operate on a **single** source document and produce a
/// single output document.  Some transformations – such as assembling a book
/// from a collection of pages – consume **multiple** inputs simultaneously and
/// produce a single aggregated output.
///
/// # Example
///
/// ```rust
/// use renderflow::graph::InputKind;
///
/// // The default for a standard edge is a single input.
/// assert_eq!(InputKind::default(), InputKind::Single);
///
/// // A collection-based edge aggregates multiple inputs.
/// let kind = InputKind::Collection;
/// assert!(kind.is_collection());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InputKind {
    /// The transformation operates on a single source document.
    #[default]
    Single,
    /// The transformation consumes a collection of source documents and
    /// produces one aggregated output (e.g. pages → book).
    Collection,
}

impl InputKind {
    /// Return `true` when this is the [`Single`](InputKind::Single) variant.
    pub fn is_single(self) -> bool {
        self == InputKind::Single
    }

    /// Return `true` when this is the [`Collection`](InputKind::Collection) variant.
    pub fn is_collection(self) -> bool {
        self == InputKind::Collection
    }
}

impl std::fmt::Display for InputKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputKind::Single => write!(f, "single"),
            InputKind::Collection => write!(f, "collection"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_single() {
        assert_eq!(InputKind::default(), InputKind::Single);
    }

    #[test]
    fn test_single_is_single() {
        assert!(InputKind::Single.is_single());
        assert!(!InputKind::Single.is_collection());
    }

    #[test]
    fn test_collection_is_collection() {
        assert!(InputKind::Collection.is_collection());
        assert!(!InputKind::Collection.is_single());
    }

    #[test]
    fn test_display_single() {
        assert_eq!(InputKind::Single.to_string(), "single");
    }

    #[test]
    fn test_display_collection() {
        assert_eq!(InputKind::Collection.to_string(), "collection");
    }

    #[test]
    fn test_clone_copy() {
        let kind = InputKind::Collection;
        let copied = kind;
        assert_eq!(kind, copied);
    }

    #[test]
    fn test_equality() {
        assert_eq!(InputKind::Single, InputKind::Single);
        assert_eq!(InputKind::Collection, InputKind::Collection);
        assert_ne!(InputKind::Single, InputKind::Collection);
    }
}
