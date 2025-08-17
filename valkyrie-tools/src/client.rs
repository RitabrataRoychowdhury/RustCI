//! Client commands for sending messages and interactive mode

use anyhow::{Context, Result};
use std::path::PathBuf;
use valkyrie_sdk::{ClientBuilder, MessageType};
use tracing::{info, error};
use colored::*;
use dialoguer::{Input, Select, Confirm};

/// Send a single message to the server
pub async fn send_message(
    endpoint: String,
    message: Option<String>,
    file: Option<PathBuf>,
    message_type: String,
    timeout: u64,
) -> Result<()> {
    info!("Connecting to {}", endpoint);
    
    let client = ClientBuilder::new()
        .endpoint(&endpoint)
        .request_timeout_ms(timeout)
        .enable_pooling(false)
        .build()
        .await
        .context("Failed to create client")?;
    
    // Get message content
    let content = if let Some(msg) = message {
        msg
    } else if let Some(file_path) = file {
        std::fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?
    } else {
        return Err(anyhow::anyhow!("Either --message or --file must be provided"));
    };
    
    println!("{} Sending message to {}", "→".green().bold(), endpoint.cyan());
    println!("{} Message type: {}", "ℹ".blue(), message_type.yellow());
    println!("{} Content: {}", "📝".blue(), content.trim());
    
    match message_type.as_str() {
        "request" => {
            let start = std::time::Instant::now();
            match client.request_string(&content).await {
                Ok(response) => {
                    let duration = start.elapsed();
                    println!("{} Response received in {:.2}ms", "✓".green().bold(), duration.as_secs_f64() * 1000.0);
                    println!("{} Response: {}", "📨".green(), response.trim());
                }
                Err(e) => {
                    error!("Request failed: {}", e);
                    println!("{} Request failed: {}", "✗".red().bold(), e.to_string().red());
                    return Err(e.into());
                }
            }
        }
        "notification" => {
            let start = std::time::Instant::now();
            match client.notify_string(&content).await {
                Ok(()) => {
                    let duration = start.elapsed();
                    println!("{} Notification sent in {:.2}ms", "✓".green().bold(), duration.as_secs_f64() * 1000.0);
                }
                Err(e) => {
                    error!("Notification failed: {}", e);
                    println!("{} Notification failed: {}", "✗".red().bold(), e.to_string().red());
                    return Err(e.into());
                }
            }
        }
        _ => {
            return Err(anyhow::anyhow!("Invalid message type: {}. Use 'request' or 'notification'", message_type));
        }
    }
    
    client.disconnect().await.ok();
    Ok(())
}

/// Interactive client mode
pub async fn interactive_mode(endpoint: String) -> Result<()> {
    println!("{}", "🚀 Valkyrie Interactive Client".cyan().bold());
    println!("{} Connecting to {}", "→".green(), endpoint.cyan());
    
    let client = ClientBuilder::new()
        .endpoint(&endpoint)
        .request_timeout_ms(30000)
        .enable_pooling(true)
        .max_connections(5)
        .build()
        .await
        .context("Failed to create client")?;
    
    if !client.is_connected().await {
        println!("{} Failed to connect to server", "✗".red().bold());
        return Err(anyhow::anyhow!("Connection failed"));
    }
    
    println!("{} Connected successfully!", "✓".green().bold());
    println!();
    
    loop {
        // Show menu
        let options = vec![
            "Send Request",
            "Send Notification", 
            "Check Connection",
            "Exit"
        ];
        
        let selection = Select::new()
            .with_prompt("What would you like to do?")
            .items(&options)
            .default(0)
            .interact()?;
        
        match selection {
            0 => {
                // Send request
                let message: String = Input::new()
                    .with_prompt("Enter message")
                    .interact_text()?;
                
                if message.trim().is_empty() {
                    continue;
                }
                
                println!("{} Sending request...", "→".yellow());
                let start = std::time::Instant::now();
                
                match client.request_string(&message).await {
                    Ok(response) => {
                        let duration = start.elapsed();
                        println!("{} Response received in {:.2}ms", "✓".green().bold(), duration.as_secs_f64() * 1000.0);
                        println!("{} {}", "📨".green(), response.trim());
                    }
                    Err(e) => {
                        println!("{} Request failed: {}", "✗".red().bold(), e.to_string().red());
                    }
                }
            }
            1 => {
                // Send notification
                let message: String = Input::new()
                    .with_prompt("Enter notification message")
                    .interact_text()?;
                
                if message.trim().is_empty() {
                    continue;
                }
                
                println!("{} Sending notification...", "→".yellow());
                let start = std::time::Instant::now();
                
                match client.notify_string(&message).await {
                    Ok(()) => {
                        let duration = start.elapsed();
                        println!("{} Notification sent in {:.2}ms", "✓".green().bold(), duration.as_secs_f64() * 1000.0);
                    }
                    Err(e) => {
                        println!("{} Notification failed: {}", "✗".red().bold(), e.to_string().red());
                    }
                }
            }
            2 => {
                // Check connection
                let connected = client.is_connected().await;
                if connected {
                    println!("{} Connection is healthy", "✓".green().bold());
                } else {
                    println!("{} Connection is not healthy", "⚠".yellow().bold());
                }
            }
            3 => {
                // Exit
                if Confirm::new()
                    .with_prompt("Are you sure you want to exit?")
                    .default(false)
                    .interact()? 
                {
                    break;
                }
            }
            _ => unreachable!(),
        }
        
        println!();
    }
    
    println!("{} Disconnecting...", "→".yellow());
    client.disconnect().await.ok();
    println!("{} Goodbye!", "👋".green());
    
    Ok(())
}