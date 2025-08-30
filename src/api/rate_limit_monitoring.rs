//! Rate Limiting Monitoring and Alerting System
//!
//! This module provides comprehensive monitoring, alerting, and analytics
//! for the rate limiting system.

use crate::api::rate_limiting::{RateLimitResult, RateLimitStats};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Rate limit monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitMonitoringConfig {
    /// Enable monitoring
    pub enabled: bool,
    /// Metrics collection interval in seconds
    pub collection_interval_seconds: u64,
    /// Alert thresholds
    pub alert_thresholds: AlertThresholds,
    /// Retention period for metrics in seconds
    pub metrics_retention_seconds: u64,
    /// Enable detailed per-endpoint metrics
    pub detailed_metrics: bool,
}

impl Default for RateLimitMonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval_seconds: 60,
            alert_thresholds: AlertThresholds::default(),
            metrics_retention_seconds: 86400 * 7, // 7 days
            detailed_metrics: true,
        }
    }
}

/// Alert threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertThresholds {
    /// Rate limit hit rate threshold (0.0 to 1.0)
    pub hit_rate_threshold: f64,
    /// Minimum requests per minute to trigger alerts
    pub min_requests_per_minute: u64,
    /// Consecutive periods above threshold to trigger alert
    pub consecutive_periods: u32,
    /// Alert cooldown period in seconds
    pub cooldown_seconds: u64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            hit_rate_threshold: 0.8, // 80% hit rate
            min_requests_per_minute: 100,
            consecutive_periods: 3,
            cooldown_seconds: 300, // 5 minutes
        }
    }
}

/// Rate limit metrics for a specific time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitMetrics {
    /// Timestamp of the metrics period
    pub timestamp: u64,
    /// Total requests in this period
    pub total_requests: u64,
    /// Requests that were rate limited
    pub rate_limited_requests: u64,
    /// Rate limit hit rate (0.0 to 1.0)
    pub hit_rate: f64,
    /// Average response time for rate limit checks (microseconds)
    pub avg_check_time_us: u64,
    /// Per-endpoint metrics
    pub endpoint_metrics: HashMap<String, EndpointMetrics>,
    /// Per-IP metrics
    pub ip_metrics: HashMap<String, IpMetrics>,
    /// Top rate limited endpoints
    pub top_limited_endpoints: Vec<TopEndpoint>,
    /// Top rate limited IPs
    pub top_limited_ips: Vec<TopIp>,
}

/// Metrics for a specific endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointMetrics {
    /// Endpoint path
    pub endpoint: String,
    /// HTTP method
    pub method: String,
    /// Total requests
    pub total_requests: u64,
    /// Rate limited requests
    pub rate_limited_requests: u64,
    /// Hit rate
    pub hit_rate: f64,
    /// Average check time
    pub avg_check_time_us: u64,
}

/// Metrics for a specific IP address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpMetrics {
    /// IP address
    pub ip: String,
    /// Total requests
    pub total_requests: u64,
    /// Rate limited requests
    pub rate_limited_requests: u64,
    /// Hit rate
    pub hit_rate: f64,
    /// Unique endpoints accessed
    pub unique_endpoints: u32,
}

/// Top rate limited endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopEndpoint {
    /// Endpoint identifier
    pub endpoint: String,
    /// Rate limited requests
    pub rate_limited_requests: u64,
    /// Hit rate
    pub hit_rate: f64,
}

/// Top rate limited IP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopIp {
    /// IP address
    pub ip: String,
    /// Rate limited requests
    pub rate_limited_requests: u64,
    /// Hit rate
    pub hit_rate: f64,
}

/// Rate limit alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitAlert {
    /// Alert ID
    pub alert_id: String,
    /// Alert type
    pub alert_type: AlertType,
    /// Alert severity
    pub severity: AlertSeverity,
    /// Alert message
    pub message: String,
    /// Alert details
    pub details: serde_json::Value,
    /// Timestamp when alert was triggered
    pub triggered_at: u64,
    /// Whether alert is still active
    pub active: bool,
}

