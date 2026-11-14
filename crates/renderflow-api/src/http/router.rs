use super::framing::{Head, Request};
use super::{HttpLimits, OPENAPI};
use crate::{ApiError, ApiResult, Operation, Service, SubmitRequest, API_SCHEMA};
use serde::Serialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;

/// Authentication is an adapter port. The shipped implementation uses one
/// local bearer credential; no cloud identity provider or browser session.
pub trait Authorizer: Send + Sync {
    fn authorize(&self, token: &[u8]) -> bool;
}
pub struct BearerToken {
    bytes: Vec<u8>,
}
impl BearerToken {
    pub fn new(bytes: &[u8]) -> ApiResult<Self> {
        if !(32..=256).contains(&bytes.len()) || !bytes.iter().all(u8::is_ascii_graphic) {
            return Err(ApiError::new(
                "invalid_token",
                "A bearer token needs 32–256 ASCII non-whitespace bytes",
                400,
            ));
        }
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }
}
impl Authorizer for BearerToken {
    fn authorize(&self, token: &[u8]) -> bool {
        if token.len() != self.bytes.len() {
            return false;
        }
        token
            .iter()
            .zip(&self.bytes)
            .fold(0_u8, |different, (left, right)| {
                different | (*left ^ *right)
            })
            == 0
    }
}

pub(super) struct Router {
    pub service: Arc<Service>,
    pub auth: Arc<dyn Authorizer>,
    pub address: SocketAddr,
    pub limits: HttpLimits,
}
pub(super) struct Response {
    pub status: u16,
    pub media: &'static str,
    pub body: Vec<u8>,
    pub headers: Vec<(&'static str, String)>,
    pub download: Option<crate::ArtifactDownload>,
}

impl Router {
    pub fn gate(&self, head: &Head) -> ApiResult<usize> {
        if head.header("origin").is_some() {
            return Err(ApiError::new(
                "origin_refused",
                "Browser-origin requests are not enabled",
                403,
            ));
        }
        let host = head
            .header("host")
            .and_then(|value| std::str::from_utf8(value).ok())
            .unwrap_or("");
        let bare_ip = match self.address.ip() {
            std::net::IpAddr::V4(ip) => ip.to_string(),
            std::net::IpAddr::V6(ip) => format!("[{ip}]"),
        };
        let default_port = self.address.port() == 80
            && (host.eq_ignore_ascii_case("localhost") || host == bare_ip);
        if !default_port
            && host != self.address.to_string()
            && !host.eq_ignore_ascii_case(&format!("localhost:{}", self.address.port()))
        {
            return Err(ApiError::new(
                "host_refused",
                "Host does not identify this loopback listener",
                403,
            ));
        }
        if head.method == "GET" && head.path == "/health" {
            return Ok(0);
        }
        let value = head.header("authorization").unwrap_or_default();
        if value.len() < 7
            || !value[..7].eq_ignore_ascii_case(b"Bearer ")
            || !self.auth.authorize(&value[7..])
        {
            return Err(ApiError::new(
                "unauthorized",
                "A valid bearer credential is required",
                401,
            ));
        }
        if let Some(pipeline) = head.upload_pipeline() {
            let limit = self
                .service
                .upload_limit(pipeline)?
                .min(self.limits.upload_bytes);
            let hash = head
                .header("x-content-sha256")
                .and_then(|value| std::str::from_utf8(value).ok())
                .unwrap_or("");
            if !crate::uploads::sha256(hash) {
                return Err(ApiError::new(
                    "upload_digest",
                    "Binary source upload requires an exact lowercase X-Content-SHA256",
                    400,
                ));
            }
            return Ok(limit);
        }
        Ok(self.limits.body_bytes)
    }

