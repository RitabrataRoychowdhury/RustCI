//! Registry Integration Example
//!
//! This example demonstrates how to use the enhanced registries with Snapshot/RCU
//! patterns for high-performance, lock-free registry operations.

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use std::collections::HashMap;
use uuid::Uuid;

use crate::valkyrie::routing::{
    EnhancedConnectionRegistry, EnhancedServiceRegistry,
    EnhancedConnectionInfo, EnhancedServiceEntry,
    ConnectionState, ServiceHealthStatus,
    ConnectionMetrics, ServiceMetrics,
};
use crate::valkyrie::routing::enhanced_registries::ServiceEndpoint;

/// Example of setting up and using enhanced registries
pub async fn registry_performance_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up enhanced registries with Snapshot/RCU...");
    
    // Create enhanced registries
    let connection_registry = Arc::new(EnhancedConnectionRegistry::new(Duration::from_secs(300)));
    let service_registry = Arc::new(EnhancedServiceRegistry::new());
    
    // Example 1: Connection Registry Operations
    println!("\n=== Connection Registry Example ===");
    
    // Register multiple connections
    let mut connection_ids = Vec::new();
    for i in 0..1000 {
        let connection = EnhancedConnectionInfo {
            id: Uuid::new_v4(),
            remote_addr: format!("192.168.1.{}:8080", i % 255).parse()?,
            state: ConnectionState::Active,
            last_activity: SystemTime::now(),
            metrics: ConnectionMetrics {
                bytes_sent: (i * 1024) as u64,
                bytes_received: (i * 2048) as u64,
                messages_sent: i as u64,
                messages_received: (i * 2) as u64,
                avg_latency_us: 50 + (i % 100) as u32,
                uptime: Duration::from_secs(i as u64),
                error_count: i % 10,
            },
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("client_type".to_string(), "test".to_string());
                meta.insert("version".to_string(), "1.0.0".to_string());
                meta
            },
        };
        
        connection_ids.push(connection.id);
        connection_registry.register_connection(connection).await?;
        
        if i % 100 == 0 {
            println!("Registered {} connections", i + 1);
        }
    }
    
    println!("Total connections registered: {}", connection_registry.connection_count());
    
    // Perform concurrent lookups to test lock-free performance
    println!("Performing concurrent lookups...");
    let start_time = std::time::Instant::now();
    let mut lookup_tasks = Vec::new();
    
    for chunk in connection_ids.chunks(100) {
        let registry = connection_registry.clone();
        let ids = chunk.to_vec();
        
        let task = tokio::spawn(async move {
            let mut found_count = 0;
            for id in ids {
                if registry.lookup_connection(&id).is_some() {
                    found_count += 1;
                }
            }
            found_count
        });
        
        lookup_tasks.push(task);
    }
    
    let mut total_found = 0;
    for task in lookup_tasks {
        total_found += task.await?;
    }
    
    let lookup_duration = start_time.elapsed();
    println!("Found {} connections in {:?}", total_found, lookup_duration);
    println!("Average lookup time: {:?}", lookup_duration / total_found as u32);
    
    // Get registry statistics
    let conn_stats = connection_registry.get_stats();
    println!("Connection Registry Stats:");
    println!("  Total lookups: {}", conn_stats.total_lookups);
    println!("  Successful lookups: {}", conn_stats.successful_lookups);
    println!("  Hit rate: {:.2}%", (conn_stats.successful_lookups as f64 / conn_stats.total_lookups as f64) * 100.0);
    println!("  Average lookup time: {}ns", conn_stats.avg_lookup_time_ns);
    
    // Example 2: Service Registry Operations
    println!("\n=== Service Registry Example ===");
    
    // Register services with different prefixes and tags
    let service_names = vec![
        "user-service", "auth-service", "payment-service",
        "notification-service", "analytics-service", "user-profile-service",
        "user-management-service", "authentication-gateway", "payment-processor",
    ];
    
    let mut service_ids = Vec::new();
    for (i, name) in service_names.iter().enumerate() {
        let service = EnhancedServiceEntry {
            id: Uuid::new_v4(),
            name: name.to_string(),
            version: format!("1.{}.0", i),
            endpoints: vec![
                ServiceEndpoint {
                    address: format!("10.0.0.{}:8080", i + 1).parse()?,
                    protocol: "http".to_string(),
                    weight: 100,
                    healthy: true,
                },
                ServiceEndpoint {
                    address: format!("10.0.0.{}:8443", i + 1).parse()?,
                    protocol: "https".to_string(),
                    weight: 100,
                    healthy: true,
                },
            ],
            tags: {
                let mut tags = HashMap::new();
                tags.insert("env".to_string(), "production".to_string());
                tags.insert("team".to_string(), match i % 3 {
                    0 => "backend",
                    1 => "platform",
                    _ => "security",
                }.to_string());
                tags.insert("tier".to_string(), if name.contains("user") { "critical" } else { "standard" }.to_string());
                tags
            },
            health_status: ServiceHealthStatus::Healthy,
            metrics: ServiceMetrics {
                request_rate: (i as f64 + 1.0) * 100.0,
                avg_response_time_ms: 50.0 + (i as f64 * 10.0),
                error_rate: 0.001 + (i as f64 * 0.0001),
                cpu_usage: 20.0 + (i as f64 * 5.0),
                memory_usage: 30.0 + (i as f64 * 3.0),
                active_connections: (i as u32 + 1) * 10,
            },
            registered_at: SystemTime::now(),
            last_health_check: SystemTime::now(),
        };
        
        service_ids.push(service.id);
        service_registry.register_service(service).await?;
    }
    
    println!("Registered {} services", service_registry.service_count());
    
    // Test prefix-based service discovery
    println!("\nTesting prefix-based service discovery:");
    
    let prefixes = vec!["user", "auth", "payment"];
    for prefix in prefixes {
        let services = service_registry.find_services_by_prefix(prefix);
        println!("  Services with prefix '{}': {}", prefix, services.len());
        for service in services {
            println!("    - {} ({})", service.name, service.version);
        }
    }
    
    // Test tag-based service discovery
    println!("\nTesting tag-based service discovery:");
    
    let tag_queries = vec![
        ("env", "production"),
        ("team", "backend"),
        ("tier", "critical"),
    ];
    
    for (key, value) in tag_queries {
        let services = service_registry.find_services_by_tag(key, value);
        println!("  Services with tag '{}:{}': {}", key, value, services.len());
        for service in services {
            println!("    - {} (team: {})", service.name, 
                     service.tags.get("team").unwrap_or(&"unknown".to_string()));
        }
    }
    
    // Test concurrent service lookups
    println!("\nPerforming concurrent service lookups...");
    let start_time = std::time::Instant::now();
    let mut service_lookup_tasks = Vec::new();
    
    for chunk in service_ids.chunks(3) {
        let registry = service_registry.clone();
        let ids = chunk.to_vec();
        
        let task = tokio::spawn(async move {
            let mut found_count = 0;
            for id in ids {
                if registry.lookup_service(&id).is_some() {
                    found_count += 1;
                }
            }
            found_count
        });
        
        service_lookup_tasks.push(task);
    }
    
    let mut total_service_found = 0;
    for task in service_lookup_tasks {
        total_service_found += task.await?;
    }
    
    let service_lookup_duration = start_time.elapsed();
    println!("Found {} services in {:?}", total_service_found, service_lookup_duration);
    
    // Get service registry statistics
    let service_stats = service_registry.get_stats();
    println!("Service Registry Stats:");
    println!("  Total lookups: {}", service_stats.total_lookups);
    println!("  Successful lookups: {}", service_stats.successful_lookups);
    println!("  Hit rate: {:.2}%", (service_stats.successful_lookups as f64 / service_stats.total_lookups as f64) * 100.0);
    println!("  Average lookup time: {}ns", service_stats.avg_lookup_time_ns);
    
    // Example 3: Health Status Updates
    println!("\n=== Health Status Updates Example ===");
    
    // Simulate health status changes
    for (i, &service_id) in service_ids.iter().enumerate() {
        let new_status = match i % 4 {
            0 => ServiceHealthStatus::Healthy,
            1 => ServiceHealthStatus::Degraded,
            2 => ServiceHealthStatus::Unhealthy,
            _ => ServiceHealthStatus::Unknown,
        };
        
        service_registry.update_service_health(&service_id, new_status).await?;
    }
    
    // Get healthy services
    let healthy_services = service_registry.get_healthy_services();
    println!("Healthy services: {}", healthy_services.len());
    
    // Example 4: Connection Metrics Updates
    println!("\n=== Connection Metrics Updates Example ===");
    
    // Update connection metrics for some connections
    for (i, &connection_id) in connection_ids.iter().take(100).enumerate() {
        let updated_metrics = ConnectionMetrics {
            bytes_sent: (i * 2048) as u64,
            bytes_received: (i * 4096) as u64,
            messages_sent: (i * 2) as u64,
            messages_received: (i * 4) as u64,
            avg_latency_us: 30 + (i % 50) as u32,
            uptime: Duration::from_secs((i * 60) as u64),
            error_count: (i % 5) as u32,
        };
        
        connection_registry.update_connection_metrics(&connection_id, updated_metrics).await?;
    }
    
    println!("Updated metrics for 100 connections");
    
    // Final statistics
    println!("\n=== Final Statistics ===");
    let final_conn_stats = connection_registry.get_stats();
    let final_service_stats = service_registry.get_stats();
    
    println!("Connection Registry:");
    println!("  Total connections: {}", connection_registry.connection_count());
    println!("  Total operations: {}", final_conn_stats.total_insertions + final_conn_stats.total_removals + final_conn_stats.total_lookups);
    
    println!("Service Registry:");
    println!("  Total services: {}", service_registry.service_count());
    println!("  Total operations: {}", final_service_stats.total_insertions + final_service_stats.total_removals + final_service_stats.total_lookups);
    
    Ok(())
}

