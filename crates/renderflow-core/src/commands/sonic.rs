//! CLI-only delivery of the Sonic SDK. No model discovery, download or daemon.

use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;

use anyhow::{ensure,Context,Result};
use serde_json::Value;
use crate::ai::{AiLocality,AiModelCatalog,AiSkillRegistry,AiSkillRuntime,OllamaProvider};
use crate::cli::{SonicArguments,SonicCommands};
use crate::dna::sonic::{self,SonicRequest};
use crate::process::ProcessCancellationToken;
use crate::publication::local;

pub fn run(command:SonicCommands)->Result<()> {
    let token=ProcessCancellationToken::new();
    let signal=token.clone();
    ctrlc::set_handler(move||signal.cancel())?;
    let (arguments,output)=match command {
        SonicCommands::Plan(arguments)=>(arguments,None),
        SonicCommands::Build(arguments)=>(arguments.source,Some(arguments.output_dir)),
    };
    let result=with_runtime(&arguments,|runtime|{
        if let Some(output)=&output {
            sonic::build(&SonicRequest {input:arguments.input.clone().into(),output_dir:std::path::PathBuf::from(output.as_str())},runtime,token)
        } else {Ok(serde_json::to_value(sonic::plan(Path::new(&arguments.input),runtime,token)?)?)}
    })?;
    let mut bytes=serde_json::to_vec_pretty(&result)?;bytes.push(b'\n');
    std::io::stdout().write_all(&bytes)?;
    ensure!(result.get("state").and_then(Value::as_str)!=Some("partial"),
        "sonic.required_semantic_not_produced: baseline and failure evidence were retained; inspect the structured semantic status");
    Ok(())
}

fn with_runtime(arguments:&SonicArguments,run:impl FnOnce(Option<&AiSkillRuntime<'_>>)->Result<Value>)->Result<Value> {
    match (&arguments.ai_catalog,&arguments.ollama_endpoint) {
        (None,None)=>run(None),
        (Some(path),Some(endpoint))=>{
            loopback(endpoint)?;
            let catalog=AiModelCatalog::from_json(std::str::from_utf8(&local::read_file(Path::new(path),local::MAX_JSON)?)?)?;
            ensure!(!catalog.providers.is_empty() && catalog.providers.iter().all(|provider|
                provider.locality==AiLocality::Local && provider.adapter=="ollama"),"sonic.cli.catalog: use an explicit local Ollama-only catalog");
            let skills=AiSkillRegistry::bundled()?;
            let provider=OllamaProvider::new(endpoint);
            let runtime=AiSkillRuntime::new(&catalog,&skills,vec![&provider]);
            run(Some(&runtime))
        },
        _=>anyhow::bail!("sonic.cli.runtime: supply both --ai-catalog and --ollama-endpoint"),
    }
}

fn loopback(endpoint:&str)->Result<()> {
    local::url(endpoint)?;
    let authority=endpoint.strip_prefix("http://").or_else(||endpoint.strip_prefix("https://")).context("sonic.endpoint.scheme")?.trim_end_matches('/');
    let local=if let Some(port)=authority.strip_prefix("localhost:") {port.parse::<u16>().is_ok_and(|port|port>0)}
        else {authority.parse::<SocketAddr>().is_ok_and(|address|address.ip().is_loopback() && address.port()>0)};
    ensure!(local,"sonic.endpoint: expected an explicit loopback host and port without a path, credentials, query or fragment");
    Ok(())
}
