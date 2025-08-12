//! REST API Translator for Valkyrie Protocol
//! 
//! Provides automatic translation between REST API calls and Valkyrie messages,
//! enabling existing REST clients to communicate with Valkyrie services seamlessly.

use super::{BridgeError, BridgeResult, HttpRequestContext};
use crate::core::networking::valkyrie::message::{
    ValkyrieMessage, MessageHeader, MessageType, MessagePayload, RoutingInfo,
    DestinationType, ServiceSelector, EndpointId, LoadBalancingStrategy,
    MessageFlags, MessagePriority, ProtocolInfo, StreamId, RoutingHints,
};
use axum::http::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn, error};
use uuid::Uuid;

/// REST API translator for converting REST calls to Valkyrie messages
pub struct RestTranslator {
    /// Translation rules configuration
    config: TranslatorConfig,
    /// API route mappings
    route_mappings: HashMap<String, RouteMapping>,
    /// Service discovery mappings
    service_mappings: HashMap<String, ServiceMapping>,
}

/// Translator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslatorConfig {
    /// Enable automatic route discovery
    pub auto_route_discovery: bool,
    /// Default service name for unmapped routes
    pub default_service: String,
    /// Enable request/response transformation
    pub enable_transformations: bool,
    /// Maximum request body size for translation
    pub max_request_size: usize,
    /// Enable caching of translation results
    pub enable_caching: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
}

impl Default for TranslatorConfig {
    fn default() -> Self {
        Self {
            auto_route_discovery: true,
            default_service: "rustci".to_string(),
            enable_transformations: true,
            max_request_size: 10 * 1024 * 1024, // 10MB
            enable_caching: true,
            cache_ttl_seconds: 300, // 5 minutes
        }
    }
}

/// Route mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMapping {
    /// REST API path pattern
    pub path_pattern: String,
    /// HTTP methods supported
    pub methods: Vec<String>,
    /// Target Valkyrie message type
    pub message_type: String,
    /// Target service name
    pub service_name: String,
    /// Request transformation rules
    pub request_transform: Option<TransformationRule>,
    /// Response transformation rules
    pub response_transform: Option<TransformationRule>,
    /// Additional routing metadata
    pub metadata: HashMap<String, String>,
}

/// Service mapping for service discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMapping {
    /// Service name
    pub name: String,
    /// Service version
    pub version: Option<String>,
    /// Service endpoints
    pub endpoints: Vec<String>,
    /// Load balancing strategy
    pub load_balancing: String,
    /// Service tags
    pub tags: HashMap<String, String>,
}

/// Transformation rule for request/response modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationRule {
    /// Field mappings (source -> target)
    pub field_mappings: HashMap<String, String>,
    /// Field transformations
    pub transformations: Vec<FieldTransformation>,
    /// Custom transformation script
    pub custom_script: Option<String>,
}

/// Field transformation specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldTransformation {
    /// Source field path
    pub source_field: String,
    /// Target field path
    pub target_field: String,
    /// Transformation type
    pub transform_type: TransformationType,
    /// Transformation parameters
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Types of field transformations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransformationType {
    /// Direct copy
    Copy,
    /// String formatting
    Format(String),
    /// Type conversion
    Convert(String),
    /// Conditional transformation
    Conditional {
        condition: String,
        true_value: serde_json::Value,
        false_value: serde_json::Value,
    },
    /// Custom function
    Custom(String),
}

/// REST request representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestRequest {
    /// HTTP method
    pub method: String,
    /// Request path
    pub path: String,
    /// Path parameters
    pub path_params: HashMap<String, String>,
    /// Query parameters
    pub query_params: HashMap<String, String>,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: Option<serde_json::Value>,
    /// Content type
    pub content_type: Option<String>,
}

/// REST response representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestResponse {
    /// HTTP status code
    pub status_code: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Option<serde_json::Value>,
    /// Content type
    pub content_type: Option<String>,
}

impl RestTranslator {
    /// Create a new REST translator
    pub fn new(config: TranslatorConfig) -> Self {
        let mut translator = Self {
            config,
            route_mappings: HashMap::new(),
            service_mappings: HashMap::new(),
        };

        // Initialize default route mappings
        translator.initialize_default_mappings();
        translator
    }