/// Alert type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    /// High rate limit hit rate
    HighHitRate,
    /// Suspicious IP activity
    SuspiciousIp,
    /// Endpoint under attack
    EndpointAttack,
    /// Rate limiter performance issue
    PerformanceIssue,
    /// Configuration issue
    ConfigurationIssue,
}

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Rate limit monitoring service
#[derive(Debug)]
pub struct RateLimitMonitor {
    config: RateLimitMonitoringConfig,
    metrics_history: Arc<RwLock<Vec<RateLimitMetrics>>>,
    current_period: Arc<RwLock<CurrentPeriodData>>,
    active_alerts: Arc<RwLock<HashMap<String, RateLimitAlert>>>,
    alert_history: Arc<RwLock<Vec<RateLimitAlert>>>,
}

/// Current period data for metrics collection
#[derive(Debug, Default)]
struct CurrentPeriodData {
    period_start: u64,
    total_requests: u64,
    rate_limited_requests: u64,
    check_times: Vec<u64>,
    endpoint_data: HashMap<String, EndpointPeriodData>,
    ip_data: HashMap<String, IpPeriodData>,
}

#[derive(Debug, Default)]
struct EndpointPeriodData {
    total_requests: u64,
    rate_limited_requests: u64,
    check_times: Vec<u64>,
}

#[derive(Debug, Default)]
struct IpPeriodData {
    total_requests: u64,
    rate_limited_requests: u64,
    endpoints: std::collections::HashSet<String>,
}

impl RateLimitMonitor {
    /// Create new rate limit monitor
    pub fn new(config: RateLimitMonitoringConfig) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            config,
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            current_period: Arc::new(RwLock::new(CurrentPeriodData {
                period_start: now,
                ..Default::default()
            })),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
            alert_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a rate limit check
    pub async fn record_check(
        &self,
        endpoint: String,
        method: String,
        ip: String,
        result: &RateLimitResult,
        check_time_us: u64,
    ) {
        if !self.config.enabled {
            return;
        }

        let mut current = self.current_period.write().await;
        
        // Check if we need to start a new period
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        if now >= current.period_start + self.config.collection_interval_seconds {
            // Finalize current period and start new one
            let metrics = self.finalize_period(&current).await;
            self.process_metrics(&metrics).await;
            
            // Reset for new period
            *current = CurrentPeriodData {
                period_start: now,
                ..Default::default()
            };
        }

        // Record the check
        current.total_requests += 1;
        current.check_times.push(check_time_us);

        let is_rate_limited = matches!(result, RateLimitResult::Exceeded { .. });
        if is_rate_limited {
            current.rate_limited_requests += 1;
        }

        // Record endpoint data
        if self.config.detailed_metrics {
            let endpoint_key = format!("{}:{}", method, endpoint);
            let endpoint_data = current.endpoint_data.entry(endpoint_key).or_default();
            endpoint_data.total_requests += 1;
            endpoint_data.check_times.push(check_time_us);
            if is_rate_limited {
                endpoint_data.rate_limited_requests += 1;
            }

            // Record IP data
            let ip_data = current.ip_data.entry(ip.clone()).or_default();
            ip_data.total_requests += 1;
            ip_data.endpoints.insert(endpoint.clone());
            if is_rate_limited {
                ip_data.rate_limited_requests += 1;
            }
        }

        debug!(
            endpoint = %endpoint,
            method = %method,
            ip = %ip,
            rate_limited = is_rate_limited,
            check_time_us = check_time_us,
            "Recorded rate limit check"
        );
    }

