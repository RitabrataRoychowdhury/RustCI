//! Valkyrie Protocol Command-Line Tools
//! 
//! A comprehensive toolkit for working with Valkyrie Protocol servers and clients.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error};

mod client;
mod benchmark;
mod debug;
mod config;

/// Valkyrie Protocol Tools
#[derive(Parser)]
#[command(name = "valkyrie")]
#[command(about = "Command-line tools for Valkyrie Protocol")]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Send messages to a Valkyrie server
    Send {
        /// Server endpoint
        #[arg(short, long, default_value = "tcp://localhost:8080")]
        endpoint: String,
        
        /// Message to send
        #[arg(short, long)]
        message: Option<String>,
        
        /// Read message from file
        #[arg(short, long)]
        file: Option<PathBuf>,
        
        /// Message type (request, notification)
        #[arg(short = 't', long, default_value = "request")]
        message_type: String,
        
        /// Request timeout in milliseconds
        #[arg(long, default_value = "5000")]
        timeout: u64,
    },
    
    /// Interactive client mode
    Interactive {
        /// Server endpoint
        #[arg(short, long, default_value = "tcp://localhost:8080")]
        endpoint: String,
    },
    
    /// Benchmark server performance
    Benchmark {
        /// Server endpoint
        #[arg(short, long, default_value = "tcp://localhost:8080")]
        endpoint: String,
        
        /// Number of concurrent connections
        #[arg(short, long, default_value = "10")]
        connections: usize,
        
        /// Number of requests per connection
        #[arg(short, long, default_value = "1000")]
        requests: usize,
        
        /// Message size in bytes
        #[arg(short = 's', long, default_value = "1024")]
        message_size: usize,
        
        /// Duration in seconds (0 = use request count)
        #[arg(short, long, default_value = "0")]
        duration: u64,
    },
    
    /// Debug server connection
    Debug {
        /// Server endpoint
        #[arg(short, long, default_value = "tcp://localhost:8080")]
        endpoint: String,
        
        /// Show detailed connection info
        #[arg(long)]
        detailed: bool,
    },
    
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Generate sample configuration
    Generate {
        /// Output file
        #[arg(short, long, default_value = "valkyrie.yaml")]
        output: PathBuf,
        
        /// Format (yaml, toml, json)
        #[arg(short, long, default_value = "yaml")]
        format: String,
    },
    
    /// Validate configuration
    Validate {
        /// Configuration file
        #[arg(short, long)]
        config: PathBuf,
    },
    
    /// Convert configuration format
    Convert {
        /// Input file
        #[arg(short, long)]
        input: PathBuf,
        
        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("valkyrie_tools={}", log_level))
        .init();
    
    match cli.command {
        Commands::Send { endpoint, message, file, message_type, timeout } => {
            client::send_message(endpoint, message, file, message_type, timeout).await
        }
        Commands::Interactive { endpoint } => {
            client::interactive_mode(endpoint).await
        }
        Commands::Benchmark { endpoint, connections, requests, message_size, duration } => {
            benchmark::run_benchmark(endpoint, connections, requests, message_size, duration).await
        }
        Commands::Debug { endpoint, detailed } => {
            debug::debug_connection(endpoint, detailed).await
        }
        Commands::Config { command } => {
            match command {
                ConfigCommands::Generate { output, format } => {
                    config::generate_config(output, format).await
                }
                ConfigCommands::Validate { config } => {
                    config::validate_config(config).await
                }
                ConfigCommands::Convert { input, output } => {
                    config::convert_config(input, output).await
                }
            }
        }
    }
}