    /// Initialize default route mappings for common REST patterns
    fn initialize_default_mappings(&mut self) {
        // Job management endpoints
        self.add_route_mapping(RouteMapping {
            path_pattern: "/api/v1/jobs".to_string(),
            methods: vec!["POST".to_string()],
            message_type: "JobRequest".to_string(),
            service_name: "rustci".to_string(),
            request_transform: Some(TransformationRule {
                field_mappings: {
                    let mut mappings = HashMap::new();
                    mappings.insert("pipeline".to_string(), "job_spec.pipeline".to_string());
                    mappings.insert("environment".to_string(), "job_spec.environment".to_string());
                    mappings
                },
                transformations: vec![],
                custom_script: None,
            }),
            response_transform: None,
            metadata: HashMap::new(),
        });

        self.add_route_mapping(RouteMapping {
            path_pattern: "/api/v1/jobs/{job_id}".to_string(),
            methods: vec!["GET".to_string()],
            message_type: "JobStatusRequest".to_string(),
            service_name: "rustci".to_string(),
            request_transform: None,
            response_transform: None,
            metadata: HashMap::new(),
        });

        self.add_route_mapping(RouteMapping {
            path_pattern: "/api/v1/jobs/{job_id}".to_string(),
            methods: vec!["DELETE".to_string()],
            message_type: "JobCancel".to_string(),
            service_name: "rustci".to_string(),
            request_transform: None,
            response_transform: None,
            metadata: HashMap::new(),
        });

        // Workspace management endpoints
        self.add_route_mapping(RouteMapping {
            path_pattern: "/api/v1/workspaces".to_string(),
            methods: vec!["GET".to_string(), "POST".to_string()],
            message_type: "WorkspaceRequest".to_string(),
            service_name: "rustci".to_string(),
            request_transform: None,
            response_transform: None,
            metadata: HashMap::new(),
        });

        // Runner management endpoints
        self.add_route_mapping(RouteMapping {
            path_pattern: "/api/v1/runners".to_string(),
            methods: vec!["GET".to_string()],
            message_type: "RunnerListRequest".to_string(),
            service_name: "rustci".to_string(),
            request_transform: None,
            response_transform: None,
            metadata: HashMap::new(),
        });

        // Default service mapping
        self.add_service_mapping(ServiceMapping {
            name: "rustci".to_string(),
            version: Some("1.0".to_string()),
            endpoints: vec!["rustci-service".to_string()],
            load_balancing: "round_robin".to_string(),
            tags: {
                let mut tags = HashMap::new();
                tags.insert("type".to_string(), "ci-cd".to_string());
                tags
            },
        });
    }

    /// Add a route mapping
    pub fn add_route_mapping(&mut self, mapping: RouteMapping) {
        let key = format!("{}:{}", mapping.path_pattern, mapping.methods.join(","));
        self.route_mappings.insert(key, mapping);
    }

    /// Add a service mapping
    pub fn add_service_mapping(&mut self, mapping: ServiceMapping) {
        self.service_mappings.insert(mapping.name.clone(), mapping);
    }

    /// Translate REST request to Valkyrie message
    pub async fn rest_to_valkyrie(
        &self,
        context: &HttpRequestContext,
        rest_request: RestRequest,
    ) -> BridgeResult<ValkyrieMessage> {
        debug!("Translating REST request to Valkyrie message: {} {}", 
               rest_request.method, rest_request.path);

        // Find matching route mapping
        let route_mapping = self.find_route_mapping(&rest_request)?;
        
        // Extract path parameters
        let path_params = self.extract_path_parameters(&route_mapping.path_pattern, &rest_request.path)?;
        
        // Determine message type
        let message_type = self.parse_message_type(&route_mapping.message_type)?;
        
        // Create routing information
        let routing = self.create_routing_info(&route_mapping, &rest_request)?;
        
        // Transform request body if needed
        let transformed_body = if let Some(transform) = &route_mapping.request_transform {
            self.apply_request_transformation(transform, &rest_request).await?
        } else {
            rest_request.body
        };

        // Create message header
        let header = MessageHeader {
            protocol_info: ProtocolInfo::default(),
            message_type,
            stream_id: StreamId::new(),
            flags: MessageFlags::default(),
            priority: MessagePriority::Normal,
            timestamp: chrono::Utc::now(),
            ttl: Some(std::time::Duration::from_secs(30)),
            correlation_id: Some(context.request_id),
            routing,
        };

        // Create message payload
        let payload_data = serde_json::json!({
            "rest_request": {
                "method": rest_request.method,
                "path": rest_request.path,
                "path_params": path_params,
                "query_params": rest_request.query_params,
                "headers": rest_request.headers,
                "body": transformed_body,
                "content_type": rest_request.content_type
            },
            "metadata": route_mapping.metadata
        });

        let payload = MessagePayload::Json(payload_data);

        Ok(ValkyrieMessage {
            header,
            payload,
            signature: None,
            trace_context: None,
        })
    }

