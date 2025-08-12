//! C Language Bindings for Valkyrie Protocol
//!
//! This module provides C-compatible FFI (Foreign Function Interface) bindings
//! for the Valkyrie Protocol, enabling integration with C/C++ applications.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::api::valkyrie::{ValkyrieClient, ClientConfig, ClientMessage, ClientMessageType};
use crate::error::Result;

/// Opaque handle to a Valkyrie client
#[repr(C)]
pub struct ValkyrieClientHandle {
    client: Arc<ValkyrieClient>,
    runtime: Arc<Runtime>,
}

/// C-compatible error codes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkyrieErrorCode {
    Success = 0,
    InvalidArgument = 1,
    ConnectionFailed = 2,
    AuthenticationFailed = 3,
    NetworkError = 4,
    InternalError = 5,
    NotFound = 6,
    PermissionDenied = 7,
    Timeout = 8,
    Unknown = 99,
}

/// C-compatible message structure
#[repr(C)]
pub struct ValkyrieMessageC {
    pub message_type: c_int,
    pub priority: c_int,
    pub data: *const c_char,
    pub data_len: c_uint,
    pub correlation_id: *const c_char,
    pub ttl_seconds: c_uint,
}

/// C-compatible client configuration
#[repr(C)]
pub struct ValkyrieConfigC {
    pub enable_encryption: c_int,
    pub enable_metrics: c_int,
    pub connect_timeout_seconds: c_uint,
    pub send_timeout_seconds: c_uint,
    pub max_connections: c_uint,
}

/// C-compatible statistics structure
#[repr(C)]
pub struct ValkyrieStatsC {
    pub active_connections: c_uint,
    pub messages_sent: c_uint,
    pub messages_received: c_uint,
    pub bytes_sent: c_uint,
    pub bytes_received: c_uint,
}

/// Message callback function type for C applications
pub type ValkyrieMessageCallback = extern "C" fn(
    connection_id: *const c_char,
    message: *const ValkyrieMessageC,
    user_data: *mut c_void,
);

/// Create a new Valkyrie client with default configuration
/// 
/// # Safety
/// 
/// The returned handle must be freed with `valkyrie_client_destroy`
#[no_mangle]
pub extern "C" fn valkyrie_client_create() -> *mut ValkyrieClientHandle {
    valkyrie_client_create_with_config(ptr::null())
}

/// Create a new Valkyrie client with custom configuration
/// 
/// # Safety
/// 
/// - `config` can be null for default configuration
/// - The returned handle must be freed with `valkyrie_client_destroy`
#[no_mangle]
pub extern "C" fn valkyrie_client_create_with_config(
    config: *const ValkyrieConfigC,
) -> *mut ValkyrieClientHandle {
    let runtime = match Runtime::new() {
        Ok(rt) => Arc::new(rt),
        Err(_) => return ptr::null_mut(),
    };

    let client_config = if config.is_null() {
        ClientConfig::default()
    } else {
        unsafe { c_config_to_rust_config(&*config) }
    };

    let client = match runtime.block_on(ValkyrieClient::new(client_config)) {
        Ok(client) => Arc::new(client),
        Err(_) => return ptr::null_mut(),
    };

    Box::into_raw(Box::new(ValkyrieClientHandle { client, runtime }))
}

/// Destroy a Valkyrie client and free its resources
/// 
/// # Safety
/// 
/// - `handle` must be a valid handle returned by `valkyrie_client_create*`
/// - `handle` must not be used after this call
#[no_mangle]
pub extern "C" fn valkyrie_client_destroy(handle: *mut ValkyrieClientHandle) {
    if !handle.is_null() {
        unsafe {
            let handle = Box::from_raw(handle);
            // The runtime and client will be dropped automatically
            drop(handle);
        }
    }
}

/// Connect to a remote endpoint
/// 
/// # Safety
/// 
/// - `handle` must be a valid client handle
/// - `endpoint_url` must be a valid null-terminated C string
/// - `connection_id_out` must point to valid memory for a c_char pointer
#[no_mangle]
pub extern "C" fn valkyrie_client_connect(
    handle: *mut ValkyrieClientHandle,
    endpoint_url: *const c_char,
    connection_id_out: *mut *mut c_char,
) -> ValkyrieErrorCode {
    if handle.is_null() || endpoint_url.is_null() || connection_id_out.is_null() {
        return ValkyrieErrorCode::InvalidArgument;
    }

    unsafe {
        let handle = &*handle;
        let url = match CStr::from_ptr(endpoint_url).to_str() {
            Ok(url) => url,
            Err(_) => return ValkyrieErrorCode::InvalidArgument,
        };

        match handle.runtime.block_on(handle.client.connect(url)) {
            Ok(connection_id) => {
                match CString::new(connection_id) {
                    Ok(c_string) => {
                        *connection_id_out = c_string.into_raw();
                        ValkyrieErrorCode::Success
                    }
                    Err(_) => ValkyrieErrorCode::InternalError,
                }
            }
            Err(_) => ValkyrieErrorCode::ConnectionFailed,
        }
    }
}

