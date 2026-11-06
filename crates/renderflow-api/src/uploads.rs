//! Narrow identifiers shared by the optional local upload adapter.
pub(crate) fn sha256(value:&str)->bool {
    value.len()==64&&value.bytes().all(|byte|byte.is_ascii_digit()||(b'a'..=b'f').contains(&byte))
}
pub(crate) fn upload_id(value:&str)->bool {value.strip_prefix("upload-").is_some_and(sha256)}
