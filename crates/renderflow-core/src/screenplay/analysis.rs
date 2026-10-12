//! Deterministic core analysis of the explicitly adopted Fountain body profile.
//!
//! These are counts of independently parsed source constructs, not afterwriting
//! statistics, inferred production locations, spoken duration, or PDF layout.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::native::{parse_native, NativeScreenplayFormat};
use super::{ScreenplayElementKind, MAX_INPUT_BYTES};

pub const ANALYSIS_SCHEMA_V1: &str = "renderflow.screenplay-analysis/v1";
pub const ANALYSIS_ALGORITHM_ID: &str = "renderflow.strict-fountain-body-counts";
pub const ANALYSIS_ALGORITHM_VERSION: u32 = 1;
pub const ANALYSIS_SCOPE: &str = "Printable ASCII and LF, body-only Fountain accepted by Renderflow's strict native parser. Scene headings and character cues are exact source text; repeated headings remain separate occurrences. Counts do not infer locations, character identities, spoken duration, or PDF layout and are not afterwriting statistics.";
pub const ANALYSIS_TOKEN_UNIT: &str = "ASCII whitespace-delimited tokens; punctuation remains attached and parentheticals are excluded from dialogue word counts.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayAnalysisAlgorithm {
    pub id: String,
    pub version: u32,
    pub origin: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayAnalysisSource {
    pub format: String,
    /// Digest of the actual bytes passed to `analyze_fountain`.
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayElementCounts {
    pub scene_heading: u64,
    pub action: u64,
    pub character: u64,
    pub dialogue: u64,
    pub parenthetical: u64,
    pub transition: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplaySceneHeadingOccurrence {
    /// Zero-based index in the independently parsed element sequence.
    pub element_index: u64,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayCharacterCounts {
    pub cue_occurrences: u64,
    pub dialogue_count: u64,
    pub dialogue_word_count: u64,
}

/// A stable source analysis: no timestamps, machine paths, provider stdout,
/// guessed semantics, PDF-derived metrics, or provider-origin claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenplayAnalysis {
    pub schema_version: String,
    pub algorithm: ScreenplayAnalysisAlgorithm,
    pub source: ScreenplayAnalysisSource,
    pub qualification: String,
    pub scope: String,
    pub token_unit: String,
    pub element_counts: ScreenplayElementCounts,
    pub total_elements: u64,
    pub scene_headings: Vec<ScreenplaySceneHeadingOccurrence>,
    /// Sorted by the exact cue text. Suffixes and punctuation are not removed;
    /// this map does not assert that two differently written cues are people.
    pub characters: BTreeMap<String, ScreenplayCharacterCounts>,
    pub action_word_count: u64,
    pub dialogue_word_count: u64,
}

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct ScreenplayAnalysisError {
    pub code: &'static str,
    pub message: String,
}

fn failure(code: &'static str, message: impl Into<String>) -> ScreenplayAnalysisError {
    ScreenplayAnalysisError { code, message: message.into() }
}

/// Analyze actual Fountain source bytes without launching any provider.
///
/// The report is deterministic for identical bytes and algorithm version.
/// Qualification stays explicit: authoring this implementation does not prove
/// native PDF rendering or runtime compatibility. A serialized report received
/// from elsewhere is merely a claim until independently recomputed from source.
pub fn analyze_fountain(bytes: &[u8]) -> Result<ScreenplayAnalysis, ScreenplayAnalysisError> {
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(failure("screenplay.analysis.bounds", "Fountain input must contain between one byte and 16 MiB"));
    }
    if bytes.iter().any(|byte| *byte != b'\n' && !(0x20..=0x7e).contains(byte)) {
        return Err(failure("screenplay.analysis.encoding", "this analysis profile accepts only printable ASCII and LF; tabs, CR, BOMs and non-ASCII text are unsupported"));
    }
    // afterwriting uses this substring to dispatch input to its FDX parser,
    // including when it appears in an otherwise ordinary action paragraph.
    if bytes.windows(b"<?xml".len()).any(|window| window == b"<?xml") {
        return Err(failure("screenplay.analysis.xml_dispatch", "Fountain containing <?xml is unsupported because afterwriting may dispatch it as FDX"));
    }
    let screenplay = parse_native(bytes, NativeScreenplayFormat::Fountain)
        .map_err(|error| failure("screenplay.analysis.profile", error.to_string()))?;
    let mut report = ScreenplayAnalysis {
        schema_version: ANALYSIS_SCHEMA_V1.to_owned(),
        algorithm: ScreenplayAnalysisAlgorithm {
            id: ANALYSIS_ALGORITHM_ID.to_owned(),
            version: ANALYSIS_ALGORITHM_VERSION,
            origin: "renderflow-core".to_owned(),
        },
        source: ScreenplayAnalysisSource {
            format: "fountain".to_owned(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            size_bytes: bytes.len() as u64,
        },
        qualification: "unqualified".to_owned(),
        scope: ANALYSIS_SCOPE.to_owned(),
        token_unit: ANALYSIS_TOKEN_UNIT.to_owned(),
        element_counts: ScreenplayElementCounts::default(),
        total_elements: screenplay.elements.len() as u64,
        scene_headings: Vec::new(),
        characters: BTreeMap::new(),
        action_word_count: 0,
        dialogue_word_count: 0,
    };
    let mut character_cues_by_id = BTreeMap::new();
    for (index, element) in screenplay.elements.iter().enumerate() {
        match element.kind {
            ScreenplayElementKind::SceneHeading => {
                report.element_counts.scene_heading += 1;
                report.scene_headings.push(ScreenplaySceneHeadingOccurrence {
                    element_index: index as u64,
                    text: element.text.clone(),
                });
            }
            ScreenplayElementKind::Action => {
                report.element_counts.action += 1;
                report.action_word_count += element.text.split_ascii_whitespace().count() as u64;
            }
            ScreenplayElementKind::Character => {
                report.element_counts.character += 1;
                character_cues_by_id.insert(element.id.as_str(), element.text.as_str());
                report.characters.entry(element.text.clone()).or_default().cue_occurrences += 1;
            }
            ScreenplayElementKind::Dialogue => {
                report.element_counts.dialogue += 1;
                let word_count = element.text.split_ascii_whitespace().count() as u64;
                report.dialogue_word_count += word_count;
                let cue = element.character_id.as_deref()
                    .and_then(|id| character_cues_by_id.get(id)).copied()
                    .ok_or_else(|| failure("screenplay.analysis.ownership", "dialogue must reference an already observed character cue"))?;
                let counts = report.characters.get_mut(cue)
                    .ok_or_else(|| failure("screenplay.analysis.ownership", "dialogue owner is missing from the exact cue map"))?;
                counts.dialogue_count += 1;
                counts.dialogue_word_count += word_count;
            }
            ScreenplayElementKind::Parenthetical => report.element_counts.parenthetical += 1,
            ScreenplayElementKind::Transition => report.element_counts.transition += 1,
            _ => return Err(failure("screenplay.analysis.profile", "parsed element is outside the six-kind analysis profile")),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRICT_FOUNTAIN: &[u8] = include_bytes!("../../../../tests/fixtures/screenplay/afterwriting-425/strict.fountain");
    const EXPECTED_ANALYSIS: &str = include_str!("../../../../tests/fixtures/screenplay/afterwriting-425/expected-analysis.json");

    #[test]
    fn fixture_matches_independently_authored_expected_counts() {
        let actual = analyze_fountain(STRICT_FOUNTAIN).unwrap();
        let expected: ScreenplayAnalysis = serde_json::from_str(EXPECTED_ANALYSIS).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.total_elements, 17);
        assert_eq!(actual.dialogue_word_count, 14);
        assert_eq!(actual.action_word_count, 12);
    }

    #[test]
    fn identical_source_produces_identical_serialized_analysis() {
        let first = serde_json::to_vec(&analyze_fountain(STRICT_FOUNTAIN).unwrap()).unwrap();
        let second = serde_json::to_vec(&analyze_fountain(STRICT_FOUNTAIN).unwrap()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn headings_remain_ordered_occurrences_without_location_inference() {
        let report = analyze_fountain(STRICT_FOUNTAIN).unwrap();
        assert_eq!(report.scene_headings.len(), 3);
        assert_eq!(report.scene_headings[0].text, report.scene_headings[2].text);
        assert_eq!(report.scene_headings[0].element_index, 0);
        assert_eq!(report.scene_headings[2].element_index, 12);
    }

    #[test]
    fn exact_character_cues_are_sorted_and_not_normalized() {
        let source = b"INT. ROOM - DAY\n\nZOE (V.O.)\nHello, world!\n\nADA\nHi.\n\nZOE\nOne-two three.\n\nZOE (V.O.)\nBye!\n";
        let report = analyze_fountain(source).unwrap();
        assert_eq!(report.characters.keys().map(String::as_str).collect::<Vec<_>>(), ["ADA", "ZOE", "ZOE (V.O.)"]);
        assert_eq!(report.characters["ZOE (V.O.)"].cue_occurrences, 2);
        assert_eq!(report.characters["ZOE (V.O.)"].dialogue_word_count, 3);
        assert_eq!(report.characters["ZOE"].dialogue_word_count, 2);
    }

    #[test]
    fn source_digest_binds_actual_bytes_including_final_newline() {
        let first = analyze_fountain(b"INT. ROOM - DAY\n").unwrap();
        let second = analyze_fountain(b"INT. ROOM - DAY").unwrap();
        assert_eq!(first.element_counts, second.element_counts);
        assert_ne!(first.source.sha256, second.source.sha256);
        assert_eq!(first.source.size_bytes, second.source.size_bytes + 1);
    }

    #[test]
    fn reports_all_six_count_fields_even_when_zero() {
        let report = analyze_fountain(b"INT. ROOM - DAY\n").unwrap();
        assert_eq!(report.element_counts, ScreenplayElementCounts { scene_heading: 1, ..Default::default() });
        assert!(report.characters.is_empty());
        assert_eq!(report.qualification, "unqualified");
        assert_eq!(report.algorithm.origin, "renderflow-core");
    }

    #[test]
    fn rejects_non_ascii_controls_cr_and_bom() {
        for source in [
            "INT. ROOM - DAY\n\nCaf\u{e9}.\n",
            "INT. ROOM - DAY\r\n",
            "\u{feff}INT. ROOM - DAY\n",
            "INT. ROOM - DAY\n\nA\tword.\n",
            "INT. ROOM - DAY\n\nA\u{7f}word.\n",
        ] {
            assert_eq!(analyze_fountain(source.as_bytes()).unwrap_err().code, "screenplay.analysis.encoding");
        }
    }

    #[test]
    fn rejects_xml_dispatch_substring_anywhere() {
        for source in [
            "<?xml version=\"1.0\"?>\n",
            "INT. ROOM - DAY\n\nA sign says <?xml in small type.\n",
            "INT. ROOM - DAY\n\nADA\nI say <?xml aloud.\n",
        ] {
            assert_eq!(analyze_fountain(source.as_bytes()).unwrap_err().code, "screenplay.analysis.xml_dispatch");
        }
    }

    #[test]
    fn refuses_multiline_dialogue_and_action_instead_of_guessing() {
        for source in [
            "INT. ROOM - DAY\n\nADA\nFirst line.\nSecond line.\n",
            "INT. ROOM - DAY\n\nFirst action line.\nSecond action line.\n",
        ] {
            assert_eq!(analyze_fountain(source.as_bytes()).unwrap_err().code, "screenplay.analysis.profile");
        }
    }

    #[test]
    fn refuses_empty_oversize_and_title_pages() {
        assert_eq!(analyze_fountain(b"").unwrap_err().code, "screenplay.analysis.bounds");
        assert_eq!(analyze_fountain(&vec![b'A'; MAX_INPUT_BYTES + 1]).unwrap_err().code, "screenplay.analysis.bounds");
        assert_eq!(analyze_fountain(b"Title: Example\n\nINT. ROOM - DAY\n").unwrap_err().code, "screenplay.analysis.profile");
    }
}