/// Send a text message to a connection
/// 
/// # Safety
/// 
/// - `handle` must be a valid client handle
/// - `connection_id` must be a valid null-terminated C string
/// - `message` must be a valid null-terminated C string
#[no_mangle]
pub extern "C" fn valkyrie_client_send_text(
    handle: *mut ValkyrieClientHandle,
    connection_id: *const c_char,
    message: *const c_char,
) -> ValkyrieErrorCode {
    if handle.is_null() || connection_id.is_null() || message.is_null() {
        return ValkyrieErrorCode::InvalidArgument;
    }

    unsafe {
        let handle = &*handle;
        let conn_id = match CStr::from_ptr(connection_id).to_str() {
            Ok(id) => id,
            Err(_) => return ValkyrieErrorCode::InvalidArgument,
        };
        let msg = match CStr::from_ptr(message).to_str() {
            Ok(msg) => msg,
            Err(_) => return ValkyrieErrorCode::InvalidArgument,
        };

        match handle.runtime.block_on(handle.client.send_text(conn_id, msg)) {
            Ok(_) => ValkyrieErrorCode::Success,
            Err(_) => ValkyrieErrorCode::NetworkError,
        }
    }
}

/// Send a binary message to a connection
/// 
/// # Safety
/// 
/// - `handle` must be a valid client handle
/// - `connection_id` must be a valid null-terminated C string
/// - `data` must point to valid memory of at least `data_len` bytes
#[no_mangle]
pub extern "C" fn valkyrie_client_send_binary(
    handle: *mut ValkyrieClientHandle,
    connection_id: *const c_char,
    data: *const c_void,
    data_len: c_uint,
) -> ValkyrieErrorCode {
    if handle.is_null() || connection_id.is_null() || data.is_null() {
        return ValkyrieErrorCode::InvalidArgument;
    }

    unsafe {
        let handle = &*handle;
        let conn_id = match CStr::from_ptr(connection_id).to_str() {
            Ok(id) => id,
            Err(_) => return ValkyrieErrorCode::InvalidArgument,
        };
        let data_slice = std::slice::from_raw_parts(data as *const u8, data_len as usize);

        match handle.runtime.block_on(handle.client.send_data(conn_id, data_slice)) {
            Ok(_) => ValkyrieErrorCode::Success,
            Err(_) => ValkyrieErrorCode::NetworkError,
        }
    }
}

/// Send a custom message to a connection
/// 
/// # Safety
/// 
/// - `handle` must be a valid client handle
/// - `connection_id` must be a valid null-terminated C string
/// - `message` must point to a valid ValkyrieMessageC structure
#[no_mangle]
pub extern "C" fn valkyrie_client_send_message(
    handle: *mut ValkyrieClientHandle,
    connection_id: *const c_char,
    message: *const ValkyrieMessageC,
) -> ValkyrieErrorCode {
    if handle.is_null() || connection_id.is_null() || message.is_null() {
        return ValkyrieErrorCode::InvalidArgument;
    }

    unsafe {
        let handle = &*handle;
        let conn_id = match CStr::from_ptr(connection_id).to_str() {
            Ok(id) => id,
            Err(_) => return ValkyrieErrorCode::InvalidArgument,
        };

        let rust_message = match c_message_to_rust_message(&*message) {
            Ok(msg) => msg,
            Err(_) => return ValkyrieErrorCode::InvalidArgument,
        };

        match handle.runtime.block_on(handle.client.send_message(conn_id, rust_message)) {
            Ok(_) => ValkyrieErrorCode::Success,
            Err(_) => ValkyrieErrorCode::NetworkError,
        }
    }
}

