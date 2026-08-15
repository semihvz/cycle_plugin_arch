import os
import subprocess

plugins = [
    ("plugin_ai", "ai_01", "Yapay Zeka Motoru", "AI_INFERENCE_READY"),
    ("plugin_network", "net_01", "Ag Analizoru", "NETWORK_TRAFFIC_OK"),
    ("plugin_storage", "disk_01", "Disk Izleyici", "DISK_READ_WRITE_OK"),
    ("plugin_crypto", "crypt_01", "Kriptografi Motoru", "ENCRYPTION_ACTIVE"),
    ("plugin_ui_bridge", "ui_01", "UI Koprusu", "UI_BRIDGE_CONNECTED")
]

base_dir = "/home/smhvz/Desktop/cycle-orc"

for name, pid, pname, pdata in plugins:
    plugin_dir = os.path.join(base_dir, name)
    
    subprocess.run(["cargo", "new", name, "--lib"], cwd=base_dir)
    
    with open(os.path.join(plugin_dir, "Cargo.toml"), "w") as f:
        f.write(f"""[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
orchestrator = {{ path = "../orchestrator" }}
""")

    with open(os.path.join(plugin_dir, "src/lib.rs"), "w") as f:
        f.write(f"""use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{{EndpointHandler, System, SystemContext}};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ExternalSystem {{
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}}

impl ExternalSystem {{
    pub fn new() -> Self {{
        let ctx = SystemContext::new("{pid}", "{pname}");
        let mut endpoints = HashMap::new();

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {{
                *ctx_clone.is_running.write().unwrap() = true;
                Ok(b"STARTED".to_vec())
            }}) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {{
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"STOPPED".to_vec())
            }}) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::IsWorking,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {{
                let status = *ctx_clone.is_running.read().unwrap();
                Ok(vec![if status {{ 1u8 }} else {{ 0u8 }}])
            }}) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataValid,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {{
                let valid = *ctx_clone.is_data_valid.read().unwrap();
                Ok(vec![if valid {{ 1u8 }} else {{ 0u8 }}])
            }}) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {{
                let mut data = ctx_clone.memory.read();
                if data.is_empty() {{
                    data = b"{pdata}".to_vec();
                }}
                Ok(data)
            }}) as EndpointHandler,
        );

        Self {{ ctx, endpoints }}
    }}
}}

impl System for ExternalSystem {{
    fn id(&self) -> &str {{ &self.ctx.id }}
    fn name(&self) -> &str {{ &self.ctx.name }}
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> {{ &self.endpoints }}
    fn context(&self) -> &SystemContext {{ &self.ctx }}
}}

#[no_mangle]
#[no_mangle]\npub extern \"C\" fn create_plugin() -> *mut Box<dyn System> {{
    let sys: Box<dyn System> = Box::new(ExternalSystem::new());
    sys
}}
""")

    print(f"Building {name}...")
    subprocess.run(["cargo", "build"], cwd=plugin_dir)

print("Done")
