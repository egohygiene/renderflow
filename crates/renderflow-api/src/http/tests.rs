//! Authored, UNRUN transport regressions. Cursor inputs do not start a socket.

use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration,Instant};
use serde_json::Value;
use crate::service::tests::Fixture;
use super::*;
use super::framing;
use super::router::Router;

const TOKEN:&str="synthetic-fixture-token-0000000000000000";
fn router(service:crate::Service)->Router {
    Router {service:Arc::new(service),auth:Arc::new(BearerToken::new(TOKEN.as_bytes()).unwrap()),address:"127.0.0.1:8787".parse().unwrap(),limits:HttpLimits::default()}
}
fn input(method:&str,path:&str,extra:&str,body:&str)->String {
    format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:8787\r\nAuthorization: Bearer {TOKEN}\r\n{extra}\r\n{body}")
}
fn call(router:&Router,raw:&[u8])->super::router::Response {
    match framing::read(&mut Cursor::new(raw),&router.limits,Instant::now(),|head|router.gate(head)) {
        Ok(request)=>router.dispatch(request,"request-fixture"),Err(error)=>router.error(error,"request-fixture"),
    }
}

#[test]
fn authenticated_binary_upload_returns_only_an_immutable_source_reference() {
    let fixture=crate::service::upload_tests::Fixture::new();let router=router(fixture.service());
    let bytes=crate::service::upload_tests::NEW;let hash=crate::package::digest(bytes);
    let raw=input("POST","/v1/pipelines/lyrics/uploads",&format!("Content-Length: {}\r\nContent-Type: application/octet-stream\r\nX-Content-SHA256: {hash}\r\n",bytes.len()),std::str::from_utf8(bytes).unwrap());
    let response=call(&router,raw.as_bytes());assert_eq!(response.status,201);
    let value:Value=serde_json::from_slice(&response.body).unwrap();let id=value["data"]["upload_id"].as_str().unwrap();
    assert_eq!(value["data"]["sha256"],hash);assert_eq!(value["data"]["review_inherited"],false);
    assert!(!String::from_utf8(response.body.clone()).unwrap().contains("New manual words"));
    assert!(response.headers.iter().any(|(name,value)|*name=="Location"&&value==&format!("/v1/uploads/{id}")));
    assert_eq!(call(&router,input("GET",&format!("/v1/uploads/{id}"),"","").as_bytes()).status,200);
    assert_eq!(router.service.active_jobs().unwrap(),0);
    assert_eq!(call(&router,input("DELETE",&format!("/v1/uploads/{id}"),"","").as_bytes()).status,204);
}

#[test]
fn upload_permission_digest_and_body_caps_refuse_before_another_body_read() {
    struct HeaderOnly(Option<Vec<u8>>);
    impl std::io::Read for HeaderOnly {
        fn read(&mut self,buffer:&mut [u8])->std::io::Result<usize> {
            let bytes=self.0.take().expect("refused upload read more body bytes");assert!(bytes.len()<=buffer.len());
            buffer[..bytes.len()].copy_from_slice(&bytes);Ok(bytes.len())
        }
    }
    let fixture=crate::service::upload_tests::Fixture::new();let mut router=router(fixture.service());router.limits.upload_bytes=8;
    let hash=crate::package::digest(b"unused");
    for (extra,status) in [
        (format!("Content-Length: 9\r\nContent-Type: application/octet-stream\r\nX-Content-SHA256: {hash}\r\n"),413),
        ("Content-Length: 2\r\nContent-Type: application/octet-stream\r\n".into(),400),
        (format!("Content-Length: 2\r\nContent-Type: application/json\r\nX-Content-SHA256: {hash}\r\n"),415),
    ] {
        let raw=input("POST","/v1/pipelines/lyrics/uploads",&extra,"");
        let error=framing::read(&mut HeaderOnly(Some(raw.into_bytes())),&router.limits,Instant::now(),|head|router.gate(head)).err().unwrap();
        assert_eq!(error.status,status);
    }
    assert_eq!(router.service.active_jobs().unwrap(),0);
}

#[test]
fn auth_host_and_origin_gates_precede_job_admission_without_echoing_credentials() {
    let fixture=Fixture::new(&["transcript_json"]);let router=router(fixture.service());
    for (raw,status) in [
        ("GET /v1/pipelines HTTP/1.1\r\nHost: 127.0.0.1:8787\r\n\r\n".to_string(),401),
        (input("GET","/v1/pipelines","","").replace("127.0.0.1:8787","rebound.invalid:8787"),403),
        (input("GET","/v1/pipelines","Origin: https://example.invalid\r\n",""),403),
    ] {
        let response=call(&router,raw.as_bytes());assert_eq!(response.status,status);
        assert!(!String::from_utf8(response.body).unwrap().contains(TOKEN));
    }
    assert_eq!(router.service.active_jobs().unwrap(),0);
    let health=call(&router,b"GET /health HTTP/1.1\r\nHost: 127.0.0.1:8787\r\n\r\n");assert_eq!(health.status,200);
}

#[test]
fn ambiguous_framing_and_literal_path_escapes_are_refused() {
    let fixture=Fixture::new(&["transcript_json"]);let router=router(fixture.service());
    for raw in [
        input("POST","/v1/plan","Content-Length: 2\r\nContent-Length: 2\r\nContent-Type: application/json\r\n","{}"),
        input("POST","/v1/plan","Content-Length: 2\r\nTransfer-Encoding: chunked\r\nContent-Type: application/json\r\n","{}"),
        input("POST","/v1/plan","Content-Length: 2\r\nContent-Encoding: gzip\r\nContent-Type: application/json\r\n","{}"),
        input("GET","/v1/jobs/%2e%2e", "", ""),
        input("GET","/v1//pipelines", "", ""),
        input("GET","/v1/pipelines", "", "GET /health HTTP/1.1\r\n\r\n"),
    ] {assert_eq!(call(&router,raw.as_bytes()).status,400);}
    assert_eq!(call(&router,input("POST","/v1/plan","Content-Type: application/json\r\n","{}").as_bytes()).status,411);
    assert_eq!(router.service.active_jobs().unwrap(),0);
}