    /// Translate Valkyrie message to REST response
    pub async fn valkyrie_to_rest(
        &self,
        message: ValkyrieMessage,
    ) -> BridgeResult<RestResponse> {
        debug!("Translating Valkyrie message to REST response");

        // Extract response data from message payload
        let response_data = match message.payload {
            MessagePayload::Json(value) => value,
            MessagePayload::Text(text) => serde_json::json!({ "message": text }),
            MessagePayload::Binary(data) => serde_json::json!({ 
                "data": base64ct::Base64::encode_string(data),
                "encoding": "base64"
            }),
            MessagePayload::Empty => serde_json::json!({}),
        };

        // Determine HTTP status code from message type
        let status_code = match message.header.message_type {
            MessageType::JobComplete => 200,
            MessageType::JobAccept => 202,
            MessageType::JobReject => 400,
            MessageType::JobFailed => 500,
            MessageType::Error => 500,
            MessageType::AuthFailure => 401,
            _ => 200,
        };

        // Create response headers
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("X-Valkyrie-Message-Type".to_string(), format!("{:?}", message.header.message_type));
        
        if let Some(correlation_id) = message.header.correlation_id {
            headers.insert("X-Correlation-ID".to_string(), correlation_id.to_string());
        }

        // Apply response transformation if configured
        let transformed_body = self.apply_response_transformation(&response_data).await?;

        Ok(RestResponse {
            status_code,
            headers,
            body: Some(transformed_body),
            content_type: Some("application/json".to_string()),
        })
    }

    /// Find matching route mapping for a REST request
    fn find_route_mapping(&self, request: &RestRequest) -> BridgeResult<&RouteMapping> {
        // Try exact path match first
        let exact_key = format!("{}:{}", request.path, request.method);
        if let Some(mapping) = self.route_mappings.get(&exact_key) {
            return Ok(mapping);
        }

        // Try pattern matching
        for (key, mapping) in &self.route_mappings {
            if mapping.methods.contains(&request.method) {
                if self.path_matches_pattern(&mapping.path_pattern, &request.path) {
                    return Ok(mapping);
                }
            }
        }

        // If auto-discovery is enabled, create a default mapping
        if self.config.auto_route_discovery {
            warn!("No route mapping found for {} {}, using default", request.method, request.path);
            // Return a reference to a default mapping (this is a simplified approach)
            // In a real implementation, you might want to cache these default mappings
            return Err(BridgeError::InvalidRequestFormat {
                details: "Route mapping not found and auto-discovery not implemented".to_string(),
            });
        }

        Err(BridgeError::InvalidRequestFormat {
            details: format!("No route mapping found for {} {}", request.method, request.path),
        })
    }