    /// Finalize current period and create metrics
    async fn finalize_period(&self, current: &CurrentPeriodData) -> RateLimitMetrics {
        let hit_rate = if current.total_requests > 0 {
            current.rate_limited_requests as f64 / current.total_requests as f64
        } else {
            0.0
        };

        let avg_check_time_us = if !current.check_times.is_empty() {
            current.check_times.iter().sum::<u64>() / current.check_times.len() as u64
        } else {
            0
        };

        // Build endpoint metrics
        let mut endpoint_metrics = HashMap::new();
        for (endpoint_key, data) in &current.endpoint_data {
            let parts: Vec<&str> = endpoint_key.splitn(2, ':').collect();
            let (method, endpoint) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                ("UNKNOWN".to_string(), endpoint_key.clone())
            };

            let endpoint_hit_rate = if data.total_requests > 0 {
                data.rate_limited_requests as f64 / data.total_requests as f64
            } else {
                0.0
            };

            let endpoint_avg_time = if !data.check_times.is_empty() {
                data.check_times.iter().sum::<u64>() / data.check_times.len() as u64
            } else {
                0
            };

            endpoint_metrics.insert(
                endpoint_key.clone(),
                EndpointMetrics {
                    endpoint,
                    method,
                    total_requests: data.total_requests,
                    rate_limited_requests: data.rate_limited_requests,
                    hit_rate: endpoint_hit_rate,
                    avg_check_time_us: endpoint_avg_time,
                },
            );
        }

        // Build IP metrics
        let mut ip_metrics = HashMap::new();
        for (ip, data) in &current.ip_data {
            let ip_hit_rate = if data.total_requests > 0 {
                data.rate_limited_requests as f64 / data.total_requests as f64
            } else {
                0.0
            };

            ip_metrics.insert(
                ip.clone(),
                IpMetrics {
                    ip: ip.clone(),
                    total_requests: data.total_requests,
                    rate_limited_requests: data.rate_limited_requests,
                    hit_rate: ip_hit_rate,
                    unique_endpoints: data.endpoints.len() as u32,
                },
            );
        }

        // Build top lists
        let mut top_limited_endpoints: Vec<_> = endpoint_metrics
            .values()
            .map(|m| TopEndpoint {
                endpoint: format!("{}:{}", m.method, m.endpoint),
                rate_limited_requests: m.rate_limited_requests,
                hit_rate: m.hit_rate,
            })
            .collect();
        top_limited_endpoints.sort_by(|a, b| b.rate_limited_requests.cmp(&a.rate_limited_requests));
        top_limited_endpoints.truncate(10);

        let mut top_limited_ips: Vec<_> = ip_metrics
            .values()
            .map(|m| TopIp {
                ip: m.ip.clone(),
                rate_limited_requests: m.rate_limited_requests,
                hit_rate: m.hit_rate,
            })
            .collect();
        top_limited_ips.sort_by(|a, b| b.rate_limited_requests.cmp(&a.rate_limited_requests));
        top_limited_ips.truncate(10);