/// Get client statistics
/// 
/// # Safety
/// 
/// - `handle` must be a valid client handle
/// - `stats_out` must point to valid memory for a ValkyrieStatsC structure
#[no_mangle]
pub extern "C" fn valkyrie_client_get_stats(
    handle: *mut ValkyrieClientHandle,
    stats_out: *mut ValkyrieStatsC,
) -> ValkyrieErrorCode {
    if handle.is_null() || stats_out.is_null() {
        return ValkyrieErrorCode::InvalidArgument;
    }

    unsafe {
        let handle = &*handle;
        match handle.runtime.block_on(handle.client.get_stats()) {
            stats => {
                *stats_out = ValkyrieStatsC {
                    active_connections: stats.active_connections as c_uint,
                    messages_sent: stats.engine_stats.transport.messages_sent as c_uint,
                    messages_received: stats.engine_stats.transport.messages_received as c_uint,
                    bytes_sent: stats.engine_stats.transport.bytes_sent as c_uint,
                    bytes_received: stats.engine_stats.transport.bytes_received as c_uint,
                };
                ValkyrieErrorCode::Success
            }
        }
    }
}

/// Close a specific connection
/// 
/// # Safety
/// 
/// - `handle` must be a valid client handle
/// - `connection_id` must be a valid null-terminated C string
#[no_mangle]
pub extern "C" fn valkyrie_client_close_connection(
    handle: *mut ValkyrieClientHandle,
    connection_id: *const c_char,
) -> ValkyrieErrorCode {
    if handle.is_null() || connection_id.is_null() {
        return ValkyrieErrorCode::InvalidArgument;
    }

    unsafe {
        let handle = &*handle;
        let conn_id = match CStr::from_ptr(connection_id).to_str() {
            Ok(id) => id,
            Err(_) => return ValkyrieErrorCode::InvalidArgument,
        };

        match handle.runtime.block_on(handle.client.close_connection(conn_id)) {
            Ok(_) => ValkyrieErrorCode::Success,
            Err(_) => ValkyrieErrorCode::NetworkError,
        }
    }
}

/// Free a connection ID string returned by valkyrie_client_connect
/// 
/// # Safety
/// 
/// - `connection_id` must be a string returned by `valkyrie_client_connect`
/// - `connection_id` must not be used after this call
#[no_mangle]
pub extern "C" fn valkyrie_free_connection_id(connection_id: *mut c_char) {
    if !connection_id.is_null() {
        unsafe {
            let _ = CString::from_raw(connection_id);
        }
    }
}

