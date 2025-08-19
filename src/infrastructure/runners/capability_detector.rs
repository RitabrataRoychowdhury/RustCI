// Runner Capability Detector
// Task 3.3: Unified Runner System

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use reqwest::Client;

use crate::error::{AppError, Result};
use crate::core::networking::valkyrie::engine::ValkyrieEngine;

/// Detects and manages runner capabilities and protocol support
pub struct RunnerCapabilityDetector {
    // HTTP client for capability detection
    http_client: Client,
    
    // Valkyrie engine for protocol detection
    valkyrie_engine: Option<Arc<ValkyrieEngine>>,
    
    // Capability cache
    capability_cache: Arc<RwLock<HashMap<String, CachedCapability>>>,
    
    // Detection configuration
    config: CapabilityDetectorConfig,
}

/// Configuration for capability detection
#[derive(Debug, Clone)]
pub struct CapabilityDetectorConfig {
    pub detection_timeout: Duration,
    pub cache_ttl: Duration,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
    pub parallel_detection: bool,
    pub enable_valkyrie_detection: bool,
    pub enable_http_detection: bool,
}

/// Cached capability information
#[derive(Debug, Clone)]
pub struct CachedCapability {
    pub runner_endpoint: String,
    pub capabilities: DetectedCapabilities,
    pub detected_at: Instant,
    pub last_verified: Instant,
    pub verification_count: u32,
}

/// Comprehensive runner capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedCapabilities {
    // Protocol support
    pub protocol_support: ProtocolSupport,
    
    // Hardware capabilities
    pub hardware: HardwareCapabilities,
    
    // Software capabilities
    pub software: SoftwareCapabilities,
    
    // Performance characteristics
    pub performance: PerformanceCapabilities,
    
    // Network capabilities
    pub network: NetworkCapabilities,
    
    // Security capabilities
    pub security: SecurityCapabilities,
    
    // Custom capabilities
    pub custom: HashMap<String, String>,
}

