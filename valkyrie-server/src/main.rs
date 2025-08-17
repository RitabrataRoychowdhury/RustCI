//! Standalone Valkyrie Protocol Server
//! 
//! A high-performance, standalone server implementation of the Valkyrie Protocol.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error};
use valkyrie_sdk::{Config, ServerBuilder};
use std::sync::Arc;

mod handlers;
mod config;
mod signals;

use handlers::{EchoHandler, HealthHandler, MetricsHandler};

/// Valkyrie Protocol Server
#[derive(Parser)]
#[command(name = "valkyrie-server")]
#[command(about = "High-performance Valkyrie Protocol Server")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the server
    Start {
        /// Configuration file path
        #[arg(short, long)]
        config: Option<PathBuf>,
        
        /// Bind address
        #[arg(short, long)]
        bind: Option<String>,
        
        /// Port number
        #[arg(short, long)]
        port: Option<u16>,
        
        /// Enable verbose logging
        #[arg(short, long)]
        verbose: bool,
    },
    /// Generate a sample configuration file
    Config {
        /// Output file path
        #[arg(short, long, default_value = "valkyrie.yaml")]
        output: PathBuf,
        
        /// Configuration format (yaml, toml, json)
        #[arg(short, long, default_value = "yaml")]
        format: String,
    },
    /// Check configuration file
    Check {
        /// Configuration file path
        #[arg(short, long, default_value = "valkyrie.yaml")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Start { config, bind, port, verbose } => {
            start_server(config, bind, port, verbose).await
        }
        Commands::Config { output, format } => {
            generate_config(output, format).await
        }
        Commands::Check { config } => {
            check_config(config).await
        }
    }
}

async fn start_server(
    config_path: Option<PathBuf>,
    bind_override: Option<String>,
    port_override: Option<u16>,
    verbose: bool,
) -> Result<()> {
    // Initialize logging
    let log_level = if verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("valkyrie_server={},valkyrie_sdk={},valkyrie_protocol={}", log_level, log_level, log_level))
        .init();
    
    info!("Starting Valkyrie Protocol Server v{}", env!("CARGO_PKG_VERSION"));
    
    // Load configuration
    let mut config = if let Some(config_path) = config_path {
        info!("Loading configuration from: {}", config_path.display());
        Config::from_file(&config_path)
            .with_context(|| format!("Failed to load configuration from {}", config_path.display()))?
    } else {
        info!("Using default configuration");
        Config::default()
    };
    
    // Apply command line overrides
    if let Some(bind) = bind_override {
        if config.server.is_none() {
            config.server = Some(valkyrie_sdk::ServerConfig::default());
        }
        if let Some(ref mut server_config) = config.server {
            if let Some((host, port)) = bind.rsplit_once(':') {
                server_config.bind_address = host.to_string();
                if let Ok(port_num) = port.parse::<u16>() {
                    server_config.port = port_num;
                }
            } else {
                server_config.bind_address = bind;
            }
        }
    }
    
    if let Some(port) = port_override {
        if config.server.is_none() {
            config.server = Some(valkyrie_sdk::ServerConfig::default());
        }
        if let Some(ref mut server_config) = config.server {
            server_config.port = port;
        }
    }
    
    let server_config = config.server.unwrap_or_default();
    
    info!("Server configuration:");
    info!("  Bind address: {}", server_config.bind_address);
    info!("  Port: {}", server_config.port);
    info!("  Max connections: {}", server_config.max_connections);
    info!("  TLS enabled: {}", server_config.enable_tls);
    
    // Build server with handlers
    let bind_address = server_config.bind_address.clone();
    let port = server_config.port;
    let enable_metrics = server_config.enable_metrics;
    
    let server = ServerBuilder::new()
        .bind_address(server_config.bind_address)
        .port(server_config.port)
        .max_connections(server_config.max_connections)
        .handler("/echo", EchoHandler)
        .handler("/health", HealthHandler)
        .handler("/metrics", MetricsHandler)
        .enable_metrics(enable_metrics)
        .build()
        .await
        .context("Failed to build server")?;
    
    // Set up signal handling
    let server_handle = Arc::new(server);
    let shutdown_server = Arc::clone(&server_handle);
    
    tokio::spawn(async move {
        if let Err(e) = signals::wait_for_shutdown().await {
            error!("Error waiting for shutdown signal: {}", e);
        }
        
        info!("Shutdown signal received, stopping server...");
        if let Err(e) = shutdown_server.stop().await {
            error!("Error stopping server: {}", e);
        }
    });
    
    // Start the server
    info!("Server starting on {}:{}", bind_address, port);
    server_handle.start().await
        .context("Failed to start server")?;
    
    info!("Server stopped gracefully");
    Ok(())
}

async fn generate_config(output: PathBuf, format: String) -> Result<()> {
    info!("Generating sample configuration file: {}", output.display());
    
    let config = config::create_sample_config();
    
    match format.as_str() {
        "yaml" | "yml" => {
            let content = serde_yaml::to_string(&config)
                .context("Failed to serialize configuration to YAML")?;
            std::fs::write(&output, content)
                .with_context(|| format!("Failed to write configuration to {}", output.display()))?;
        }
        "toml" => {
            let content = toml::to_string_pretty(&config)
                .context("Failed to serialize configuration to TOML")?;
            std::fs::write(&output, content)
                .with_context(|| format!("Failed to write configuration to {}", output.display()))?;
        }
        "json" => {
            let content = serde_json::to_string_pretty(&config)
                .context("Failed to serialize configuration to JSON")?;
            std::fs::write(&output, content)
                .with_context(|| format!("Failed to write configuration to {}", output.display()))?;
        }
        _ => {
            return Err(anyhow::anyhow!("Unsupported format: {}. Use yaml, toml, or json", format));
        }
    }
    
    info!("Configuration file generated successfully");
    Ok(())
}

async fn check_config(config_path: PathBuf) -> Result<()> {
    info!("Checking configuration file: {}", config_path.display());
    
    match Config::from_file(&config_path) {
        Ok(config) => {
            info!("Configuration is valid");
            
            if let Some(server_config) = &config.server {
                info!("Server configuration:");
                info!("  Bind: {}:{}", server_config.bind_address, server_config.port);
                info!("  Max connections: {}", server_config.max_connections);
                info!("  TLS: {}", server_config.enable_tls);
            }
            
            if let Some(client_config) = &config.client {
                info!("Client configuration:");
                info!("  Endpoint: {}", client_config.endpoint);
                info!("  Timeout: {}ms", client_config.request_timeout_ms);
                info!("  Pooling: {}", client_config.enable_pooling);
            }
            
            info!("Global configuration:");
            info!("  Log level: {}", config.global.log_level);
            info!("  Metrics: {}", config.global.enable_metrics);
            
            Ok(())
        }
        Err(e) => {
            error!("Configuration is invalid: {}", e);
            Err(e.into())
        }
    }
}