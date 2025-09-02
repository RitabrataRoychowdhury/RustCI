use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::deployment::blue_green_manager::{TrafficRouter, DeploymentEnvironment};
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficRoutingConfig {
    pub blue_upstream: String,
    pub green_upstream: String,
    pub health_check_interval: std::time::Duration,
    pub failover_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSplit {
    pub blue_percentage: f64,
    pub green_percentage: f64,
}

impl TrafficSplit {
    pub fn new(blue_percentage: f64, green_percentage: f64) -> Result<Self> {
        if (blue_percentage + green_percentage - 100.0).abs() > f64::EPSILON {
            return Err(AppError::ValidationFailed(
                "Traffic split percentages must sum to 100".to_string()
            ));
        }
        
        Ok(Self {
            blue_percentage,
            green_percentage,
        })
    }

    pub fn all_blue() -> Self {
        Self {
            blue_percentage: 100.0,
            green_percentage: 0.0,
        }
    }

    pub fn all_green() -> Self {
        Self {
            blue_percentage: 0.0,
            green_percentage: 100.0,
        }
    }
}

pub struct ProductionTrafficRouter {
    config: TrafficRoutingConfig,
    current_split: Arc<RwLock<TrafficSplit>>,
    load_balancer: Arc<dyn LoadBalancerAdapter>,
}

impl ProductionTrafficRouter {
    pub fn new(
        config: TrafficRoutingConfig,
        load_balancer: Arc<dyn LoadBalancerAdapter>,
    ) -> Self {
        Self {
            config,
            current_split: Arc::new(RwLock::new(TrafficSplit::all_blue())),
            load_balancer,
        }
    }

    pub async fn gradual_traffic_switch(
        &self,
        target_environment: &DeploymentEnvironment,
        steps: u32,
        step_duration: std::time::Duration,
    ) -> Result<()> {
        let step_percentage = 100.0 / steps as f64;
        
        for step in 1..=steps {
            let percentage = step_percentage * step as f64;
            
            let new_split = match target_environment {
                DeploymentEnvironment::Blue => TrafficSplit::new(percentage, 100.0 - percentage)?,
                DeploymentEnvironment::Green => TrafficSplit::new(100.0 - percentage, percentage)?,
            };

            self.apply_traffic_split(&new_split).await?;
            
            log::info!("Traffic switch step {}/{}: {}% to {:?}", 
                      step, steps, percentage, target_environment);
            
            if step < steps {
                tokio::time::sleep(step_duration).await;
            }
        }

        Ok(())
    }

    async fn apply_traffic_split(&self, split: &TrafficSplit) -> Result<()> {
        // Update load balancer configuration
        self.load_balancer.update_traffic_split(split).await?;
        
        // Update internal state
        {
            let mut current_split = self.current_split.write().await;
            *current_split = split.clone();
        }

        log::info!("Applied traffic split: {}% blue, {}% green", 
                  split.blue_percentage, split.green_percentage);
        
        Ok(())
    }

    pub async fn get_traffic_split(&self) -> TrafficSplit {
        let split = self.current_split.read().await;
        split.clone()
    }

    pub async fn emergency_failover(&self, target_environment: &DeploymentEnvironment) -> Result<()> {
        log::warn!("Emergency failover to {:?} environment", target_environment);
        
        let emergency_split = match target_environment {
            DeploymentEnvironment::Blue => TrafficSplit::all_blue(),
            DeploymentEnvironment::Green => TrafficSplit::all_green(),
        };

        self.apply_traffic_split(&emergency_split).await?;
        
        log::info!("Emergency failover completed");
        Ok(())
    }
}

#[async_trait::async_trait]
impl TrafficRouter for ProductionTrafficRouter {
    async fn switch_traffic(&self, environment: &DeploymentEnvironment) -> Result<()> {
        let target_split = match environment {
            DeploymentEnvironment::Blue => TrafficSplit::all_blue(),
            DeploymentEnvironment::Green => TrafficSplit::all_green(),
        };

        self.apply_traffic_split(&target_split).await
    }

    async fn get_current_traffic_split(&self) -> Result<HashMap<DeploymentEnvironment, f64>> {
        let split = self.get_traffic_split().await;
        
        let mut result = HashMap::new();
        result.insert(DeploymentEnvironment::Blue, split.blue_percentage);
        result.insert(DeploymentEnvironment::Green, split.green_percentage);
        
        Ok(result)
    }
}

#[async_trait::async_trait]
pub trait LoadBalancerAdapter: Send + Sync {
    async fn update_traffic_split(&self, split: &TrafficSplit) -> Result<()>;
    async fn get_upstream_health(&self, upstream: &str) -> Result<bool>;
    async fn enable_upstream(&self, upstream: &str) -> Result<()>;
    async fn disable_upstream(&self, upstream: &str) -> Result<()>;
}

pub struct HAProxyAdapter {
    admin_socket_path: String,
}

impl HAProxyAdapter {
    pub fn new(admin_socket_path: String) -> Self {
        Self { admin_socket_path }
    }

    async fn send_haproxy_command(&self, command: &str) -> Result<String> {
        use tokio::net::UnixStream;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut stream = UnixStream::connect(&self.admin_socket_path).await
            .map_err(|e| AppError::NetworkError(format!("Failed to connect to HAProxy admin socket: {}", e)))?;

        stream.write_all(command.as_bytes()).await
            .map_err(|e| AppError::NetworkError(format!("Failed to send command to HAProxy: {}", e)))?;

        let mut response = String::new();
        stream.read_to_string(&mut response).await
            .map_err(|e| AppError::NetworkError(format!("Failed to read response from HAProxy: {}", e)))?;

        Ok(response)
    }
}

#[async_trait::async_trait]
impl LoadBalancerAdapter for HAProxyAdapter {
    async fn update_traffic_split(&self, split: &TrafficSplit) -> Result<()> {
        // Set weights for blue and green upstreams
        let blue_weight = (split.blue_percentage as u32).max(1);
        let green_weight = (split.green_percentage as u32).max(1);

        let blue_command = format!("set weight backend/blue {}\n", blue_weight);
        let green_command = format!("set weight backend/green {}\n", green_weight);

        self.send_haproxy_command(&blue_command).await?;
        self.send_haproxy_command(&green_command).await?;

        Ok(())
    }

    async fn get_upstream_health(&self, upstream: &str) -> Result<bool> {
        let command = format!("show stat\n");
        let response = self.send_haproxy_command(&command).await?;
        
        // Parse HAProxy stats to check upstream health
        // This is a simplified implementation
        Ok(response.contains(&format!("{},UP", upstream)))
    }

    async fn enable_upstream(&self, upstream: &str) -> Result<()> {
        let command = format!("enable server backend/{}\n", upstream);
        self.send_haproxy_command(&command).await?;
        Ok(())
    }

    async fn disable_upstream(&self, upstream: &str) -> Result<()> {
        let command = format!("disable server backend/{}\n", upstream);
        self.send_haproxy_command(&command).await?;
        Ok(())
    }
}

// Mock adapter for testing
pub struct MockLoadBalancerAdapter {
    traffic_split: Arc<RwLock<TrafficSplit>>,
}

impl MockLoadBalancerAdapter {
    pub fn new() -> Self {
        Self {
            traffic_split: Arc::new(RwLock::new(TrafficSplit::all_blue())),
        }
    }
}

#[async_trait::async_trait]
impl LoadBalancerAdapter for MockLoadBalancerAdapter {
    async fn update_traffic_split(&self, split: &TrafficSplit) -> Result<()> {
        let mut current_split = self.traffic_split.write().await;
        *current_split = split.clone();
        Ok(())
    }

    async fn get_upstream_health(&self, _upstream: &str) -> Result<bool> {
        Ok(true)
    }

    async fn enable_upstream(&self, _upstream: &str) -> Result<()> {
        Ok(())
    }

    async fn disable_upstream(&self, _upstream: &str) -> Result<()> {
        Ok(())
    }
}

impl Default for MockLoadBalancerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_traffic_split_validation() {
        let valid_split = TrafficSplit::new(70.0, 30.0);
        assert!(valid_split.is_ok());

        let invalid_split = TrafficSplit::new(70.0, 40.0);
        assert!(invalid_split.is_err());
    }

    #[tokio::test]
    async fn test_traffic_router_switch() {
        let config = TrafficRoutingConfig {
            blue_upstream: "blue:8080".to_string(),
            green_upstream: "green:8080".to_string(),
            health_check_interval: std::time::Duration::from_secs(30),
            failover_threshold: 3,
        };

        let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
        let router = ProductionTrafficRouter::new(config, load_balancer);

        // Test switching to green
        let result = router.switch_traffic(&DeploymentEnvironment::Green).await;
        assert!(result.is_ok());

        let split = router.get_current_traffic_split().await.unwrap();
        assert_eq!(split.get(&DeploymentEnvironment::Green), Some(&100.0));
        assert_eq!(split.get(&DeploymentEnvironment::Blue), Some(&0.0));
    }

    #[tokio::test]
    async fn test_gradual_traffic_switch() {
        let config = TrafficRoutingConfig {
            blue_upstream: "blue:8080".to_string(),
            green_upstream: "green:8080".to_string(),
            health_check_interval: std::time::Duration::from_secs(30),
            failover_threshold: 3,
        };

        let load_balancer = Arc::new(MockLoadBalancerAdapter::new());
        let router = ProductionTrafficRouter::new(config, load_balancer);

        // Test gradual switch to green
        let result = router.gradual_traffic_switch(
            &DeploymentEnvironment::Green,
            4,
            std::time::Duration::from_millis(100),
        ).await;
        
        assert!(result.is_ok());

        let split = router.get_current_traffic_split().await.unwrap();
        assert_eq!(split.get(&DeploymentEnvironment::Green), Some(&100.0));
    }
}