        RateLimitMetrics {
            timestamp: current.period_start,
            total_requests: current.total_requests,
            rate_limited_requests: current.rate_limited_requests,
            hit_rate,
            avg_check_time_us,
            endpoint_metrics,
            ip_metrics,
            top_limited_endpoints,
            top_limited_ips,
        }
    }

    /// Process metrics and check for alerts
    async fn process_metrics(&self, metrics: &RateLimitMetrics) {
        // Store metrics
        let mut history = self.metrics_history.write().await;
        history.push(metrics.clone());

        // Clean up old metrics
        let cutoff_time = metrics.timestamp - self.config.metrics_retention_seconds;
        history.retain(|m| m.timestamp >= cutoff_time);

        info!(
            timestamp = metrics.timestamp,
            total_requests = metrics.total_requests,
            rate_limited_requests = metrics.rate_limited_requests,
            hit_rate = metrics.hit_rate,
            "Processed rate limit metrics"
        );

        // Check for alerts
        self.check_alerts(metrics).await;
    }

    /// Check for alert conditions
    async fn check_alerts(&self, metrics: &RateLimitMetrics) {
        // Check high hit rate alert
        if metrics.hit_rate >= self.config.alert_thresholds.hit_rate_threshold
            && metrics.total_requests >= self.config.alert_thresholds.min_requests_per_minute
        {
            self.trigger_alert(
                AlertType::HighHitRate,
                AlertSeverity::High,
                format!(
                    "High rate limit hit rate: {:.2}% ({} requests)",
                    metrics.hit_rate * 100.0,
                    metrics.total_requests
                ),
                serde_json::json!({
                    "hit_rate": metrics.hit_rate,
                    "total_requests": metrics.total_requests,
                    "threshold": self.config.alert_thresholds.hit_rate_threshold
                }),
            )
            .await;
        }

        // Check for suspicious IP activity
        for ip_metric in metrics.ip_metrics.values() {
            if ip_metric.hit_rate >= 0.9 && ip_metric.total_requests >= 50 {
                self.trigger_alert(
                    AlertType::SuspiciousIp,
                    AlertSeverity::Medium,
                    format!(
                        "Suspicious IP activity: {} ({:.1}% hit rate, {} requests)",
                        ip_metric.ip,
                        ip_metric.hit_rate * 100.0,
                        ip_metric.total_requests
                    ),
                    serde_json::json!({
                        "ip": ip_metric.ip,
                        "hit_rate": ip_metric.hit_rate,
                        "total_requests": ip_metric.total_requests,
                        "unique_endpoints": ip_metric.unique_endpoints
                    }),
                )
                .await;
            }
        }

        // Check for endpoint attacks
        for endpoint_metric in metrics.endpoint_metrics.values() {
            if endpoint_metric.hit_rate >= 0.95 && endpoint_metric.total_requests >= 100 {
                self.trigger_alert(
                    AlertType::EndpointAttack,
                    AlertSeverity::High,
                    format!(
                        "Potential endpoint attack: {} {} ({:.1}% hit rate, {} requests)",
                        endpoint_metric.method,
                        endpoint_metric.endpoint,
                        endpoint_metric.hit_rate * 100.0,
                        endpoint_metric.total_requests
                    ),
                    serde_json::json!({
                        "endpoint": endpoint_metric.endpoint,
                        "method": endpoint_metric.method,
                        "hit_rate": endpoint_metric.hit_rate,
                        "total_requests": endpoint_metric.total_requests
                    }),
                )
                .await;
            }
        }

        // Check for performance issues
        if metrics.avg_check_time_us > 10000 {
            // More than 10ms average
            self.trigger_alert(
                AlertType::PerformanceIssue,
                AlertSeverity::Medium,
                format!(
                    "Rate limiter performance issue: {:.2}ms average check time",
                    metrics.avg_check_time_us as f64 / 1000.0
                ),
                serde_json::json!({
                    "avg_check_time_us": metrics.avg_check_time_us,
                    "total_requests": metrics.total_requests
                }),
            )
            .await;
        }
    }

    /// Trigger an alert
    async fn trigger_alert(
        &self,
        alert_type: AlertType,
        severity: AlertSeverity,
        message: String,
        details: serde_json::Value,
    ) {
        let alert_id = format!("{:?}_{}", alert_type, SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs());

        let alert = RateLimitAlert {
            alert_id: alert_id.clone(),
            alert_type,
            severity,
            message: message.clone(),
            details,
            triggered_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            active: true,
        };

        // Store alert
        let mut active_alerts = self.active_alerts.write().await;
        let mut alert_history = self.alert_history.write().await;

        active_alerts.insert(alert_id.clone(), alert.clone());
        alert_history.push(alert.clone());

        warn!(
            alert_id = %alert_id,
            alert_type = ?alert.alert_type,
            severity = ?alert.severity,
            message = %message,
            "Rate limit alert triggered"
        );

        // Clean up old alerts from history
        let cutoff_time = alert.triggered_at - (86400 * 30); // 30 days
        alert_history.retain(|a| a.triggered_at >= cutoff_time);
    }

    /// Get current metrics
    pub async fn get_current_metrics(&self) -> Option<RateLimitMetrics> {
        let history = self.metrics_history.read().await;
        history.last().cloned()
    }

    /// Get metrics history
    pub async fn get_metrics_history(&self, limit: Option<usize>) -> Vec<RateLimitMetrics> {
        let history = self.metrics_history.read().await;
        let limit = limit.unwrap_or(100);
        
        if history.len() <= limit {
            history.clone()
        } else {
            history[history.len() - limit..].to_vec()
        }
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<RateLimitAlert> {
        let alerts = self.active_alerts.read().await;
        alerts.values().cloned().collect()
    }

    /// Get alert history
    pub async fn get_alert_history(&self, limit: Option<usize>) -> Vec<RateLimitAlert> {
        let history = self.alert_history.read().await;
        let limit = limit.unwrap_or(100);
        
        if history.len() <= limit {
            history.clone()
        } else {
            history[history.len() - limit..].to_vec()
        }
    }

    /// Acknowledge an alert
    pub async fn acknowledge_alert(&self, alert_id: &str) -> bool {
        let mut active_alerts = self.active_alerts.write().await;
        if let Some(alert) = active_alerts.get_mut(alert_id) {
            alert.active = false;
            true
        } else {
            false
        }
    }

    /// Get monitoring statistics
    pub async fn get_stats(&self) -> MonitoringStats {
        let history = self.metrics_history.read().await;
        let active_alerts = self.active_alerts.read().await;
        let alert_history = self.alert_history.read().await;

        MonitoringStats {
            metrics_collected: history.len(),
            active_alerts: active_alerts.len(),
            total_alerts: alert_history.len(),
            monitoring_enabled: self.config.enabled,
            collection_interval_seconds: self.config.collection_interval_seconds,
        }
    }
}