    pub fn dispatch(&self, request: Request, id: &str) -> Response {
        match self.route(request, id) {
            Ok(response) => response,
            Err(error) => self.error(error, id),
        }
    }
    pub fn error(&self, error: ApiError, id: &str) -> Response {
        let mut headers = Vec::new();
        if error.status == 401 {
            headers.push(("WWW-Authenticate", "Bearer realm=\"renderflow-api\"".into()));
        }
        Response {
            status: error.status,
            media: "application/json",
            body: serde_json::to_vec(
                &json!({"schema_version":API_SCHEMA,"request_id":id,"error":error}),
            )
            .unwrap_or_else(|_| b"{}".to_vec()),
            headers,
            download: None,
        }
    }
    fn json<T: Serialize>(&self, status: u16, value: T, id: &str) -> ApiResult<Response> {
        let body =
            serde_json::to_vec(&json!({"schema_version":API_SCHEMA,"request_id":id,"data":value}))
                .map_err(|_| ApiError::internal())?;
        if body.len() > self.limits.json_response_bytes {
            return Err(ApiError::new(
                "response_limit",
                "API JSON response exceeds configured byte limit",
                413,
            ));
        }
        Ok(Response {
            status,
            media: "application/json",
            body,
            headers: Vec::new(),
            download: None,
        })
    }
    fn route(&self, request: Request, id: &str) -> ApiResult<Response> {
        let head = &request.head;
        let (path, query) = head.path.split_once('?').unwrap_or((&head.path, ""));
        if !query.is_empty() && !path.ends_with("/events") {
            return Err(ApiError::new(
                "query_refused",
                "This endpoint has no query parameters",
                400,
            ));
        }
        if let Some(pipeline) = head.upload_pipeline() {
            let hash = head
                .header("x-content-sha256")
                .and_then(|value| std::str::from_utf8(value).ok())
                .unwrap_or("");
            let view = self.service.upload_source(pipeline, request.body, hash)?;
            let location = format!("/v1/uploads/{}", view.upload_id);
            let mut response = self.json(201, view, id)?;
            response.headers.push(("Location", location));
            return Ok(response);
        }
        match (head.method.as_str(),path) {
            ("GET","/health")=>return self.json(200,json!({"live":true,"service":"renderflow-api","version":env!("CARGO_PKG_VERSION")}),id),
            ("GET","/ready")=>return self.json(if self.service.ready(){200}else{503},json!({"accepting_jobs":self.service.ready(),"pipeline_count":self.service.pipelines().len(),"provider_readiness":"not_probed","runtime_qualified":false}),id),
            ("GET","/openapi.json")=> {
                if OPENAPI.len()>self.limits.json_response_bytes {return Err(ApiError::new("response_limit","OpenAPI response exceeds byte limit",413));}
                return Ok(Response {status:200,media:"application/json",body:OPENAPI.as_bytes().to_vec(),headers:Vec::new(),download:None});
            },
            ("GET","/schemas/renderflow-sdk-api-v1.schema.json")=> {
                let schema=renderflow::api_contract::json_schema().map_err(|_|ApiError::internal())?;
                let body=serde_json::to_vec(&schema).map_err(|_|ApiError::internal())?;
                if body.len()>self.limits.json_response_bytes {return Err(ApiError::new("response_limit","SDK schema exceeds the configured response limit",413));}
                return Ok(Response {status:200,media:"application/schema+json",body,headers:Vec::new(),download:None});
            },
            ("GET","/v1/pipelines")=>return self.json(200,self.service.pipelines(),id),
            ("GET","/v1/capabilities")=> {
                let sdk=renderflow::EngineBuilder::new().build().map_err(|_|ApiError::internal())?.provider_capabilities();
                return self.json(200,json!({"service_profile":"operator_registered","service_profiles":crate::profiles::inventory(),"operations":["inspect","plan","execute","job_status","cancel","events","artifacts","forget","source_upload","upload_status","forget_upload","stream_download","sdk_schema"],
                    "uploads":self.service.pipelines().iter().any(|pipeline|pipeline.source_uploads),"upload_scope":"one immutable source for operator-enabled built-in text profiles", "remote_bind":false,"external_provider_jobs":self.service.pipelines().iter().any(|pipeline|pipeline.local_processes),"durable_jobs":false,"runtime_qualified":false,
                    "limits":self.service.limits(),"http":self.limits,"sdk_inventory":sdk,"sdk_inventory_scope":"Broader library inventory; service admission uses a finite registered profile and explicit operator effect permission"}),id);
            },
            ("GET","/v1/formats")=> {
                use renderflow::composition::CompositionOutput::*;
                let outputs = [Svg,Html,TranscriptJson,TranscriptText,Png,Pdf,Ora,Psd,Idml].iter()
                    .map(|format|json!({"id":format.name(),"media_type":format.media_type(),"implemented":format.supported()})).collect::<Vec<_>>();
                return self.json(200,json!({"profile":renderflow::composition::COMPOSITION_PROFILE,"outputs":outputs,
                    "profiles":crate::profiles::inventory()}),id);
            },
            ("GET","/v1/transforms")=>return self.json(200,json!({"scope":"finite registered service profiles; actual branch selection remains in the SDK",
                "profiles":crate::profiles::inventory()}),id),
            ("POST","/v1/inspect"|"/v1/plan"|"/v1/execute")=> {
                let body:SubmitRequest=renderflow::evidence::json::object(&request.body,self.limits.body_bytes)
                    .map_err(|_|ApiError::new("invalid_json","Request must be a bounded object with known fields and unique keys",400))?;
                let operation=match path {"/v1/inspect"=>Operation::Inspect,"/v1/plan"=>Operation::Plan,_=>Operation::Execute};
                let view=self.service.submit(operation,body)?;let location=format!("/v1/jobs/{}",view.job_id);
                let mut response=self.json(202,view,id)?;response.headers.push(("Location",location));return Ok(response);
            },
            _=>{},
        }
        let parts = path.split('/').collect::<Vec<_>>();
        if let ["", "v1", "uploads", upload] = parts.as_slice() {
            if !crate::uploads::upload_id(upload) {
                return Err(ApiError::new(
                    "upload_not_found",
                    "Unknown or expired upload",
                    404,
                ));
            }
            return match head.method.as_str() {
                "GET" => self.json(200, self.service.upload(upload)?, id),
                "DELETE" => {
                    self.service.forget_upload(upload)?;
                    Ok(Response {
                        status: 204,
                        media: "application/json",
                        body: Vec::new(),
                        headers: Vec::new(),
                        download: None,
                    })
                }
                _ => Err(ApiError::new(
                    "route_not_found",
                    "Unknown method or versioned API route",
                    404,
                )),
            };
        }
        if parts.len() >= 4 && parts[1] == "v1" && parts[2] == "jobs" {
            let job = parts[3];
            if job.len() != 68
                || !job.starts_with("job-")
                || !job[4..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(ApiError::new("job_not_found", "Unknown job ID", 404));
            }
            match (head.method.as_str(), parts.as_slice()) {
                ("GET", [_, _, _, _]) => return self.json(200, self.service.job(job)?, id),
                ("DELETE", [_, _, _, _]) => {
                    let view = self.service.cancel(job)?;
                    return self.json(if view.state.terminal() { 200 } else { 202 }, view, id);
                }
                ("DELETE", [_, _, _, _, "retention"]) => {
                    self.service.forget(job)?;
                    return Ok(Response {
                        status: 204,
                        media: "application/json",
                        body: Vec::new(),
                        headers: Vec::new(),
                        download: None,
                    });
                }
                ("GET", [_, _, _, _, "artifacts"]) => {
                    return self.json(200, self.service.artifacts(job)?, id)
                }
                ("GET" | "HEAD", [_, _, _, _, "artifacts", artifact]) => {
                    if head.header("range").is_some() {
                        return Err(ApiError::new("range_refused","Partial artifact delivery is not supported; request the full verified snapshot",416));
                    }
                    let download = self.service.prepare_download(job, artifact)?;
                    let media=match download.media_type.as_str() {
                        "application/json"=>"application/json","text/plain"=>"text/plain","text/html"=>"text/html",
                        "image/svg+xml"=>"image/svg+xml","image/png"=>"image/png","application/pdf"=>"application/pdf",
                        "text/vtt"=>"text/vtt","application/x-subrip"=>"application/x-subrip","application/ttml+xml"=>"application/ttml+xml",
                        "text/markdown"=>"text/markdown","application/yaml"=>"application/yaml","application/xml"=>"application/xml",
                        "application/epub+zip"=>"application/epub+zip",
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"=>"application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                        _=>"application/octet-stream",
                    };
                    let headers = vec![
                        (
                            "Content-Disposition",
                            format!("attachment; filename=\"{}\"", download.filename),
                        ),
                        ("X-Content-SHA256", download.sha256.clone()),
                        ("ETag", format!("\"sha256-{}\"", download.sha256)),
                    ];
                    return Ok(Response {
                        status: 200,
                        media,
                        body: Vec::new(),
                        headers,
                        download: Some(download),
                    });
                }
                ("GET", [_, _, _, _, "events"]) => {
                    let header = head
                        .header("last-event-id")
                        .map(|value| std::str::from_utf8(value).unwrap_or("invalid"));
                    if !query.is_empty() && header.is_some() {
                        return Err(ApiError::new(
                            "invalid_cursor",
                            "Use after or Last-Event-ID, not both",
                            400,
                        ));
                    }
                    let cursor = if query.is_empty() {
                        header.unwrap_or("0")
                    } else {
                        query.strip_prefix("after=").unwrap_or("invalid")
                    };
                    if cursor.is_empty()
                        || cursor.len() > 20
                        || !cursor.bytes().all(|byte| byte.is_ascii_digit())
                    {
                        return Err(ApiError::new(
                            "invalid_cursor",
                            "Event cursor must be a nonnegative integer",
                            400,
                        ));
                    }
                    let after = cursor.parse::<u64>().map_err(|_| {
                        ApiError::new("invalid_cursor", "Event cursor is out of range", 400)
                    })?;
                    let events = self.service.events(job, after)?;
                    let mut body = Vec::new();
                    for event in &events.events {
                        body.extend_from_slice(
                            format!(
                                "id: {}\nevent: progress\ndata: {}\n\n",
                                event.id,
                                serde_json::to_string(&event.event)
                                    .map_err(|_| ApiError::internal())?
                            )
                            .as_bytes(),
                        );
                    }
                    let state = json!({"job_id":events.job_id,"state":events.state,"last_event_id":events.last_event_id,"dropped_events":events.dropped_events,"cursor_gap":events.cursor_gap});
                    body.extend_from_slice(format!("event: job\ndata: {state}\n\n").as_bytes());
                    if body.len() > self.limits.json_response_bytes {
                        return Err(ApiError::new(
                            "response_limit",
                            "Event replay response exceeds byte limit",
                            413,
                        ));
                    }
                    return Ok(Response {
                        status: 200,
                        media: "text/event-stream",
                        body,
                        headers: vec![("X-Accel-Buffering", "no".into())],
                        download: None,
                    });
                }
                _ => {}
            }
        }
        Err(ApiError::new(
            "route_not_found",
            "Unknown method or versioned API route",
            404,
        ))
    }
}