/// Benchmark registry performance under load
pub async fn registry_performance_benchmark() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running registry performance benchmark...");
    
    let connection_registry = Arc::new(EnhancedConnectionRegistry::new(Duration::from_secs(300)));
    let service_registry = Arc::new(EnhancedServiceRegistry::new());
    
    // Benchmark connection registry
    println!("\n=== Connection Registry Benchmark ===");
    
    let num_connections = 10_000;
    let num_lookups = 100_000;
    
    // Register connections
    let start_time = std::time::Instant::now();
    let mut connection_ids = Vec::new();
    
    for i in 0..num_connections {
        let connection = EnhancedConnectionInfo {
            id: Uuid::new_v4(),
            remote_addr: format!("10.0.{}.{}:8080", i / 255, i % 255).parse()?,
            state: ConnectionState::Active,
            last_activity: SystemTime::now(),
            metrics: ConnectionMetrics::default(),
            metadata: HashMap::new(),
        };
        
        connection_ids.push(connection.id);
        connection_registry.register_connection(connection).await?;
    }
    
    let registration_time = start_time.elapsed();
    println!("Registered {} connections in {:?}", num_connections, registration_time);
    println!("Registration rate: {:.0} ops/sec", num_connections as f64 / registration_time.as_secs_f64());
    
    // Benchmark lookups
    let start_time = std::time::Instant::now();
    let mut successful_lookups = 0;
    
    for _ in 0..num_lookups {
        let random_id = &connection_ids[fastrand::usize(0..connection_ids.len())];
        if connection_registry.lookup_connection(random_id).is_some() {
            successful_lookups += 1;
        }
    }
    
    let lookup_time = start_time.elapsed();
    println!("Performed {} lookups in {:?}", num_lookups, lookup_time);
    println!("Lookup rate: {:.0} ops/sec", num_lookups as f64 / lookup_time.as_secs_f64());
    println!("Average lookup time: {:?}", lookup_time / num_lookups);
    println!("Hit rate: {:.2}%", (successful_lookups as f64 / num_lookups as f64) * 100.0);
    
    // Benchmark service registry
    println!("\n=== Service Registry Benchmark ===");
    
    let num_services = 1_000;
    let num_service_lookups = 10_000;
    
    // Register services
    let start_time = std::time::Instant::now();
    let mut service_ids = Vec::new();
    
    for i in 0..num_services {
        let service = EnhancedServiceEntry {
            id: Uuid::new_v4(),
            name: format!("service-{}", i),
            version: "1.0.0".to_string(),
            endpoints: vec![],
            tags: HashMap::new(),
            health_status: ServiceHealthStatus::Healthy,
            metrics: ServiceMetrics::default(),
            registered_at: SystemTime::now(),
            last_health_check: SystemTime::now(),
        };
        
        service_ids.push(service.id);
        service_registry.register_service(service).await?;
    }
    
    let service_registration_time = start_time.elapsed();
    println!("Registered {} services in {:?}", num_services, service_registration_time);
    println!("Service registration rate: {:.0} ops/sec", num_services as f64 / service_registration_time.as_secs_f64());
    
    // Benchmark service lookups
    let start_time = std::time::Instant::now();
    let mut successful_service_lookups = 0;
    
    for _ in 0..num_service_lookups {
        let random_id = &service_ids[fastrand::usize(0..service_ids.len())];
        if service_registry.lookup_service(random_id).is_some() {
            successful_service_lookups += 1;
        }
    }
    
    let service_lookup_time = start_time.elapsed();
    println!("Performed {} service lookups in {:?}", num_service_lookups, service_lookup_time);
    println!("Service lookup rate: {:.0} ops/sec", num_service_lookups as f64 / service_lookup_time.as_secs_f64());
    println!("Average service lookup time: {:?}", service_lookup_time / num_service_lookups);
    println!("Service hit rate: {:.2}%", (successful_service_lookups as f64 / num_service_lookups as f64) * 100.0);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_registry_integration() {
        let result = registry_performance_example().await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_registry_benchmark() {
        let result = registry_performance_benchmark().await;
        assert!(result.is_ok());
    }
}