#[test]
fn body_header_type_and_json_contract_limits_are_direct_refusals() {
    let fixture=Fixture::new(&["transcript_json"]);let router=router(fixture.service());
    assert_eq!(call(&router,input("POST","/v1/execute","Content-Length: 999999\r\nContent-Type: application/json\r\n","").as_bytes()).status,413);
    assert_eq!(call(&router,input("POST","/v1/execute","Content-Length: 2\r\nContent-Type: text/plain\r\n","{}").as_bytes()).status,415);
    for body in ["[]",r#"{"pipeline_id":"page","pipeline_id":"page"}"#,r#"{"pipeline_id":"page","config_path":"/host/secret"}"#] {
        let raw=input("POST","/v1/plan",&format!("Content-Length: {}\r\nContent-Type: application/json\r\n",body.len()),body);
        assert_eq!(call(&router,raw.as_bytes()).status,400);
    }
    let huge=input("GET","/health",&format!("X-Large: {}\r\n","x".repeat(router.limits.header_bytes)),"");
    assert_eq!(call(&router,huge.as_bytes()).status,431);assert_eq!(router.service.active_jobs().unwrap(),0);
}

#[test]
fn valid_posts_expose_status_and_finite_sdk_event_replay() {
    let fixture=Fixture::new(&["transcript_json"]);let router=router(fixture.service());let body=r#"{"pipeline_id":"page"}"#;
    let raw=input("POST","/v1/inspect",&format!("Content-Length: {}\r\nContent-Type: application/json\r\n",body.len()),body);
    let response=call(&router,raw.as_bytes());assert_eq!(response.status,202);
    let value:Value=serde_json::from_slice(&response.body).unwrap();let id=value["data"]["job_id"].as_str().unwrap();
    assert!(response.headers.iter().any(|(name,value)|*name=="Location" && value==&format!("/v1/jobs/{id}")));
    let deadline=Instant::now()+Duration::from_secs(15);
    while !router.service.job(id).unwrap().state.terminal() {
        assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));
    }
    let response=call(&router,input("GET",&format!("/v1/jobs/{id}/events?after=0"),"","").as_bytes());
    assert_eq!(response.status,200);assert_eq!(response.media,"text/event-stream");
    let text=String::from_utf8(response.body).unwrap();assert!(text.contains("event: progress") && text.contains("renderflow.progress/v1") && text.contains("event: job"));
    let bad=call(&router,input("GET",&format!("/v1/jobs/{id}/events?after=0"),"Last-Event-ID: 0\r\n","").as_bytes());assert_eq!(bad.status,400);
}

#[test]
fn http_configuration_is_loopback_and_bounded_before_socket_start() {
    let value=include_bytes!("../../../../examples/http-api/service.json");
    let mut config:HttpConfig=renderflow::evidence::json::object(value,65536).unwrap();config.validate().unwrap();
    config.bind="0.0.0.0:8787".parse().unwrap();assert!(config.validate().is_err());
    config.bind="127.0.0.1:8787".parse().unwrap();config.http.workers=config.http.connections+1;assert!(config.validate().is_err());
    assert!(BearerToken::new(b"short").is_err());assert!(!BearerToken::new(TOKEN.as_bytes()).unwrap().authorize(b"wrong"));
}

#[test]
fn maintained_openapi_envelopes_match_serialized_job_fields() {
    let api:Value=serde_json::from_str(OPENAPI).unwrap();assert_eq!(api["openapi"],"3.1.0");
    assert_eq!(api["paths"].as_object().unwrap().len(),17);
    let fixture=Fixture::new(&["transcript_json"]);let service=fixture.service();
    let job=service.submit(crate::Operation::Inspect,crate::SubmitRequest{pipeline_id:"page".into(),dry_run:false,upload_id:None}).unwrap();
    let job_id=job.job_id.clone();
    let actual=serde_json::to_value(job).unwrap().as_object().unwrap().keys().cloned().collect::<std::collections::BTreeSet<_>>();
    let declared=api["components"]["schemas"]["JobView"]["required"].as_array().unwrap().iter().map(|value|value.as_str().unwrap().to_string()).collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual,declared);assert_eq!(api["paths"]["/health"]["get"]["security"],serde_json::json!([]));
    assert!(api["paths"]["/v1/execute"]["post"]["requestBody"]["required"].as_bool().unwrap());
    let deadline=Instant::now()+Duration::from_secs(15);
    while !service.job(&job_id).unwrap().state.terminal() {assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
}

#[test]
fn idle_or_absolute_read_errors_become_structured_timeouts() {
    struct TimedOut;
    impl std::io::Read for TimedOut {fn read(&mut self,_:&mut [u8])->std::io::Result<usize>{Err(std::io::ErrorKind::TimedOut.into())}}
    let error=framing::read(&mut TimedOut,&HttpLimits::default(),Instant::now(),|_|Ok(HttpLimits::default().body_bytes)).err().unwrap();assert_eq!(error.status,408);
    let limits=HttpLimits::default();let started=Instant::now()-Duration::from_millis(limits.read_deadline_ms+1);
    assert_eq!(framing::read(&mut Cursor::new(b""),&limits,started,|_|Ok(HttpLimits::default().body_bytes)).err().unwrap().status,408);
}
