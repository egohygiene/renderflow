//! Support commands do not render, initialize logging, inspect tools or upload.
use anyhow::Result;
use crate::{cli::DiagnosticsCommands,diagnostics};
pub(crate) fn run(command:DiagnosticsCommands)->Result<()>{
    let owner=diagnostics::owner();let cancel=||false;
    let value=match command{
        DiagnosticsCommands::Preview{request}=>serde_json::to_value(owner.preview(&owner.read_request(&request,&cancel)?,&cancel)?)?,
        DiagnosticsCommands::Bundle{request,output}=>serde_json::to_value(owner.create_bundle(&owner.read_request(&request,&cancel)?,&output,&cancel)?)?,
        DiagnosticsCommands::Inspect{bundle}=>serde_json::to_value(owner.inspect_bundle(&bundle,&cancel)?)?,
        DiagnosticsCommands::Doctor=>serde_json::to_value(owner.health())?,
    };
    println!("{}",serde_json::to_string_pretty(&value)?);Ok(())
}
