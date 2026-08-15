use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlowConfig {
    #[serde(flatten)]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginInput {
    pub source: String,
    pub stream_id: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginConfig {
    pub plugin_name: String,
    #[serde(default)]
    pub plugin_inputs: Vec<PluginInput>,
    #[serde(default)]
    pub plugin_params: serde_json::Value,
    #[serde(default)]
    pub plugin_outputs: Vec<String>,
}

impl FlowConfig {
    pub fn load(path: &str) -> anyhow::Result<Vec<PluginConfig>> {
        let content = fs::read_to_string(path)?;
        let config: Vec<PluginConfig> = serde_json::from_str(&content)?;
        Ok(config)
    }
}
