use orchestrator::endpoint::StandardEndpoint;
use orchestrator::system::{EndpointHandler, System, SystemContext};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ExternalSystem {
    ctx: SystemContext,
    endpoints: HashMap<StandardEndpoint, EndpointHandler>,
}

impl ExternalSystem {
    pub fn new() -> Self {
        let ctx = SystemContext::new("disk_01", "Disk Izleyici");
        let mut endpoints = HashMap::new();

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Start,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = true;
                Ok(b"STARTED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::Stop,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                *ctx_clone.is_running.write().unwrap() = false;
                Ok(b"STOPPED".to_vec())
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::IsWorking,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let status = *ctx_clone.is_running.read().unwrap();
                Ok(vec![if status { 1u8 } else { 0u8 }])
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataValid,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let valid = *ctx_clone.is_data_valid.read().unwrap();
                Ok(vec![if valid { 1u8 } else { 0u8 }])
            }) as EndpointHandler,
        );

        let ctx_clone = ctx.clone();
        endpoints.insert(
            StandardEndpoint::DataMonitor,
            Arc::new(move |_ctx: &SystemContext, _payload: Option<Vec<u8>>| {
                let mut data = ctx_clone.memory.read();
                if data.is_empty() {
                    data = b"DISK_READ_WRITE_OK".to_vec();
                }
                Ok(data)
            }) as EndpointHandler,
        );

        Self { ctx, endpoints }
    }
}

impl System for ExternalSystem {
    fn id(&self) -> &str { &self.ctx.id }
    fn name(&self) -> &str { &self.ctx.name }
    fn endpoints(&self) -> &HashMap<StandardEndpoint, EndpointHandler> { &self.endpoints }
    fn context(&self) -> &SystemContext { &self.ctx }
}

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut Box<dyn System> {
    let sys: Box<dyn System> = Box::new(ExternalSystem::new());
    Box::into_raw(Box::new(sys))
}
