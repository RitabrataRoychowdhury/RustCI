//! Configuration management tools

use anyhow::{Context, Result};
use std::path::PathBuf;
use valkyrie_sdk::{Config, ServerConfig, ClientConfig, config::GlobalConfig, retry::RetryPolicy};
use colored::*;

/// Generate sample configuration file
pub async fn generate_config(output: PathBuf, format: String) -> Result<()> {
    println!("{} Generating sample configuration", "→".green().bold());
    println!("  Output: {}", output.display().to_string().cyan());
    println!("  Format: {}", format.yellow());
    
    let config = create_sample_config();
    
    let content = match format.as_str() {
        "yaml" | "yml" => {
            serde_yaml::to_string(&config)
                .context("Failed to serialize to YAML")?
        }
        "json" => {
            serde_json::to_string_pretty(&config)
                .context("Failed to serialize to JSON")?
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported format: {}. Use 'yaml' or 'json'", format));
        }
    };
    
    std::fs::write(&output, content)
        .with_context(|| format!("Failed to write to {}", output.display()))?;
    
    println!("{} Configuration file generated successfully", "✓".green().bold());
    Ok(())
}

/// Validate configuration file
pub async fn validate_config(config_path: PathBuf) -> Result<()> {
    println!("{} Validating configuration", "→".green().bold());
    println!("  File: {}", config_path.display().to_string().cyan());
    
    match Config::from_file(&config_path) {
        Ok(config) => {
            println!("{} Configuration is valid", "✓".green().bold());
            
            // Show configuration summary
            println!();
            println!("{} Configuration Summary:", "📋".blue().bold());
            
            if let Some(server) = &config.server {
                println!("  {} Server:", "🖥".blue());
                println!("    Bind: {}:{}", server.bind_address, server.port);
                println!("    Max connections: {}", server.max_connections);
                println!("    TLS: {}", if server.enable_tls { "enabled".green() } else { "disabled".red() });
                println!("    Metrics: {}", if server.enable_metrics { "enabled".green() } else { "disabled".red() });
            }
            
            if let Some(client) = &config.client {
                println!("  {} Client:", "💻".blue());
                println!("    Endpoint: {}", client.endpoint);
                println!("    Timeout: {}ms", client.request_timeout_ms);
                println!("    Pooling: {}", if client.enable_pooling { "enabled".green() } else { "disabled".red() });
                println!("    Max connections: {}", client.max_connections);
            }
            
            println!("  {} Global:", "🌐".blue());
            println!("    Log level: {}", config.global.log_level);
            println!("    Metrics: {}", if config.global.enable_metrics { "enabled".green() } else { "disabled".red() });
            if let Some(endpoint) = &config.global.metrics_endpoint {
                println!("    Metrics endpoint: {}", endpoint);
            }
            
            Ok(())
        }
        Err(e) => {
            println!("{} Configuration is invalid", "✗".red().bold());
            println!("  Error: {}", e.to_string().red());
            Err(e.into())
        }
    }
}

/// Convert configuration between formats
pub async fn convert_config(input: PathBuf, output: PathBuf) -> Result<()> {
    println!("{} Converting configuration", "→".green().bold());
    println!("  Input: {}", input.display().to_string().cyan());
    println!("  Output: {}", output.display().to_string().cyan());
    
    // Load configuration
    let config = Config::from_file(&input)
        .with_context(|| format!("Failed to load configuration from {}", input.display()))?;
    
    // Determine output format from extension
    let output_format = output.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("yaml");
    
    let content = match output_format {
        "yaml" | "yml" => {
            serde_yaml::to_string(&config)
                .context("Failed to serialize to YAML")?
        }
        "json" => {
            serde_json::to_string_pretty(&config)
                .context("Failed to serialize to JSON")?
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported output format: {}", output_format));
        }
    };
    
    std::fs::write(&output, content)
        .with_context(|| format!("Failed to write to {}", output.display()))?;
    
    println!("{} Configuration converted successfully", "✓".green().bold());
    Ok(())
}

/// Create a comprehensive sample configuration
fn create_sample_config() -> Config {
    Config {
        server: Some(ServerConfig {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 10000,
            connection_timeout_ms: 30000,
            enable_tls: false,
            tls_cert_path: Some("certs/server.crt".to_string()),
            tls_key_path: Some("certs/server.key".to_string()),
            enable_metrics: true,
        }),
        client: Some(ClientConfig {
            endpoint: "tcp://localhost:8080".to_string(),
            connect_timeout_ms: 5000,
            request_timeout_ms: 30000,
            max_connections: 10,
            enable_pooling: true,
            retry_policy: RetryPolicy::exponential_backoff(3, 100, 5000),
            enable_metrics: true,
        }),
        global: GlobalConfig {
            log_level: "info".to_string(),
            enable_metrics: true,
            metrics_endpoint: Some("http://localhost:9090/metrics".to_string()),
        },
    }
}