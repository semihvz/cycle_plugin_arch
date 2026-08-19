use crate::endpoint::StandardEndpoint;
use std::ffi::c_void;
use std::sync::Arc;
use core::sync::atomic::AtomicBool;

// C-ABI Endpoint function signature (Zero-copy, No V-Table)
pub type RawEndpointFn = unsafe extern "C" fn(
    plugin_state: *mut c_void, 
    endpoint_id: u32, 
    payload: *const u8, 
    payload_len: usize, 
    out_buf: *mut u8, 
    out_max_len: usize
) -> usize;

// Context that plugins can access lock-free
pub struct SystemContext {
    pub id: String,
    pub name: String,
    pub is_running: Arc<AtomicBool>,
    pub is_data_valid: Arc<AtomicBool>,
}

impl SystemContext {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            is_running: Arc::new(AtomicBool::new(false)),
            is_data_valid: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub struct SystemInstance {
    pub id: String,
    pub name: String,
    pub context: Arc<SystemContext>,
    // Pointer to internal plugin state (so orchestrator doesn't need to know the type)
    pub plugin_state: *mut c_void,
    // The raw function pointer for endpoints
    pub endpoint_handler: RawEndpointFn,
}

// Ensure the struct can be shared across threads safely
unsafe impl Send for SystemInstance {}
unsafe impl Sync for SystemInstance {}

impl SystemInstance {
    pub fn new(
        id: String, 
        name: String, 
        plugin_state: *mut c_void, 
        endpoint_handler: RawEndpointFn
    ) -> Self {
        Self {
            id: id.clone(),
            name: name.clone(),
            context: Arc::new(SystemContext::new(&id, &name)),
            plugin_state,
            endpoint_handler,
        }
    }

    #[inline(always)]
    pub fn call(&self, endpoint: StandardEndpoint, payload: &[u8], out_buf: &mut [u8]) -> usize {
        unsafe {
            (self.endpoint_handler)(
                self.plugin_state,
                endpoint as u32,
                payload.as_ptr(),
                payload.len(),
                out_buf.as_mut_ptr(),
                out_buf.len()
            )
        }
    }
}
