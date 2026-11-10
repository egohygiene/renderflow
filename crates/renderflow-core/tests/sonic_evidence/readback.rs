//! Retained-bundle cases authored for the deferred qualification pass.
use super::*;
use renderflow::dna::sonic::readback::{inspect,ReadbackRequest};
fn selected(root:&Path)->(PathBuf,ReadbackRequest) {
    let (input,_)=fixture(root);let output=root.join("bundle");sonic::build(&request(&input,&output),None,token()).unwrap();
    let pin=hash(&fs::read(output.join("sonic-run.json")).unwrap());
    (input.clone(),ReadbackRequest {schema_version:"renderflow.sonic-readback-request/v1".into(),directory:output,receipt_sha256:pin,input:Some(input)})
}
#[test]
fn current_bundle_readback_checks_bytes_without_creating_a_store_or_approval() {
    let root=tempfile::tempdir().unwrap();let (_,r)=selected(root.path());let before=fs::read(r.directory.join("sonic-run.json")).unwrap();
    let observed=inspect(&r,token()).unwrap();assert_eq!(observed.source_state,"current");assert_eq!(observed.bundle_state,"complete");assert_eq!(observed.review_state,"candidate");assert!(!observed.runtime_qualified);
    assert_eq!(observed.publication_approval,"not_granted");assert!(!r.directory.join("artifacts").exists());assert_eq!(fs::read(r.directory.join("sonic-run.json")).unwrap(),before);
}
#[test]
fn changed_original_is_distinct_from_retained_candidate_integrity() {
    let root=tempfile::tempdir().unwrap();let (input,mut r)=selected(root.path());fs::write(root.path().join("source.wav"),b"changed").unwrap();
    assert_eq!(inspect(&r,token()).unwrap().source_state,"unavailable_or_changed");fs::write(&input,b"changed recipe").unwrap();assert_eq!(inspect(&r,token()).unwrap().source_state,"stale");
    fs::remove_file(input).unwrap();assert_eq!(inspect(&r,token()).unwrap().source_state,"unavailable");r.input=None;assert_eq!(inspect(&r,token()).unwrap().source_state,"not_supplied");
}
#[test]
fn external_receipt_pin_and_exact_cas_and_inventory_membership_are_required() {
    let root=tempfile::tempdir().unwrap();let (_,mut r)=selected(root.path());let pin=r.receipt_sha256.clone();r.receipt_sha256="0".repeat(64);assert!(inspect(&r,token()).is_err());r.receipt_sha256=pin;
    fs::write(r.directory.join("extra-private.txt"),b"unexpected").unwrap();assert!(inspect(&r,token()).is_err());fs::remove_file(r.directory.join("extra-private.txt")).unwrap();
    let lineage=read(&r.directory.join("lineage.json"));let relative=lineage["produced"][0]["payload"]["relative_path"].as_str().unwrap();fs::write(r.directory.join(relative),b"changed CAS").unwrap();assert!(inspect(&r,token()).is_err());
}
#[test]
fn required_unavailable_semantics_stay_partial_without_provider_resolution() {
    let root=tempfile::tempdir().unwrap();let (input,mut recipe)=fixture(root.path());recipe.semantic.mode=SemanticMode::Required;save(&input,&recipe);let output=root.path().join("partial");
    sonic::build(&request(&input,&output),None,token()).unwrap();let r=ReadbackRequest {schema_version:"renderflow.sonic-readback-request/v1".into(),directory:output.clone(),receipt_sha256:hash(&fs::read(output.join("sonic-run.json")).unwrap()),input:Some(input)};
    let result=inspect(&r,token()).unwrap();assert_eq!(result.bundle_state,"partial");assert_eq!(result.source_state,"current");assert_eq!(result.provider_state,"not_reobserved; retained declarations only");
}
#[test]
fn loose_or_symbolic_partial_folders_cannot_be_promoted() {
    let root=tempfile::tempdir().unwrap();fs::write(root.path().join("sonic-dna.json"),b"{}").unwrap();let r=ReadbackRequest {schema_version:"renderflow.sonic-readback-request/v1".into(),directory:root.path().into(),receipt_sha256:"a".repeat(64),input:None};
    assert!(inspect(&r,token()).is_err());assert!(!root.path().join("sonic-run.json").exists());assert_eq!(fs::read(root.path().join("sonic-dna.json")).unwrap(),b"{}");
}
