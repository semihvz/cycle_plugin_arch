use flow_engine::config::{FlowConfig, PluginConfig, PluginInput};

#[test]
fn test_plugin_config_deserialization() {
    let json_data = r#"
    [
        {
            "plugin_name": "test_producer",
            "enabled": true,
            "plugin_inputs": [],
            "plugin_params": { "interval": "1m" },
            "plugin_outputs": ["producer_stream"]
        },
        {
            "plugin_name": "test_consumer",
            "plugin_inputs": [
                {
                    "source": "test_producer",
                    "stream_id": "producer_stream",
                    "params": {}
                }
            ]
        }
    ]
    "#;

    let plugins: Vec<PluginConfig> = serde_json::from_str(json_data).expect("Failed to deserialize PluginConfig");
    assert_eq!(plugins.len(), 2);

    assert_eq!(plugins[0].plugin_name, "test_producer");
    assert!(plugins[0].enabled);
    assert_eq!(plugins[0].plugin_outputs, vec!["producer_stream".to_string()]);

    assert_eq!(plugins[1].plugin_name, "test_consumer");
    assert!(plugins[1].enabled); // default_true should yield true
    assert_eq!(plugins[1].plugin_inputs.len(), 1);
    assert_eq!(plugins[1].plugin_inputs[0].stream_id, "producer_stream");
}
