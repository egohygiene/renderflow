use anyhow::Result;

use super::Transform;

/// Replaces emoji characters in text with a `[emoji]` placeholder.
///
/// This is a V1 implementation that prepares the system for future
/// asset-based rendering (e.g., SVG/PDF embedding).
pub struct EmojiTransform;

impl EmojiTransform {
    pub fn new() -> Self {
        EmojiTransform
    }

    fn is_emoji(c: char) -> bool {
        let cp = c as u32;
        matches!(
            cp,
            // Emoticons
            0x1F600..=0x1F64F
            // Miscellaneous Symbols and Pictographs
            | 0x1F300..=0x1F5FF
            // Transport and Map Symbols
            | 0x1F680..=0x1F6FF
            // Supplemental Symbols and Pictographs
            | 0x1F900..=0x1F9FF
            // Symbols and Pictographs Extended-A
            | 0x1FA00..=0x1FAFF
            // Miscellaneous Symbols
            | 0x2600..=0x26FF
            // Dingbats
            | 0x2700..=0x27BF
            // Enclosed Alphanumeric Supplement (regional indicators, etc.)
            | 0x1F1E0..=0x1F1FF
            // Variation Selectors (emoji presentation selectors)
            | 0xFE00..=0xFE0F
            // Combining Enclosing Keycap
            | 0x20E3
            // Mahjong Tile Red Dragon
            | 0x1F004
            // Playing Card Black Joker
            | 0x1F0CF
        )
    }
}

impl Default for EmojiTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform for EmojiTransform {
    fn name(&self) -> &'static str {
        "EmojiTransform"
    }

    fn apply(&self, input: String) -> Result<String> {
        let mut result = String::with_capacity(input.len());
        for c in input.chars() {
            if Self::is_emoji(c) {
                result.push_str("[emoji]");
            } else {
                result.push(c);
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emoji_transform_replaces_emoticon() {
        let transform = EmojiTransform::new();
        let result = transform.apply("Hello 😀 World".to_string()).unwrap();
        assert_eq!(result, "Hello [emoji] World");
    }

    #[test]
    fn test_emoji_transform_replaces_multiple_emoji() {
        let transform = EmojiTransform::new();
        let result = transform.apply("😀😂🎉".to_string()).unwrap();
        assert_eq!(result, "[emoji][emoji][emoji]");
    }

    #[test]
    fn test_emoji_transform_preserves_plain_text() {
        let transform = EmojiTransform::new();
        let input = "Hello, World!".to_string();
        let result = transform.apply(input.clone()).unwrap();
        assert_eq!(result, input);
    }

    #[test]
    fn test_emoji_transform_empty_string() {
        let transform = EmojiTransform::new();
        let result = transform.apply(String::new()).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_emoji_transform_no_crash_on_unknown_characters() {
        let transform = EmojiTransform::new();
        // Mix of ASCII, Unicode letters, and emoji
        let result = transform
            .apply("Caf\u{00E9} ☕ is great 👍".to_string())
            .unwrap();
        assert_eq!(result, "Caf\u{00E9} [emoji] is great [emoji]");
    }

    #[test]
    fn test_emoji_transform_misc_symbols() {
        let transform = EmojiTransform::new();
        // ☀ (U+2600) is in Miscellaneous Symbols
        let result = transform.apply("sunny ☀ day".to_string()).unwrap();
        assert_eq!(result, "sunny [emoji] day");
    }

    #[test]
    fn test_emoji_transform_dingbats() {
        let transform = EmojiTransform::new();
        // ✈ (U+2708) is in Dingbats
        let result = transform.apply("fly ✈ away".to_string()).unwrap();
        assert_eq!(result, "fly [emoji] away");
    }

    #[test]
    fn test_emoji_transform_text_with_newlines() {
        let transform = EmojiTransform::new();
        let result = transform
            .apply("Line one 😀\nLine two 🎉".to_string())
            .unwrap();
        assert_eq!(result, "Line one [emoji]\nLine two [emoji]");
    }
}
