//! Protocol Negotiation for Valkyrie Bridge
//! 
//! Handles automatic protocol detection and negotiation between HTTP/HTTPS
//! and Valkyrie Protocol, with transparent fallback capabilities.

use super::{BridgeError, BridgeResult};
use axum::http::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Protocol negotiation manager
pub struct ProtocolNegotiator {
    /// Supported protocols in order of preference
    supported_protocols: Vec<SupportedProtocol>,
    /// Negotiation configuration
    config: NegotiationConfig,
}

/// Supported protocol information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedProtocol {
    /// Protocol name
    pub name: String,
    /// Protocol version
    pub version: String,
    /// Protocol capabilities
    pub capabilities: Vec<String>,
    /// Protocol priority (higher = more preferred)
    pub priority: u8,
    /// Whether this protocol is enabled
    pub enabled: bool,
}

/// Protocol negotiation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NegotiationConfig {
    /// Enable automatic protocol detection
    pub auto_detection_enabled: bool,
    /// Enable protocol upgrade suggestions
    pub upgrade_suggestions_enabled: bool,
    /// Default protocol when negotiation fails
    pub default_protocol: String,
    /// Maximum negotiation attempts
    pub max_negotiation_attempts: u8,
    /// Negotiation timeout in milliseconds
    pub negotiation_timeout_ms: u64,
    /// Enable protocol version compatibility checks
    pub version_compatibility_enabled: bool,
}

impl Default for NegotiationConfig {
    fn default() -> Self {
        Self {
            auto_detection_enabled: true,
            upgrade_suggestions_enabled: true,
            default_protocol: "http/1.1".to_string(),
            max_negotiation_attempts: 3,
            negotiation_timeout_ms: 5000,
            version_compatibility_enabled: true,
        }
    }
}

/// Protocol negotiation result
#[derive(Debug, Clone)]
pub struct NegotiationResult {
    /// Selected protocol
    pub selected_protocol: SupportedProtocol,
    /// Negotiation method used
    pub negotiation_method: NegotiationMethod,
    /// Whether fallback was used
    pub fallback_used: bool,
    /// Additional negotiation metadata
    pub metadata: HashMap<String, String>,
}

/// Protocol negotiation methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NegotiationMethod {
    /// HTTP Accept header negotiation
    HttpAccept,
    /// HTTP Upgrade header negotiation
    HttpUpgrade,
    /// Custom header negotiation
    CustomHeader(String),
    /// Query parameter negotiation
    QueryParameter(String),
    /// User-Agent based detection
    UserAgent,
    /// Automatic detection based on request characteristics
    AutoDetection,
    /// Default fallback
    Fallback,
}

/// Client capabilities detected during negotiation
#[derive(Debug, Clone)]
pub struct ClientCapabilities {
    /// Supported protocols
    pub protocols: Vec<String>,
    /// Supported encodings
    pub encodings: Vec<String>,
    /// Supported content types
    pub content_types: Vec<String>,
    /// WebSocket support
    pub websocket_support: bool,
    /// HTTP/2 support
    pub http2_support: bool,
    /// Valkyrie protocol support
    pub valkyrie_support: bool,
    /// Client version information
    pub client_version: Option<String>,
}

impl ProtocolNegotiator {
    /// Create a new protocol negotiator
    pub fn new(config: NegotiationConfig) -> Self {
        let supported_protocols = vec![
            SupportedProtocol {
                name: "valkyrie".to_string(),
                version: "1.0".to_string(),
                capabilities: vec![
                    "streaming".to_string(),
                    "multiplexing".to_string(),
                    "encryption".to_string(),
                    "compression".to_string(),
                ],
                priority: 100,
                enabled: true,
            },
            SupportedProtocol {
                name: "websocket".to_string(),
                version: "13".to_string(),
                capabilities: vec![
                    "bidirectional".to_string(),
                    "real-time".to_string(),
                ],
                priority: 80,
                enabled: true,
            },
            SupportedProtocol {
                name: "http".to_string(),
                version: "2.0".to_string(),
                capabilities: vec![
                    "multiplexing".to_string(),
                    "server-push".to_string(),
                ],
                priority: 60,
                enabled: true,
            },
            SupportedProtocol {
                name: "http".to_string(),
                version: "1.1".to_string(),
                capabilities: vec![
                    "keep-alive".to_string(),
                    "chunked-encoding".to_string(),
                ],
                priority: 40,
                enabled: true,
            },
        ];

        Self {
            supported_protocols,
            config,
        }
    }

