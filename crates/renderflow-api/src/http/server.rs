use std::io::{self,Read,Write};
use std::net::{TcpListener,TcpStream};
use std::path::Path;
use std::sync::{Arc,Mutex,atomic::{AtomicBool,AtomicU64,AtomicUsize,Ordering},mpsc};
use std::time::{Duration,Instant};
use crate::{ApiError,ApiResult,Service};
use super::{Authorizer,HttpConfig};
use super::framing;
use super::router::{Router,Response};

struct Permit(Arc<AtomicUsize>);
impl Drop for Permit {fn drop(&mut self){self.0.fetch_sub(1,Ordering::AcqRel);}}
struct Connection {stream:TcpStream,_permit:Permit}

struct Reader<'a> {stream:&'a mut TcpStream,deadline:Instant,idle:Duration}
impl Read for Reader<'_> {
    fn read(&mut self,buffer:&mut [u8])->io::Result<usize> {
        let remaining=self.deadline.checked_duration_since(Instant::now()).ok_or_else(||io::Error::from(io::ErrorKind::TimedOut))?;
        self.stream.set_read_timeout(Some(remaining.min(self.idle)))?;self.stream.read(buffer)
    }
}

/// Starts only when explicitly called by the optional executable/embedding host.
/// No listener, external tool or provider probe occurs merely by linking core.
pub fn serve(config:HttpConfig,base:&Path,auth:Arc<dyn Authorizer>,stop:Arc<AtomicBool>)->ApiResult<()> {
    config.validate()?;
    let service=Arc::new(Service::new(config.service,base)?);
    let listener=TcpListener::bind(config.bind).map_err(|_|ApiError::new("listener_error","Could not bind the loopback listener",500))?;
    listener.set_nonblocking(true).map_err(|_|ApiError::new("listener_error","Could not configure listener polling",500))?;
    let address=listener.local_addr().map_err(|_|ApiError::internal())?;
    let router=Arc::new(Router {service:service.clone(),auth,address,limits:config.http.clone()});
    let connections=Arc::new(AtomicUsize::new(0));let sequence=Arc::new(AtomicU64::new(1));
    let (sender,receiver)=mpsc::sync_channel::<Connection>(config.http.connections);
    let receiver=Arc::new(Mutex::new(receiver));
    for _ in 0..config.http.workers {
        let receiver=receiver.clone();let router=router.clone();let sequence=sequence.clone();let stop=stop.clone();
        std::thread::spawn(move || loop {
            if stop.load(Ordering::Acquire){break;}
            let connection=match receiver.lock(){Ok(receiver)=>receiver.recv_timeout(Duration::from_millis(100)),Err(_)=>break};
            match connection {
                Ok(mut connection)=> {
                    let id=format!("request-{}",sequence.fetch_add(1,Ordering::Relaxed));let started=Instant::now();
                    let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||process(&mut connection.stream,&router,&id)));
                    let (status,delivered)=outcome.unwrap_or((500,false));
                    // Correlation/status only. Never log URL/query/header/body,
                    // bearer data, source text, SDK models or native error paths.
                    eprintln!("{}",serde_json::json!({"event":"request","request_id":id,"status":status,"delivered":delivered,"elapsed_ms":started.elapsed().as_millis()}));
                },
                Err(mpsc::RecvTimeoutError::Timeout)=>{},Err(mpsc::RecvTimeoutError::Disconnected)=>break,
            }
        });
    }
    eprintln!("{}",serde_json::json!({"event":"listening","address":address.to_string(),"pipeline_count":service.pipelines().len(),"runtime_qualified":false}));
    let mut failure=None;
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream,peer))=> {
                if !peer.ip().is_loopback(){drop(stream);continue;}
                if connections.fetch_update(Ordering::AcqRel,Ordering::Acquire,|count|if count<config.http.connections{Some(count+1)}else{None}).is_err() {
                    // A rejected socket is closed before parsing, auth or body
                    // allocation; no unbounded per-connection thread is spawned.
                    drop(stream);continue;
                }
                let connection=Connection {stream,_permit:Permit(connections.clone())};
                if let Err(error)=sender.try_send(connection){drop(error);}
            },
            Err(error) if error.kind()==io::ErrorKind::WouldBlock=>std::thread::sleep(Duration::from_millis(10)),
            Err(_)=> {failure=Some(ApiError::new("listener_error","Listener accept failed",500));stop.store(true,Ordering::Release);break;},
        }
    }
    drop(listener);drop(sender);drop(receiver);service.shutdown();
    let deadline=Instant::now()+Duration::from_millis(config.http.shutdown_grace_ms);
    while Instant::now()<deadline && (connections.load(Ordering::Acquire)>0 || service.active_jobs().unwrap_or(0)>0) {
        std::thread::sleep(Duration::from_millis(20));
    }
    if service.active_jobs().unwrap_or(0)>0 {
        eprintln!("{}",serde_json::json!({"event":"shutdown","native_jobs_still_returning":true,"recovery_inferred":false}));
    }
    if let Some(error)=failure{Err(error)}else{Ok(())}
}

