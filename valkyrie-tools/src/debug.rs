//! Debug tools for connection diagnostics

use anyhow::{Context, Result};
use valkyrie_sdk::ClientBuilder;
use colored::*;
use std::time::Instant;

/// Debug server connection
pub async fn debug_connection(endpoint: String, detailed: bool) -> Result<()> {
    println!("{}", "🔍 Valkyrie Connection Debug".cyan().bold());
    println!();
    
    println!("{} Target: {}", "🎯".blue(), endpoint.cyan());
    println!();
    
    // Test basic connectivity
    println!("{} Testing basic connectivity...", "→".yellow());
    let connect_start = Instant::now();
    
    let client = match ClientBuilder::new()
        .endpoint(&endpoint)
        .connect_timeout_ms(5000)
        .request_timeout_ms(10000)
        .enable_pooling(false)
        .build()
        .await
    {
        Ok(client) => {
            let connect_duration = connect_start.elapsed();
            println!("{} Connection established in {:.2}ms", 
                "✓".green().bold(), 
                connect_duration.as_secs_f64() * 1000.0
            );
            client
        }
        Err(e) => {
            println!("{} Connection failed: {}", "✗".red().bold(), e.to_string().red());
            return Err(e.into());
        }
    };
    
    // Test health check
    println!("{} Testing health check...", "→".yellow());
    let health_start = Instant::now();
    
    match client.request_string("/health").await {
        Ok(response) => {
            let health_duration = health_start.elapsed();
            println!("{} Health check successful in {:.2}ms", 
                "✓".green().bold(), 
                health_duration.as_secs_f64() * 1000.0
            );
            
            if detailed {
                println!("  Response: {}", response.trim().green());
            }
        }
        Err(e) => {
            println!("{} Health check failed: {}", "⚠".yellow().bold(), e.to_string().yellow());
        }
    }
    
    // Test echo functionality
    println!("{} Testing echo functionality...", "→".yellow());
    let echo_message = "Hello, Valkyrie Debug!";
    let echo_start = Instant::now();
    
    match client.request_string(echo_message).await {
        Ok(response) => {
            let echo_duration = echo_start.elapsed();
            println!("{} Echo test successful in {:.2}ms", 
                "✓".green().bold(), 
                echo_duration.as_secs_f64() * 1000.0
            );
            
            if detailed {
                println!("  Sent: {}", echo_message.cyan());
                println!("  Received: {}", response.trim().green());
            }
            
            // Verify echo response
            if response.contains(echo_message) {
                println!("  {} Echo response contains original message", "✓".green());
            } else {
                println!("  {} Echo response doesn't match expected format", "⚠".yellow());
            }
        }
        Err(e) => {
            println!("{} Echo test failed: {}", "✗".red().bold(), e.to_string().red());
        }
    }
    
    // Test notification
    println!("{} Testing notification...", "→".yellow());
    let notify_start = Instant::now();
    
    match client.notify_string("Debug notification test").await {
        Ok(()) => {
            let notify_duration = notify_start.elapsed();
            println!("{} Notification sent successfully in {:.2}ms", 
                "✓".green().bold(), 
                notify_duration.as_secs_f64() * 1000.0
            );
        }
        Err(e) => {
            println!("{} Notification failed: {}", "✗".red().bold(), e.to_string().red());
        }
    }
    
    // Performance test
    if detailed {
        println!("{} Running mini performance test...", "→".yellow());
        
        let mut latencies = Vec::new();
        let test_count = 10;
        
        for i in 1..=test_count {
            let start = Instant::now();
            match client.request_string(&format!("Performance test {}", i)).await {
                Ok(_) => {
                    let duration = start.elapsed();
                    latencies.push(duration.as_micros() as f64);
                }
                Err(e) => {
                    println!("  {} Request {} failed: {}", "✗".red(), i, e);
                }
            }
        }
        
        if !latencies.is_empty() {
            latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let min = latencies[0];
            let max = latencies[latencies.len() - 1];
            let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
            let p95_idx = ((latencies.len() as f64) * 0.95) as usize;
            let p95 = latencies[p95_idx.min(latencies.len() - 1)];
            
            println!("  {} Performance results (μs):", "📊".blue());
            println!("    Min: {:.1}", min);
            println!("    Max: {:.1}", max);
            println!("    Avg: {:.1}", avg);
            println!("    P95: {:.1}", p95);
        }
    }
    
    // Connection status
    println!("{} Checking final connection status...", "→".yellow());
    if client.is_connected().await {
        println!("{} Connection is still healthy", "✓".green().bold());
    } else {
        println!("{} Connection appears to be unhealthy", "⚠".yellow().bold());
    }
    
    // Cleanup
    println!("{} Disconnecting...", "→".yellow());
    client.disconnect().await.ok();
    
    println!();
    println!("{} Debug session completed", "🏁".green().bold());
    
    Ok(())
}