/// Protocol support detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSupport {
    pub valkyrie: ValkyrieProtocolSupport,
    pub http: HttpProtocolSupport,
    pub websocket: WebSocketSupport,
    pub grpc: GrpcSupport,
    pub preferred_protocol: PreferredProtocol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValkyrieProtocolSupport {
    pub supported: bool,
    pub version: Option<String>,
    pub features: Vec<String>,
    pub performance_tier: PerformanceTier,
    pub latency_benchmark: Option<Duration>,
    pub throughput_benchmark: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProtocolSupport {
    pub supported: bool,
    pub version: String, // HTTP/1.1, HTTP/2, HTTP/3
    pub features: Vec<String>,
    pub max_concurrent_requests: Option<u32>,
    pub keep_alive_support: bool,
    pub compression_support: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketSupport {
    pub supported: bool,
    pub version: Option<String>,
    pub extensions: Vec<String>,
    pub max_message_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcSupport {
    pub supported: bool,
    pub version: Option<String>,
    pub streaming_support: bool,
    pub compression: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PreferredProtocol {
    Valkyrie,
    Http,
    WebSocket,
    Grpc,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceTier {
    Ultra,    // <10μs latency
    High,     // <100μs latency
    Standard, // <1ms latency
    Basic,    // >1ms latency
}

/// Hardware capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCapabilities {
    pub cpu: CpuCapabilities,
    pub memory: MemoryCapabilities,
    pub storage: StorageCapabilities,
    pub gpu: Option<GpuCapabilities>,
    pub network: NetworkHardware,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCapabilities {
    pub cores: u32,
    pub threads: u32,
    pub architecture: String, // x86_64, arm64, etc.
    pub features: Vec<String>, // AVX, SSE, NEON, etc.
    pub frequency: Option<u64>, // MHz
    pub cache_sizes: Vec<u64>, // L1, L2, L3 cache sizes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCapabilities {
    pub total: u64, // bytes
    pub available: u64, // bytes
    pub memory_type: String, // DDR4, DDR5, etc.
    pub bandwidth: Option<u64>, // MB/s
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCapabilities {
    pub total: u64, // bytes
    pub available: u64, // bytes
    pub storage_type: String, // SSD, NVMe, HDD
    pub read_speed: Option<u64>, // MB/s
    pub write_speed: Option<u64>, // MB/s
    pub iops: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuCapabilities {
    pub count: u32,
    pub model: String,
    pub memory: u64, // bytes
    pub compute_capability: Option<String>,
    pub cuda_support: bool,
    pub opencl_support: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkHardware {
    pub interfaces: Vec<NetworkInterface>,
    pub bandwidth: Option<u64>, // Mbps
    pub latency: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub interface_type: String, // ethernet, wifi, etc.
    pub speed: Option<u64>, // Mbps
    pub mtu: Option<u32>,
}

/// Software capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareCapabilities {
    pub operating_system: OperatingSystem,
    pub container_runtime: Vec<ContainerRuntime>,
    pub programming_languages: Vec<ProgrammingLanguage>,
    pub build_tools: Vec<BuildTool>,
    pub testing_frameworks: Vec<TestingFramework>,
    pub deployment_tools: Vec<DeploymentTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatingSystem {
    pub name: String, // Linux, Windows, macOS
    pub version: String,
    pub distribution: Option<String>, // Ubuntu, CentOS, etc.
    pub kernel_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRuntime {
    pub name: String, // Docker, Podman, containerd
    pub version: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgrammingLanguage {
    pub name: String, // Rust, Python, Node.js, etc.
    pub version: String,
    pub package_managers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildTool {
    pub name: String, // cargo, npm, maven, etc.
    pub version: String,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestingFramework {
    pub name: String, // pytest, jest, cargo test, etc.
    pub version: String,
    pub parallel_support: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentTool {
    pub name: String, // kubectl, helm, terraform, etc.
    pub version: String,
    pub cloud_providers: Vec<String>,
}

/// Performance capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceCapabilities {
    pub benchmark_results: BenchmarkResults,
    pub resource_limits: ResourceLimits,
    pub scaling_characteristics: ScalingCharacteristics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub cpu_benchmark: Option<f64>, // FLOPS or similar
    pub memory_benchmark: Option<f64>, // MB/s
    pub storage_benchmark: Option<f64>, // IOPS
    pub network_benchmark: Option<f64>, // Mbps
    pub job_execution_benchmark: Option<Duration>, // Average job time
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_concurrent_jobs: u32,
    pub max_memory_per_job: Option<u64>,
    pub max_cpu_per_job: Option<f64>,
    pub max_execution_time: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingCharacteristics {
    pub supports_horizontal_scaling: bool,
    pub supports_vertical_scaling: bool,
    pub auto_scaling_enabled: bool,
    pub scaling_metrics: Vec<String>,
}

/// Network capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkCapabilities {
    pub connectivity: ConnectivityInfo,
    pub security: NetworkSecurity,
    pub quality_of_service: QosCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityInfo {
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
    pub hostname: String,
    pub ports: Vec<PortInfo>,
    pub firewall_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortInfo {
    pub port: u16,
    pub protocol: String, // TCP, UDP
    pub service: String,
    pub accessible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSecurity {
    pub tls_support: TlsSupport,
    pub vpn_support: bool,
    pub firewall_enabled: bool,
    pub intrusion_detection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsSupport {
    pub versions: Vec<String>, // TLS 1.2, TLS 1.3
    pub cipher_suites: Vec<String>,
    pub certificate_validation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QosCapabilities {
    pub bandwidth_control: bool,
    pub priority_queuing: bool,
    pub traffic_shaping: bool,
    pub latency_guarantees: bool,
}

/// Security capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityCapabilities {
    pub authentication: AuthenticationCapabilities,
    pub authorization: AuthorizationCapabilities,
    pub encryption: EncryptionCapabilities,
    pub compliance: ComplianceCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationCapabilities {
    pub methods: Vec<String>, // OAuth, JWT, mTLS, etc.
    pub multi_factor: bool,
    pub certificate_auth: bool,
    pub api_key_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationCapabilities {
    pub rbac_support: bool,
    pub abac_support: bool,
    pub policy_engine: Option<String>,
    pub fine_grained_permissions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionCapabilities {
    pub at_rest: bool,
    pub in_transit: bool,
    pub key_management: Vec<String>,
    pub algorithms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCapabilities {
    pub standards: Vec<String>, // SOC2, HIPAA, etc.
    pub audit_logging: bool,
    pub data_residency: bool,
    pub privacy_controls: bool,
}

impl Default for CapabilityDetectorConfig {
    fn default() -> Self {
        Self {
            detection_timeout: Duration::from_secs(30),
            cache_ttl: Duration::from_secs(300), // 5 minutes
            retry_attempts: 3,
            retry_delay: Duration::from_secs(1),
            parallel_detection: true,
            enable_valkyrie_detection: true,
            enable_http_detection: true,
        }
    }
}

impl RunnerCapabilityDetector {
    /// Create a new capability detector
    pub async fn new(
        valkyrie_engine: Option<Arc<ValkyrieEngine>>,
        config: CapabilityDetectorConfig,
    ) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(config.detection_timeout)
            .build()
            .map_err(|e| AppError::InternalError {
                component: "capability_detector".to_string(),
                message: format!("Failed to create HTTP client: {}", e),
            })?;
        
        Ok(Self {
            http_client,
            valkyrie_engine,
            capability_cache: Arc::new(RwLock::new(HashMap::new())),
            config,
        })
    }
    
    /// Detect capabilities for a runner endpoint
    pub async fn detect_capabilities(
        &self,
        endpoint: &str,
    ) -> Result<DetectedCapabilities> {
        // Check cache first
        if let Some(cached) = self.get_cached_capability(endpoint).await {
            if cached.detected_at.elapsed() < self.config.cache_ttl {
                return Ok(cached.capabilities);
            }
        }
        
        // Perform fresh detection
        let capabilities = self.perform_capability_detection(endpoint).await?;
        
        // Cache the results
        self.cache_capability(endpoint, capabilities.clone()).await;
        
        Ok(capabilities)
    }
    
    /// Perform actual capability detection
    async fn perform_capability_detection(
        &self,
        endpoint: &str,
    ) -> Result<DetectedCapabilities> {
        let mut detection_tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        
        // Detect protocol support
        if self.config.parallel_detection {
            // Run detections in parallel
            let protocol_task = self.detect_protocol_support(endpoint);
            let hardware_task = self.detect_hardware_capabilities(endpoint);
            let software_task = self.detect_software_capabilities(endpoint);
            let performance_task = self.detect_performance_capabilities(endpoint);
            let network_task = self.detect_network_capabilities(endpoint);
            let security_task = self.detect_security_capabilities(endpoint);
            
            let (protocol, hardware, software, performance, network, security) = tokio::try_join!(
                protocol_task,
                hardware_task,
                software_task,
                performance_task,
                network_task,
                security_task
            )?;
            
            Ok(DetectedCapabilities {
                protocol_support: protocol,
                hardware,
                software,
                performance,
                network,
                security,
                custom: HashMap::new(),
            })
        } else {
            // Run detections sequentially
            let protocol_support = self.detect_protocol_support(endpoint).await?;
            let hardware = self.detect_hardware_capabilities(endpoint).await?;
            let software = self.detect_software_capabilities(endpoint).await?;
            let performance = self.detect_performance_capabilities(endpoint).await?;
            let network = self.detect_network_capabilities(endpoint).await?;
            let security = self.detect_security_capabilities(endpoint).await?;
            
            Ok(DetectedCapabilities {
                protocol_support,
                hardware,
                software,
                performance,
                network,
                security,
                custom: HashMap::new(),
            })
        }
    }
    
    /// Detect protocol support
    async fn detect_protocol_support(&self, endpoint: &str) -> Result<ProtocolSupport> {
        let mut valkyrie_support = ValkyrieProtocolSupport {
            supported: false,
            version: None,
            features: Vec::new(),
            performance_tier: PerformanceTier::Basic,
            latency_benchmark: None,
            throughput_benchmark: None,
        };
        
        let mut http_support = HttpProtocolSupport {
            supported: false,
            version: "HTTP/1.1".to_string(),
            features: Vec::new(),
            max_concurrent_requests: None,
            keep_alive_support: false,
            compression_support: Vec::new(),
        };
        
        // Detect Valkyrie protocol support
        if self.config.enable_valkyrie_detection {
            if let Some(valkyrie_engine) = &self.valkyrie_engine {
                valkyrie_support = self.detect_valkyrie_support(endpoint, valkyrie_engine).await?;
            }
        }
        
        // Detect HTTP protocol support
        if self.config.enable_http_detection {
            http_support = self.detect_http_support(endpoint).await?;
        }
        
        // Determine preferred protocol
        let preferred_protocol = if valkyrie_support.supported && 
            matches!(valkyrie_support.performance_tier, PerformanceTier::Ultra | PerformanceTier::High) {
            PreferredProtocol::Valkyrie
        } else if http_support.supported {
            PreferredProtocol::Http
        } else {
            PreferredProtocol::Auto
        };
        
        Ok(ProtocolSupport {
            valkyrie: valkyrie_support,
            http: http_support,
            websocket: WebSocketSupport {
                supported: false, // Would be detected
                version: None,
                extensions: Vec::new(),
                max_message_size: None,
            },
            grpc: GrpcSupport {
                supported: false, // Would be detected
                version: None,
                streaming_support: false,
                compression: Vec::new(),
            },
            preferred_protocol,
        })
    }
    
    /// Detect Valkyrie protocol support
    async fn detect_valkyrie_support(
        &self,
        _endpoint: &str,
        _valkyrie_engine: &ValkyrieEngine,
    ) -> Result<ValkyrieProtocolSupport> {
        // This would perform actual Valkyrie protocol detection
        // For now, return a placeholder
        Ok(ValkyrieProtocolSupport {
            supported: true, // Would be detected
            version: Some("2.0.0".to_string()),
            features: vec!["zero_copy".to_string(), "intelligent_routing".to_string()],
            performance_tier: PerformanceTier::High,
            latency_benchmark: Some(Duration::from_micros(50)),
            throughput_benchmark: Some(1000000), // 1M ops/sec
        })
    }
    
    /// Detect HTTP protocol support
    async fn detect_http_support(&self, endpoint: &str) -> Result<HttpProtocolSupport> {
        // Attempt HTTP connection to detect capabilities
        let response = self.http_client
            .get(&format!("http://{}/health", endpoint))
            .send()
            .await;
        
        match response {
            Ok(resp) => {
                let version = match resp.version() {
                    reqwest::Version::HTTP_09 => "HTTP/0.9",
                    reqwest::Version::HTTP_10 => "HTTP/1.0",
                    reqwest::Version::HTTP_11 => "HTTP/1.1",
                    reqwest::Version::HTTP_2 => "HTTP/2",
                    reqwest::Version::HTTP_3 => "HTTP/3",
                    _ => "HTTP/1.1",
                };
                
                Ok(HttpProtocolSupport {
                    supported: true,
                    version: version.to_string(),
                    features: vec!["keep_alive".to_string()],
                    max_concurrent_requests: Some(100), // Would be detected
                    keep_alive_support: true,
                    compression_support: vec!["gzip".to_string(), "deflate".to_string()],
                })
            }
            Err(_) => {
                Ok(HttpProtocolSupport {
                    supported: false,
                    version: "HTTP/1.1".to_string(),
                    features: Vec::new(),
                    max_concurrent_requests: None,
                    keep_alive_support: false,
                    compression_support: Vec::new(),
                })
            }
        }
    }
    
    // Placeholder implementations for other detection methods
    async fn detect_hardware_capabilities(&self, _endpoint: &str) -> Result<HardwareCapabilities> {
        // Would perform actual hardware detection
        Ok(HardwareCapabilities {
            cpu: CpuCapabilities {
                cores: 4,
                threads: 8,
                architecture: "x86_64".to_string(),
                features: vec!["AVX2".to_string(), "SSE4".to_string()],
                frequency: Some(3000), // 3 GHz
                cache_sizes: vec![32768, 262144, 8388608], // L1, L2, L3
            },
            memory: MemoryCapabilities {
                total: 8 * 1024 * 1024 * 1024, // 8 GB
                available: 6 * 1024 * 1024 * 1024, // 6 GB
                memory_type: "DDR4".to_string(),
                bandwidth: Some(25600), // MB/s
            },
            storage: StorageCapabilities {
                total: 500 * 1024 * 1024 * 1024, // 500 GB
                available: 400 * 1024 * 1024 * 1024, // 400 GB
                storage_type: "NVMe SSD".to_string(),
                read_speed: Some(3500), // MB/s
                write_speed: Some(3000), // MB/s
                iops: Some(500000),
            },
            gpu: None,
            network: NetworkHardware {
                interfaces: vec![NetworkInterface {
                    name: "eth0".to_string(),
                    interface_type: "ethernet".to_string(),
                    speed: Some(1000), // 1 Gbps
                    mtu: Some(1500),
                }],
                bandwidth: Some(1000),
                latency: Some(Duration::from_millis(1)),
            },
        })
    }
    
    async fn detect_software_capabilities(&self, _endpoint: &str) -> Result<SoftwareCapabilities> {
        // Would perform actual software detection
        Ok(SoftwareCapabilities {
            operating_system: OperatingSystem {
                name: "Linux".to_string(),
                version: "5.4.0".to_string(),
                distribution: Some("Ubuntu".to_string()),
                kernel_version: Some("5.4.0-74-generic".to_string()),
            },
            container_runtime: vec![ContainerRuntime {
                name: "Docker".to_string(),
                version: "20.10.7".to_string(),
                features: vec!["buildkit".to_string(), "compose".to_string()],
            }],
            programming_languages: vec![
                ProgrammingLanguage {
                    name: "Rust".to_string(),
                    version: "1.70.0".to_string(),
                    package_managers: vec!["cargo".to_string()],
                },
                ProgrammingLanguage {
                    name: "Python".to_string(),
                    version: "3.9.7".to_string(),
                    package_managers: vec!["pip".to_string(), "conda".to_string()],
                },
            ],
            build_tools: vec![BuildTool {
                name: "cargo".to_string(),
                version: "1.70.0".to_string(),
                features: vec!["workspace".to_string(), "features".to_string()],
            }],
            testing_frameworks: vec![TestingFramework {
                name: "cargo test".to_string(),
                version: "1.70.0".to_string(),
                parallel_support: true,
            }],
            deployment_tools: vec![DeploymentTool {
                name: "kubectl".to_string(),
                version: "1.21.0".to_string(),
                cloud_providers: vec!["AWS".to_string(), "GCP".to_string()],
            }],
        })
    }
    
    async fn detect_performance_capabilities(&self, _endpoint: &str) -> Result<PerformanceCapabilities> {
        // Would perform actual performance benchmarking
        Ok(PerformanceCapabilities {
            benchmark_results: BenchmarkResults {
                cpu_benchmark: Some(1000.0), // GFLOPS
                memory_benchmark: Some(25600.0), // MB/s
                storage_benchmark: Some(500000.0), // IOPS
                network_benchmark: Some(1000.0), // Mbps
                job_execution_benchmark: Some(Duration::from_secs(30)),
            },
            resource_limits: ResourceLimits {
                max_concurrent_jobs: 10,
                max_memory_per_job: Some(2 * 1024 * 1024 * 1024), // 2 GB
                max_cpu_per_job: Some(2.0), // 2 cores
                max_execution_time: Some(Duration::from_secs(3600)), // 1 hour
            },
            scaling_characteristics: ScalingCharacteristics {
                supports_horizontal_scaling: false,
                supports_vertical_scaling: true,
                auto_scaling_enabled: false,
                scaling_metrics: vec!["cpu".to_string(), "memory".to_string()],
            },
        })
    }
    
    async fn detect_network_capabilities(&self, _endpoint: &str) -> Result<NetworkCapabilities> {
        // Would perform actual network capability detection
        Ok(NetworkCapabilities {
            connectivity: ConnectivityInfo {
                public_ip: Some("203.0.113.1".to_string()),
                private_ip: Some("10.0.1.100".to_string()),
                hostname: "runner-001".to_string(),
                ports: vec![PortInfo {
                    port: 8080,
                    protocol: "TCP".to_string(),
                    service: "HTTP".to_string(),
                    accessible: true,
                }],
                firewall_rules: vec!["allow-http".to_string()],
            },
            security: NetworkSecurity {
                tls_support: TlsSupport {
                    versions: vec!["TLS 1.2".to_string(), "TLS 1.3".to_string()],
                    cipher_suites: vec!["ECDHE-RSA-AES256-GCM-SHA384".to_string()],
                    certificate_validation: true,
                },
                vpn_support: false,
                firewall_enabled: true,
                intrusion_detection: false,
            },
            quality_of_service: QosCapabilities {
                bandwidth_control: false,
                priority_queuing: false,
                traffic_shaping: false,
                latency_guarantees: false,
            },
        })
    }
    
    async fn detect_security_capabilities(&self, _endpoint: &str) -> Result<SecurityCapabilities> {
        // Would perform actual security capability detection
        Ok(SecurityCapabilities {
            authentication: AuthenticationCapabilities {
                methods: vec!["JWT".to_string(), "API_KEY".to_string()],
                multi_factor: false,
                certificate_auth: true,
                api_key_auth: true,
            },
            authorization: AuthorizationCapabilities {
                rbac_support: true,
                abac_support: false,
                policy_engine: Some("OPA".to_string()),
                fine_grained_permissions: true,
            },
            encryption: EncryptionCapabilities {
                at_rest: true,
                in_transit: true,
                key_management: vec!["local".to_string()],
                algorithms: vec!["AES-256".to_string(), "ChaCha20".to_string()],
            },
            compliance: ComplianceCapabilities {
                standards: vec!["SOC2".to_string()],
                audit_logging: true,
                data_residency: false,
                privacy_controls: true,
            },
        })
    }
    
    /// Get cached capability if available and valid
    async fn get_cached_capability(&self, endpoint: &str) -> Option<CachedCapability> {
        let cache = self.capability_cache.read().await;
        cache.get(endpoint).cloned()
    }
    
    /// Cache capability detection results
    async fn cache_capability(&self, endpoint: &str, capabilities: DetectedCapabilities) {
        let cached = CachedCapability {
            runner_endpoint: endpoint.to_string(),
            capabilities,
            detected_at: Instant::now(),
            last_verified: Instant::now(),
            verification_count: 1,
        };
        
        let mut cache = self.capability_cache.write().await;
        cache.insert(endpoint.to_string(), cached);
    }
    
    /// Verify cached capabilities are still valid
    pub async fn verify_capabilities(&self, endpoint: &str) -> Result<bool> {
        // Quick verification check
        let response = self.http_client
            .head(&format!("http://{}/health", endpoint))
            .send()
            .await;
        
        Ok(response.is_ok())
    }
    
    /// Clear capability cache
    pub async fn clear_cache(&self) {
        let mut cache = self.capability_cache.write().await;
        cache.clear();
    }
    
    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> CacheStats {
        let cache = self.capability_cache.read().await;
        let now = Instant::now();
        
        let total_entries = cache.len();
        let expired_entries = cache.values()
            .filter(|c| now.duration_since(c.detected_at) > self.config.cache_ttl)
            .count();
        
        CacheStats {
            total_entries,
            valid_entries: total_entries - expired_entries,
            expired_entries,
            cache_hit_rate: 0.0, // Would be calculated from actual usage
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub expired_entries: usize,
    pub cache_hit_rate: f64,
}