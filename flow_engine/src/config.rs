use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlowConfig {
    pub plugin: Vec<PluginConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub plugin_type: PluginType,
    #[serde(default)]
    pub inputs: HashMap<String, String>, // Local name -> Global stream name
    #[serde(default)]
    pub outputs: Vec<String>,            // Global stream names
    #[serde(default)]
    pub params: Option<toml::Value>,     // Plugin specific parameters
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Producer,
    Processor,
    Consumer,
}

impl FlowConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: FlowConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