/// Convert C configuration to Rust configuration
unsafe fn c_config_to_rust_config(c_config: &ValkyrieConfigC) -> ClientConfig {
    use crate::api::valkyrie::*;
    use std::time::Duration;

    ClientConfig {
        security: ClientSecurityConfig {
            enable_encryption: c_config.enable_encryption != 0,
            ..Default::default()
        },
        performance: ClientPerformanceConfig {
            max_connections_per_endpoint: c_config.max_connections,
            ..Default::default()
        },
        timeouts: ClientTimeoutConfig {
            connect_timeout: Duration::from_secs(c_config.connect_timeout_seconds as u64),
            send_timeout: Duration::from_secs(c_config.send_timeout_seconds as u64),
            ..Default::default()
        },
        features: ClientFeatureFlags {
            enable_metrics: c_config.enable_metrics != 0,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Convert C message to Rust message
unsafe fn c_message_to_rust_message(c_message: &ValkyrieMessageC) -> Result<ClientMessage> {
    use crate::api::valkyrie::*;
    use std::time::Duration;

    let message_type = match c_message.message_type {
        0 => ClientMessageType::Text,
        1 => ClientMessageType::Binary,
        2 => ClientMessageType::Json,
        3 => ClientMessageType::Control,
        _ => return Err(crate::error::AppError::ValidationError("Invalid message type".to_string())),
    };

    let priority = match c_message.priority {
        0 => ClientMessagePriority::Low,
        1 => ClientMessagePriority::Normal,
        2 => ClientMessagePriority::High,
        3 => ClientMessagePriority::Critical,
        _ => ClientMessagePriority::Normal,
    };

    let payload = if c_message.data.is_null() {
        ClientPayload::Binary(vec![])
    } else {
        let data_slice = std::slice::from_raw_parts(
            c_message.data as *const u8,
            c_message.data_len as usize,
        );
        match message_type {
            ClientMessageType::Text | ClientMessageType::Json => {
                let text = std::str::from_utf8(data_slice)
                    .map_err(|e| crate::error::AppError::ValidationError(format!("Invalid UTF-8: {}", e)))?;
                ClientPayload::Text(text.to_string())
            }
            ClientMessageType::Binary | ClientMessageType::Control => {
                ClientPayload::Binary(data_slice.to_vec())
            }
        }
    };

    let correlation_id = if c_message.correlation_id.is_null() {
        None
    } else {
        Some(CStr::from_ptr(c_message.correlation_id).to_str()
            .map_err(|e| crate::error::AppError::ValidationError(format!("Invalid correlation ID: {}", e)))?
            .to_string())
    };

    let ttl = if c_message.ttl_seconds > 0 {
        Some(Duration::from_secs(c_message.ttl_seconds as u64))
    } else {
        None
    };

    Ok(ClientMessage {
        message_type,
        priority,
        payload,
        correlation_id,
        ttl,
        metadata: std::collections::HashMap::new(),
    })
}

/// Generate C header file content
pub fn generate_c_header() -> String {
    r#"
#ifndef VALKYRIE_PROTOCOL_H
#define VALKYRIE_PROTOCOL_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

/* Opaque handle to a Valkyrie client */
typedef struct ValkyrieClientHandle ValkyrieClientHandle;

/* Error codes */
typedef enum {
    VALKYRIE_SUCCESS = 0,
    VALKYRIE_INVALID_ARGUMENT = 1,
    VALKYRIE_CONNECTION_FAILED = 2,
    VALKYRIE_AUTHENTICATION_FAILED = 3,
    VALKYRIE_NETWORK_ERROR = 4,
    VALKYRIE_INTERNAL_ERROR = 5,
    VALKYRIE_NOT_FOUND = 6,
    VALKYRIE_PERMISSION_DENIED = 7,
    VALKYRIE_TIMEOUT = 8,
    VALKYRIE_UNKNOWN = 99
} ValkyrieErrorCode;

/* Message structure */
typedef struct {
    int message_type;
    int priority;
    const char* data;
    unsigned int data_len;
    const char* correlation_id;
    unsigned int ttl_seconds;
} ValkyrieMessageC;

/* Client configuration */
typedef struct {
    int enable_encryption;
    int enable_metrics;
    unsigned int connect_timeout_seconds;
    unsigned int send_timeout_seconds;
    unsigned int max_connections;
} ValkyrieConfigC;

/* Statistics structure */
typedef struct {
    unsigned int active_connections;
    unsigned int messages_sent;
    unsigned int messages_received;
    unsigned int bytes_sent;
    unsigned int bytes_received;
} ValkyrieStatsC;

/* Message callback function type */
typedef void (*ValkyrieMessageCallback)(
    const char* connection_id,
    const ValkyrieMessageC* message,
    void* user_data
);

/* Client management functions */
ValkyrieClientHandle* valkyrie_client_create(void);
ValkyrieClientHandle* valkyrie_client_create_with_config(const ValkyrieConfigC* config);
void valkyrie_client_destroy(ValkyrieClientHandle* handle);

/* Connection management functions */
ValkyrieErrorCode valkyrie_client_connect(
    ValkyrieClientHandle* handle,
    const char* endpoint_url,
    char** connection_id_out
);
ValkyrieErrorCode valkyrie_client_close_connection(
    ValkyrieClientHandle* handle,
    const char* connection_id
);

/* Message sending functions */
ValkyrieErrorCode valkyrie_client_send_text(
    ValkyrieClientHandle* handle,
    const char* connection_id,
    const char* message
);
ValkyrieErrorCode valkyrie_client_send_binary(
    ValkyrieClientHandle* handle,
    const char* connection_id,
    const void* data,
    unsigned int data_len
);
ValkyrieErrorCode valkyrie_client_send_message(
    ValkyrieClientHandle* handle,
    const char* connection_id,
    const ValkyrieMessageC* message
);

/* Statistics and monitoring functions */
ValkyrieErrorCode valkyrie_client_get_stats(
    ValkyrieClientHandle* handle,
    ValkyrieStatsC* stats_out
);

/* Memory management functions */
void valkyrie_free_connection_id(char* connection_id);

#ifdef __cplusplus
}
#endif

#endif /* VALKYRIE_PROTOCOL_H */
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    #[test]
    fn test_c_config_conversion() {
        let c_config = ValkyrieConfigC {
            enable_encryption: 1,
            enable_metrics: 1,
            connect_timeout_seconds: 10,
            send_timeout_seconds: 5,
            max_connections: 50,
        };

        let rust_config = unsafe { c_config_to_rust_config(&c_config) };
        assert!(rust_config.security.enable_encryption);
        assert!(rust_config.features.enable_metrics);
        assert_eq!(rust_config.performance.max_connections_per_endpoint, 50);
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(ValkyrieErrorCode::Success as i32, 0);
        assert_eq!(ValkyrieErrorCode::InvalidArgument as i32, 1);
        assert_eq!(ValkyrieErrorCode::ConnectionFailed as i32, 2);
    }
}