    /// Negotiate protocol based on HTTP headers
    pub async fn negotiate_protocol(
        &self,
        headers: &HeaderMap,
        query_params: &HashMap<String, String>,
    ) -> BridgeResult<NegotiationResult> {
        debug!("Starting protocol negotiation");

        // Detect client capabilities
        let client_capabilities = self.detect_client_capabilities(headers, query_params)?;
        
        // Try different negotiation methods in order of preference
        let negotiation_methods = vec![
            NegotiationMethod::CustomHeader("X-Valkyrie-Protocol".to_string()),
            NegotiationMethod::HttpUpgrade,
            NegotiationMethod::HttpAccept,
            NegotiationMethod::QueryParameter("protocol".to_string()),
            NegotiationMethod::UserAgent,
            NegotiationMethod::AutoDetection,
        ];

        for method in negotiation_methods {
            if let Ok(result) = self.try_negotiation_method(&method, headers, query_params, &client_capabilities).await {
                info!("Protocol negotiation successful: {:?} via {:?}", result.selected_protocol.name, method);
                return Ok(result);
            }
        }

        // Fallback to default protocol
        warn!("Protocol negotiation failed, using fallback: {}", self.config.default_protocol);
        self.create_fallback_result()
    }

    /// Detect client capabilities from headers and parameters
    fn detect_client_capabilities(
        &self,
        headers: &HeaderMap,
        query_params: &HashMap<String, String>,
    ) -> BridgeResult<ClientCapabilities> {
        let mut capabilities = ClientCapabilities {
            protocols: Vec::new(),
            encodings: Vec::new(),
            content_types: Vec::new(),
            websocket_support: false,
            http2_support: false,
            valkyrie_support: false,
            client_version: None,
        };

        // Check for Valkyrie protocol support
        if headers.contains_key("X-Valkyrie-Protocol") || 
           query_params.contains_key("valkyrie") {
            capabilities.valkyrie_support = true;
            capabilities.protocols.push("valkyrie".to_string());
        }

        // Check for WebSocket support
        if let Some(upgrade) = headers.get("upgrade") {
            if upgrade.to_str().unwrap_or("").to_lowercase().contains("websocket") {
                capabilities.websocket_support = true;
                capabilities.protocols.push("websocket".to_string());
            }
        }

        // Check for HTTP/2 support
        if let Some(http2_settings) = headers.get("http2-settings") {
            capabilities.http2_support = true;
            capabilities.protocols.push("http/2.0".to_string());
        }

        // Parse Accept header for content types
        if let Some(accept) = headers.get("accept") {
            if let Ok(accept_str) = accept.to_str() {
                capabilities.content_types = accept_str
                    .split(',')
                    .map(|s| s.trim().split(';').next().unwrap_or("").to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }

        // Parse Accept-Encoding header
        if let Some(encoding) = headers.get("accept-encoding") {
            if let Ok(encoding_str) = encoding.to_str() {
                capabilities.encodings = encoding_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
            }
        }

        // Extract client version from User-Agent
        if let Some(user_agent) = headers.get("user-agent") {
            capabilities.client_version = user_agent.to_str().ok().map(String::from);
        }

        // Default to HTTP/1.1 if no specific protocols detected
        if capabilities.protocols.is_empty() {
            capabilities.protocols.push("http/1.1".to_string());
        }

        Ok(capabilities)
    }

    /// Try a specific negotiation method
    async fn try_negotiation_method(
        &self,
        method: &NegotiationMethod,
        headers: &HeaderMap,
        query_params: &HashMap<String, String>,
        client_capabilities: &ClientCapabilities,
    ) -> BridgeResult<NegotiationResult> {
        match method {
            NegotiationMethod::CustomHeader(header_name) => {
                self.negotiate_via_custom_header(header_name, headers, client_capabilities).await
            }
            NegotiationMethod::HttpUpgrade => {
                self.negotiate_via_http_upgrade(headers, client_capabilities).await
            }
            NegotiationMethod::HttpAccept => {
                self.negotiate_via_http_accept(headers, client_capabilities).await
            }
            NegotiationMethod::QueryParameter(param_name) => {
                self.negotiate_via_query_param(param_name, query_params, client_capabilities).await
            }
            NegotiationMethod::UserAgent => {
                self.negotiate_via_user_agent(headers, client_capabilities).await
            }
            NegotiationMethod::AutoDetection => {
                self.negotiate_via_auto_detection(client_capabilities).await
            }
            NegotiationMethod::Fallback => {
                self.create_fallback_result()
            }
        }
    }

    /// Negotiate via custom header
    async fn negotiate_via_custom_header(
        &self,
        header_name: &str,
        headers: &HeaderMap,
        client_capabilities: &ClientCapabilities,
    ) -> BridgeResult<NegotiationResult> {
        if let Some(header_value) = headers.get(header_name) {
            if let Ok(protocol_str) = header_value.to_str() {
                if let Some(protocol) = self.find_matching_protocol(protocol_str) {
                    return Ok(NegotiationResult {
                        selected_protocol: protocol,
                        negotiation_method: NegotiationMethod::CustomHeader(header_name.to_string()),
                        fallback_used: false,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        Err(BridgeError::ProtocolNegotiationFailed {
            reason: format!("Custom header {} not found or invalid", header_name),
        })
    }

    /// Negotiate via HTTP Upgrade header
    async fn negotiate_via_http_upgrade(
        &self,
        headers: &HeaderMap,
        client_capabilities: &ClientCapabilities,
    ) -> BridgeResult<NegotiationResult> {
        if let Some(upgrade) = headers.get("upgrade") {
            if let Ok(upgrade_str) = upgrade.to_str() {
                for protocol_name in upgrade_str.split(',').map(|s| s.trim()) {
                    if let Some(protocol) = self.find_matching_protocol(protocol_name) {
                        return Ok(NegotiationResult {
                            selected_protocol: protocol,
                            negotiation_method: NegotiationMethod::HttpUpgrade,
                            fallback_used: false,
                            metadata: HashMap::new(),
                        });
                    }
                }
            }
        }

        Err(BridgeError::ProtocolNegotiationFailed {
            reason: "No suitable protocol found in Upgrade header".to_string(),
        })
    }

    /// Negotiate via HTTP Accept header
    async fn negotiate_via_http_accept(
        &self,
        headers: &HeaderMap,
        client_capabilities: &ClientCapabilities,
    ) -> BridgeResult<NegotiationResult> {
        // Check if client accepts Valkyrie-specific content types
        if client_capabilities.content_types.contains(&"application/valkyrie".to_string()) {
            if let Some(valkyrie_protocol) = self.find_protocol_by_name("valkyrie") {
                return Ok(NegotiationResult {
                    selected_protocol: valkyrie_protocol,
                    negotiation_method: NegotiationMethod::HttpAccept,
                    fallback_used: false,
                    metadata: HashMap::new(),
                });
            }
        }

        Err(BridgeError::ProtocolNegotiationFailed {
            reason: "No Valkyrie content type in Accept header".to_string(),
        })
    }

    /// Negotiate via query parameter
    async fn negotiate_via_query_param(
        &self,
        param_name: &str,
        query_params: &HashMap<String, String>,
        client_capabilities: &ClientCapabilities,
    ) -> BridgeResult<NegotiationResult> {
        if let Some(protocol_name) = query_params.get(param_name) {
            if let Some(protocol) = self.find_matching_protocol(protocol_name) {
                return Ok(NegotiationResult {
                    selected_protocol: protocol,
                    negotiation_method: NegotiationMethod::QueryParameter(param_name.to_string()),
                    fallback_used: false,
                    metadata: HashMap::new(),
                });
            }
        }

        Err(BridgeError::ProtocolNegotiationFailed {
            reason: format!("Query parameter {} not found or invalid", param_name),
        })
    }

    /// Negotiate via User-Agent detection
    async fn negotiate_via_user_agent(
        &self,
        headers: &HeaderMap,
        client_capabilities: &ClientCapabilities,
    ) -> BridgeResult<NegotiationResult> {
        if let Some(user_agent) = headers.get("user-agent") {
            if let Ok(ua_str) = user_agent.to_str() {
                // Check for known Valkyrie clients
                if ua_str.contains("Valkyrie") || ua_str.contains("RustCI") {
                    if let Some(valkyrie_protocol) = self.find_protocol_by_name("valkyrie") {
                        return Ok(NegotiationResult {
                            selected_protocol: valkyrie_protocol,
                            negotiation_method: NegotiationMethod::UserAgent,
                            fallback_used: false,
                            metadata: {
                                let mut meta = HashMap::new();
                                meta.insert("user_agent".to_string(), ua_str.to_string());
                                meta
                            },
                        });
                    }
                }
            }
        }

        Err(BridgeError::ProtocolNegotiationFailed {
            reason: "User-Agent does not indicate Valkyrie support".to_string(),
        })
    }

    /// Negotiate via automatic detection
    async fn negotiate_via_auto_detection(
        &self,
        client_capabilities: &ClientCapabilities,
    ) -> BridgeResult<NegotiationResult> {
        // Select the highest priority protocol that the client supports
        let mut best_protocol: Option<SupportedProtocol> = None;
        let mut best_priority = 0;

        for supported in &self.supported_protocols {
            if !supported.enabled {
                continue;
            }

            let client_supports = client_capabilities.protocols.iter()
                .any(|client_proto| {
                    client_proto.contains(&supported.name) ||
                    (supported.name == "valkyrie" && client_capabilities.valkyrie_support) ||
                    (supported.name == "websocket" && client_capabilities.websocket_support) ||
                    (supported.name == "http" && supported.version == "2.0" && client_capabilities.http2_support)
                });

            if client_supports && supported.priority > best_priority {
                best_protocol = Some(supported.clone());
                best_priority = supported.priority;
            }
        }

        if let Some(protocol) = best_protocol {
            Ok(NegotiationResult {
                selected_protocol: protocol,
                negotiation_method: NegotiationMethod::AutoDetection,
                fallback_used: false,
                metadata: HashMap::new(),
            })
        } else {
            Err(BridgeError::ProtocolNegotiationFailed {
                reason: "No compatible protocol found via auto-detection".to_string(),
            })
        }
    }

    /// Create fallback negotiation result
    fn create_fallback_result(&self) -> BridgeResult<NegotiationResult> {
        let fallback_protocol = self.find_protocol_by_name(&self.config.default_protocol)
            .or_else(|| self.supported_protocols.first().cloned())
            .ok_or_else(|| BridgeError::ProtocolNegotiationFailed {
                reason: "No fallback protocol available".to_string(),
            })?;

        Ok(NegotiationResult {
            selected_protocol: fallback_protocol,
            negotiation_method: NegotiationMethod::Fallback,
            fallback_used: true,
            metadata: HashMap::new(),
        })
    }

    /// Find a protocol by exact name match
    fn find_protocol_by_name(&self, name: &str) -> Option<SupportedProtocol> {
        self.supported_protocols.iter()
            .find(|p| p.enabled && p.name == name)
            .cloned()
    }

    /// Find a matching protocol (supports partial matches)
    fn find_matching_protocol(&self, protocol_str: &str) -> Option<SupportedProtocol> {
        let protocol_lower = protocol_str.to_lowercase();
        
        // Try exact match first
        if let Some(protocol) = self.find_protocol_by_name(&protocol_lower) {
            return Some(protocol);
        }

        // Try partial matches
        self.supported_protocols.iter()
            .find(|p| p.enabled && protocol_lower.contains(&p.name))
            .cloned()
    }

    /// Generate protocol upgrade suggestions
    pub fn generate_upgrade_suggestions(
        &self,
        current_protocol: &str,
    ) -> Vec<SupportedProtocol> {
        if !self.config.upgrade_suggestions_enabled {
            return Vec::new();
        }

        let current_priority = self.find_protocol_by_name(current_protocol)
            .map(|p| p.priority)
            .unwrap_or(0);

        self.supported_protocols.iter()
            .filter(|p| p.enabled && p.priority > current_priority)
            .cloned()
            .collect()
    }

    /// Check if protocol upgrade is available
    pub fn can_upgrade_protocol(&self, from: &str, to: &str) -> bool {
        let from_protocol = self.find_protocol_by_name(from);
        let to_protocol = self.find_protocol_by_name(to);

        match (from_protocol, to_protocol) {
            (Some(from_p), Some(to_p)) => to_p.priority > from_p.priority,
            _ => false,
        }
    }
}