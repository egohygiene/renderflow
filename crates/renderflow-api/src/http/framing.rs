use std::collections::BTreeSet;
use std::io::Read;
use std::time::{Duration,Instant};
use crate::{ApiError,ApiResult};
use super::HttpLimits;

pub(super) struct Head {pub method:String,pub path:String,pub headers:Vec<(String,Vec<u8>)>}
impl Head {
    pub fn header(&self,name:&str)->Option<&[u8]> {
        self.headers.iter().find(|(key,_)|key==name).map(|(_,value)|value.trim_ascii())
    }
}
pub(super) struct Request {pub head:Head,pub body:Vec<u8>}

pub(super) fn read<R:Read>(input:&mut R,limits:&HttpLimits,started:Instant,
    gate:impl FnOnce(&Head)->ApiResult<()>) -> ApiResult<Request> {
    let mut buffer=Vec::new();
    let (end,head)=loop {
        if started.elapsed()>=Duration::from_millis(limits.read_deadline_ms) {return Err(timeout());}
        let mut headers=[httparse::EMPTY_HEADER;64];let mut request=httparse::Request::new(&mut headers);
        match request.parse(&buffer) {
            Ok(httparse::Status::Complete(end))=> {
                if end>limits.header_bytes || request.headers.len()>limits.headers {return Err(header_limit());}
                if request.version!=Some(1) {return Err(ApiError::new("http_version","Only HTTP/1.1 is admitted",400));}
                let method=request.method.ok_or_else(bad)?.to_string();let path=request.path.ok_or_else(bad)?.to_string();
                if path.len()>1024 || !path.starts_with('/') || path.contains(['%','\\']) || path.contains("//") || path.contains("..") {
                    return Err(ApiError::new("request_target","Request targets must use the literal versioned API paths",400));
                }
                let mut names=BTreeSet::new();let mut copied=Vec::new();
                for header in request.headers.iter() {
                    let name=header.name.to_ascii_lowercase();
                    if !names.insert(name.clone()) {return Err(ApiError::new("duplicate_header","Duplicate request header refused",400));}
                    copied.push((name,header.value.to_vec()));
                }
                break (end,Head {method,path,headers:copied});
            },
            Ok(httparse::Status::Partial)=>{},
            Err(httparse::Error::TooManyHeaders)=>return Err(header_limit()),
            Err(_)=>return Err(bad()),
        }
        if buffer.len()>=limits.header_bytes {return Err(header_limit());}
        let mut chunk=[0_u8;1024];let allowed=chunk.len().min(limits.header_bytes-buffer.len());
        let count=input.read(&mut chunk[..allowed]).map_err(io_error)?;
        if count==0 {return Err(bad());} buffer.extend_from_slice(&chunk[..count]);
    };
    // Gate before body decoding/additional reads. A bounded initial header
    // read can already contain coalesced body bytes from the same packet.
    gate(&head)?;
    if head.header("transfer-encoding").is_some() || head.header("content-encoding").is_some()
        || head.header("upgrade").is_some() {return Err(ApiError::new("unsupported_framing","Encoded, chunked and upgraded requests are not admitted",400));}
    if head.header("expect").is_some() {return Err(ApiError::new("expectation_refused","Expect/100-continue is not supported",417));}
    let length=match head.header("content-length") {
        Some(value) if !value.is_empty() && value.len()<=20 && value.iter().all(u8::is_ascii_digit)=>
            std::str::from_utf8(value).ok().and_then(|value|value.parse::<usize>().ok()).ok_or_else(bad)?,
        Some(_)=>return Err(bad()),None if head.method=="POST"=>return Err(ApiError::new("length_required","POST requires Content-Length",411)),
        None=>0,
    };
    if length>limits.body_bytes {return Err(ApiError::new("body_limit","Request body byte limit exceeded",413));}
    if head.method!="POST" && length!=0 {return Err(ApiError::new("unexpected_body","This method does not accept a body",400));}
    if head.method=="POST" {
        let media=head.header("content-type").and_then(|value|std::str::from_utf8(value).ok()).unwrap_or("");
        if !matches!(media.to_ascii_lowercase().as_str(),"application/json"|"application/json; charset=utf-8") {
            return Err(ApiError::new("content_type","POST requires application/json with UTF-8",415));
        }
    }
    let mut body=buffer[end..].to_vec();
    if body.len()>length {return Err(ApiError::new("excess_body","Pipelined or excess request bytes refused",400));}
    while body.len()<length {
        if started.elapsed()>=Duration::from_millis(limits.read_deadline_ms) {return Err(timeout());}
        let mut chunk=[0_u8;1024];let amount=chunk.len().min(length-body.len());
        let count=input.read(&mut chunk[..amount]).map_err(io_error)?;
        if count==0 {return Err(ApiError::new("incomplete_body","Request ended before Content-Length bytes arrived",400));}
        body.extend_from_slice(&chunk[..count]);
    }
    if started.elapsed()>=Duration::from_millis(limits.read_deadline_ms) {return Err(timeout());}
    Ok(Request {head,body})
}

fn header_limit()->ApiError {ApiError::new("header_limit","Request header byte or count limit exceeded",431)}
fn bad()->ApiError {ApiError::new("invalid_http","Malformed HTTP request",400)}
fn timeout()->ApiError {ApiError::new("request_timeout","Request read deadline or idle budget exceeded",408)}
fn io_error(error:std::io::Error)->ApiError {
    if matches!(error.kind(),std::io::ErrorKind::TimedOut|std::io::ErrorKind::WouldBlock) {timeout()} else {bad()}
}