/// Monitoring statistics
#[derive(Debug, Serialize)]
pub struct MonitoringStats {
    /// Number of metrics periods collected
    pub metrics_collected: usize,
    /// Number of active alerts
    pub active_alerts: usize,
    /// Total number of alerts triggered
    pub total_alerts: usize,
    /// Whether monitoring is enabled
    pub monitoring_enabled: bool,
    /// Collection interval in seconds
    pub collection_interval_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_monitor_creation() {
        let config = RateLimitMonitoringConfig::default();
        let monitor = RateLimitMonitor::new(config);
        
        let stats = monitor.get_stats().await;
        assert_eq!(stats.metrics_collected, 0);
        assert_eq!(stats.active_alerts, 0);
        assert!(stats.monitoring_enabled);
    }

    #[tokio::test]
    async fn test_record_check() {
        let config = RateLimitMonitoringConfig::default();
        let monitor = RateLimitMonitor::new(config);
        
        let result = RateLimitResult::Allowed {
            remaining: 99,
            reset_time: 1234567890,
            limit: 100,
        };
        
        monitor.record_check(
            "/api/v2/pipelines".to_string(),
            "GET".to_string(),
            "192.168.1.1".to_string(),
            &result,
            1000,
        ).await;
        
        // Check that data was recorded
        let current = monitor.current_period.read().await;
        assert_eq!(current.total_requests, 1);
        assert_eq!(current.rate_limited_requests, 0);
    }

    #[tokio::test]
    async fn test_alert_triggering() {
        let config = RateLimitMonitoringConfig::default();
        let monitor = RateLimitMonitor::new(config);
        
        monitor.trigger_alert(
            AlertType::HighHitRate,
            AlertSeverity::High,
            "Test alert".to_string(),
            serde_json::json!({"test": true}),
        ).await;
        
        let active_alerts = monitor.get_active_alerts().await;
        assert_eq!(active_alerts.len(), 1);
        assert_eq!(active_alerts[0].message, "Test alert");
    }

    #[test]
    fn test_alert_thresholds_default() {
        let thresholds = AlertThresholds::default();
        assert_eq!(thresholds.hit_rate_threshold, 0.8);
        assert_eq!(thresholds.min_requests_per_minute, 100);
        assert_eq!(thresholds.consecutive_periods, 3);
    }
}