    /// Check if a path matches a pattern (supports {param} placeholders)
    fn path_matches_pattern(&self, pattern: &str, path: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return false;
        }

        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if pattern_part.starts_with('{') && pattern_part.ends_with('}') {
                // This is a parameter, it matches any value
                continue;
            } else if pattern_part != path_part {
                return false;
            }
        }

        true
    }

    /// Extract path parameters from a path using a pattern
    fn extract_path_parameters(
        &self,
        pattern: &str,
        path: &str,
    ) -> BridgeResult<HashMap<String, String>> {
        let mut params = HashMap::new();
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return Ok(params);
        }

        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if pattern_part.starts_with('{') && pattern_part.ends_with('}') {
                let param_name = &pattern_part[1..pattern_part.len()-1];
                params.insert(param_name.to_string(), path_part.to_string());
            }
        }

        Ok(params)
    }

    /// Parse message type string to MessageType enum
    fn parse_message_type(&self, message_type_str: &str) -> BridgeResult<MessageType> {
        match message_type_str {
            "JobRequest" => Ok(MessageType::JobRequest),
            "JobStatusRequest" => Ok(MessageType::JobRequest), // Map to generic job request
            "JobCancel" => Ok(MessageType::JobCancel),
            "WorkspaceRequest" => Ok(MessageType::JobRequest), // Map to generic request
            "RunnerListRequest" => Ok(MessageType::JobRequest), // Map to generic request
            _ => {
                warn!("Unknown message type: {}, using JobRequest", message_type_str);
                Ok(MessageType::JobRequest)
            }
        }
    }

    /// Create routing information for the message
    fn create_routing_info(
        &self,
        route_mapping: &RouteMapping,
        request: &RestRequest,
    ) -> BridgeResult<RoutingInfo> {
        let service_mapping = self.service_mappings.get(&route_mapping.service_name)
            .ok_or_else(|| BridgeError::InvalidRequestFormat {
                details: format!("Service mapping not found: {}", route_mapping.service_name),
            })?;

        let load_balancing = match service_mapping.load_balancing.as_str() {
            "round_robin" => LoadBalancingStrategy::RoundRobin,
            "least_connections" => LoadBalancingStrategy::LeastConnections,
            "weighted" => LoadBalancingStrategy::Weighted,
            "consistent_hash" => LoadBalancingStrategy::ConsistentHash,
            _ => LoadBalancingStrategy::RoundRobin,
        };

        Ok(RoutingInfo {
            source: EndpointId::new("rest_gateway"),
            destination: DestinationType::Service(ServiceSelector {
                service_name: service_mapping.name.clone(),
                version: service_mapping.version.clone(),
                tags: service_mapping.tags.clone(),
            }),
            hints: RoutingHints::default(),
            load_balancing,
        })
    }

    /// Apply request transformation rules
    async fn apply_request_transformation(
        &self,
        transform: &TransformationRule,
        request: &RestRequest,
    ) -> BridgeResult<Option<serde_json::Value>> {
        if !self.config.enable_transformations {
            return Ok(request.body.clone());
        }

        let mut result = request.body.clone().unwrap_or(serde_json::json!({}));

        // Apply field mappings
        for (source, target) in &transform.field_mappings {
            if let Some(value) = self.get_nested_value(&result, source) {
                self.set_nested_value(&mut result, target, value)?;
            }
        }

        // Apply field transformations
        for transformation in &transform.transformations {
            self.apply_field_transformation(&mut result, transformation).await?;
        }

        Ok(Some(result))
    }

    /// Apply response transformation
    async fn apply_response_transformation(
        &self,
        response_data: &serde_json::Value,
    ) -> BridgeResult<serde_json::Value> {
        // For now, return the response data as-is
        // In a full implementation, this would apply configured transformations
        Ok(response_data.clone())
    }

    /// Apply a single field transformation
    async fn apply_field_transformation(
        &self,
        data: &mut serde_json::Value,
        transformation: &FieldTransformation,
    ) -> BridgeResult<()> {
        let source_value = self.get_nested_value(data, &transformation.source_field);
        
        if let Some(value) = source_value {
            let transformed_value = match &transformation.transform_type {
                TransformationType::Copy => value,
                TransformationType::Format(format_str) => {
                    // Simple string formatting
                    serde_json::Value::String(format_str.replace("{}", &value.to_string()))
                }
                TransformationType::Convert(target_type) => {
                    self.convert_value_type(value, target_type)?
                }
                TransformationType::Conditional { condition, true_value, false_value } => {
                    if self.evaluate_condition(condition, &value) {
                        true_value.clone()
                    } else {
                        false_value.clone()
                    }
                }
                TransformationType::Custom(_function_name) => {
                    // Custom transformation would be implemented here
                    value
                }
            };

            self.set_nested_value(data, &transformation.target_field, transformed_value)?;
        }

        Ok(())
    }

    /// Get nested value from JSON using dot notation
    fn get_nested_value(&self, data: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = data;

        for part in parts {
            match current {
                serde_json::Value::Object(map) => {
                    current = map.get(part)?;
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }

    /// Set nested value in JSON using dot notation
    fn set_nested_value(
        &self,
        data: &mut serde_json::Value,
        path: &str,
        value: serde_json::Value,
    ) -> BridgeResult<()> {
        let parts: Vec<&str> = path.split('.').collect();
        
        if parts.is_empty() {
            return Err(BridgeError::InvalidRequestFormat {
                details: "Empty path for nested value".to_string(),
            });
        }

        let mut current = data;
        
        // Navigate to the parent of the target field
        for part in &parts[..parts.len()-1] {
            match current {
                serde_json::Value::Object(map) => {
                    current = map.entry(part.to_string())
                        .or_insert_with(|| serde_json::json!({}));
                }
                _ => {
                    return Err(BridgeError::InvalidRequestFormat {
                        details: format!("Cannot set nested value at path: {}", path),
                    });
                }
            }
        }

        // Set the final value
        if let serde_json::Value::Object(map) = current {
            map.insert(parts.last().unwrap().to_string(), value);
        }

        Ok(())
    }

    /// Convert value to target type
    fn convert_value_type(
        &self,
        value: serde_json::Value,
        target_type: &str,
    ) -> BridgeResult<serde_json::Value> {
        match target_type {
            "string" => Ok(serde_json::Value::String(value.to_string())),
            "number" => {
                if let Ok(num) = value.to_string().parse::<f64>() {
                    Ok(serde_json::json!(num))
                } else {
                    Err(BridgeError::InvalidRequestFormat {
                        details: format!("Cannot convert {} to number", value),
                    })
                }
            }
            "boolean" => {
                let bool_val = match value {
                    serde_json::Value::Bool(b) => b,
                    serde_json::Value::String(s) => s.to_lowercase() == "true",
                    serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                    _ => false,
                };
                Ok(serde_json::Value::Bool(bool_val))
            }
            _ => Ok(value), // Unknown type, return as-is
        }
    }

    /// Evaluate a simple condition
    fn evaluate_condition(&self, condition: &str, value: &serde_json::Value) -> bool {
        // Simple condition evaluation (in a real implementation, this would be more sophisticated)
        match condition {
            "not_null" => !value.is_null(),
            "is_string" => value.is_string(),
            "is_number" => value.is_number(),
            "is_boolean" => value.is_boolean(),
            _ => false,
        }
    }
}