fn process(stream:&mut TcpStream,router:&Router,id:&str)->(u16,bool) {
    if stream.set_nonblocking(false).is_err(){return (500,false);}
    let started=Instant::now();let mut head_only=false;
    let request={
        let mut input=Reader {stream:&mut *stream,deadline:started+Duration::from_millis(router.limits.read_deadline_ms),idle:Duration::from_millis(router.limits.read_idle_ms)};
        framing::read(&mut input,&router.limits,started,|head|{head_only=head.method=="HEAD";router.gate(head)})
    };
    let response=match request {Ok(request)=>router.dispatch(request,id),Err(error)=>router.error(error,id)};
    let status=response.status;
    let delivered=write_response(stream,response,id,head_only,Duration::from_millis(router.limits.write_deadline_ms)).is_ok();
    (status,delivered)
}

fn write_response(stream:&mut TcpStream,mut response:Response,id:&str,head_only:bool,budget:Duration)->io::Result<()> {
    let deadline=Instant::now()+budget;
    let reason=match response.status {200=>"OK",201=>"Created",202=>"Accepted",204=>"No Content",400=>"Bad Request",401=>"Unauthorized",403=>"Forbidden",404=>"Not Found",
        408=>"Request Timeout",409=>"Conflict",411=>"Length Required",413=>"Content Too Large",415=>"Unsupported Media Type",416=>"Range Not Satisfiable",417=>"Expectation Failed",
        422=>"Unprocessable Content",431=>"Request Header Fields Too Large",503=>"Service Unavailable",_=>"Internal Server Error"};
    let length=response.download.as_ref().map(|download|download.size_bytes).unwrap_or(response.body.len() as u64);
    let mut headers=format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; sandbox\r\nX-Request-ID: {}\r\n",response.status,reason,response.media,length,id);
    for (name,value) in &response.headers {
        if value.contains(['\r','\n']){return Err(io::Error::from(io::ErrorKind::InvalidInput));}
        headers.push_str(name);headers.push_str(": ");headers.push_str(value);headers.push_str("\r\n");
    }
    headers.push_str("\r\n");write_bytes(stream,headers.as_bytes(),deadline)?;
    if !head_only {
        if let Some(download)=response.download.as_mut() {
            let mut output=DeliveryWriter {stream,deadline};
            download.copy_to(&mut output).map_err(|_|io::Error::other("Artifact delivery incomplete"))?;
        }else{write_bytes(stream,&response.body,deadline)?;}
    }
    Ok(())
}
struct DeliveryWriter<'a> {stream:&'a mut TcpStream,deadline:Instant}
impl Write for DeliveryWriter<'_> {
    fn write(&mut self,bytes:&[u8])->io::Result<usize> {write_bytes(self.stream,bytes,self.deadline)?;Ok(bytes.len())}
    fn flush(&mut self)->io::Result<()> {Ok(())}
}
fn write_bytes(stream:&mut TcpStream,bytes:&[u8],deadline:Instant)->io::Result<()> {
    let mut position=0;
    while position<bytes.len() {
        let remaining=deadline.checked_duration_since(Instant::now()).ok_or_else(||io::Error::from(io::ErrorKind::TimedOut))?;
        stream.set_write_timeout(Some(remaining))?;
        let count=stream.write(&bytes[position..bytes.len().min(position+65536)])?;
        if count==0{return Err(io::Error::from(io::ErrorKind::WriteZero));}position+=count;
    }
    Ok(())
}
