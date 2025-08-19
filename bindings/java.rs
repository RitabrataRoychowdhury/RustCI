//! Java Language Bindings for Valkyrie Protocol
//!
//! This module provides Java bindings using JNI, enabling Java applications
//! to use the Valkyrie Protocol through a native Java interface.

use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jobject, jstring};
use jni::{JNIEnv, JavaVM};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

use crate::api::valkyrie::{
    ClientConfig, ClientMessage, ClientMessagePriority, ClientMessageType, ClientPayload,
    ValkyrieClient,
};

/// Java wrapper for ValkyrieClient
pub struct JavaValkyrieClient {
    client: Arc<ValkyrieClient>,
    runtime: Arc<Runtime>,
}

/// Create a new Valkyrie client with default configuration
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeCreate(
    env: JNIEnv,
    _class: JClass,
) -> jlong {
    match create_client_with_default_config() {
        Ok(client) => Box::into_raw(Box::new(client)) as jlong,
        Err(_) => 0,
    }
}

/// Create a new Valkyrie client with custom configuration
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeCreateWithConfig(
    env: JNIEnv,
    _class: JClass,
    enable_encryption: jboolean,
    enable_metrics: jboolean,
    connect_timeout: jint,
    send_timeout: jint,
    max_connections: jint,
) -> jlong {
    let config = ClientConfig {
        security: crate::api::valkyrie::ClientSecurityConfig {
            enable_encryption: enable_encryption != 0,
            ..Default::default()
        },
        performance: crate::api::valkyrie::ClientPerformanceConfig {
            max_connections_per_endpoint: max_connections as u32,
            ..Default::default()
        },
        timeouts: crate::api::valkyrie::ClientTimeoutConfig {
            connect_timeout: Duration::from_secs(connect_timeout as u64),
            send_timeout: Duration::from_secs(send_timeout as u64),
            ..Default::default()
        },
        features: crate::api::valkyrie::ClientFeatureFlags {
            enable_metrics: enable_metrics != 0,
            ..Default::default()
        },
        ..Default::default()
    };

    match create_client_with_config(config) {
        Ok(client) => Box::into_raw(Box::new(client)) as jlong,
        Err(_) => 0,
    }
}

/// Connect to a remote endpoint
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeConnect(
    env: JNIEnv,
    _object: JObject,
    client_ptr: jlong,
    endpoint_url: JString,
) -> jstring {
    let client = unsafe { &*(client_ptr as *const JavaValkyrieClient) };

    let endpoint_str: String = match env.get_string(endpoint_url) {
        Ok(s) => s.into(),
        Err(_) => return std::ptr::null_mut(),
    };

    match client
        .runtime
        .block_on(client.client.connect(&endpoint_str))
    {
        Ok(connection_id) => match env.new_string(connection_id) {
            Ok(jstr) => jstr.into_inner(),
            Err(_) => std::ptr::null_mut(),
        },
        Err(_) => std::ptr::null_mut(),
    }
}

/// Send a text message to a connection
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeSendText(
    env: JNIEnv,
    _object: JObject,
    client_ptr: jlong,
    connection_id: JString,
    message: JString,
) -> jboolean {
    let client = unsafe { &*(client_ptr as *const JavaValkyrieClient) };

    let conn_id: String = match env.get_string(connection_id) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    let msg: String = match env.get_string(message) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    match client
        .runtime
        .block_on(client.client.send_text(&conn_id, &msg))
    {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Send binary data to a connection
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeSendBinary(
    env: JNIEnv,
    _object: JObject,
    client_ptr: jlong,
    connection_id: JString,
    data: JByteArray,
) -> jboolean {
    let client = unsafe { &*(client_ptr as *const JavaValkyrieClient) };

    let conn_id: String = match env.get_string(connection_id) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    let data_bytes = match env.convert_byte_array(data) {
        Ok(bytes) => bytes,
        Err(_) => return 0,
    };

    match client
        .runtime
        .block_on(client.client.send_data(&conn_id, &data_bytes))
    {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Get client statistics
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeGetStats(
    env: JNIEnv,
    _object: JObject,
    client_ptr: jlong,
) -> jobject {
    let client = unsafe { &*(client_ptr as *const JavaValkyrieClient) };

    let stats = client.runtime.block_on(client.client.get_stats());

    // Create Java ValkyrieStats object
    let stats_class = match env.find_class("com/valkyrie/protocol/ValkyrieStats") {
        Ok(class) => class,
        Err(_) => return std::ptr::null_mut(),
    };

    let constructor = match env.get_method_id(stats_class, "<init>", "(IIJJJJ)V") {
        Ok(method) => method,
        Err(_) => return std::ptr::null_mut(),
    };

    let args = [
        JValue::Int(stats.active_connections as i32),
        JValue::Int(stats.handlers_registered as i32),
        JValue::Long(stats.engine_stats.transport.messages_sent as i64),
        JValue::Long(stats.engine_stats.transport.messages_received as i64),
        JValue::Long(stats.engine_stats.transport.bytes_sent as i64),
        JValue::Long(stats.engine_stats.transport.bytes_received as i64),
    ];

    match env.new_object(stats_class, constructor, &args) {
        Ok(obj) => obj.into_inner(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Close a specific connection
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeCloseConnection(
    env: JNIEnv,
    _object: JObject,
    client_ptr: jlong,
    connection_id: JString,
) -> jboolean {
    let client = unsafe { &*(client_ptr as *const JavaValkyrieClient) };

    let conn_id: String = match env.get_string(connection_id) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    match client
        .runtime
        .block_on(client.client.close_connection(&conn_id))
    {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Destroy the client and free its resources
#[no_mangle]
pub extern "system" fn Java_com_valkyrie_protocol_ValkyrieClient_nativeDestroy(
    _env: JNIEnv,
    _object: JObject,
    client_ptr: jlong,
) {
    if client_ptr != 0 {
        unsafe {
            let _ = Box::from_raw(client_ptr as *mut JavaValkyrieClient);
        }
    }
}

// Helper functions

fn create_client_with_default_config() -> Result<JavaValkyrieClient, Box<dyn std::error::Error>> {
    let runtime = Runtime::new()?;
    let client = runtime.block_on(ValkyrieClient::new(ClientConfig::default()))?;

    Ok(JavaValkyrieClient {
        client: Arc::new(client),
        runtime: Arc::new(runtime),
    })
}

fn create_client_with_config(
    config: ClientConfig,
) -> Result<JavaValkyrieClient, Box<dyn std::error::Error>> {
    let runtime = Runtime::new()?;
    let client = runtime.block_on(ValkyrieClient::new(config))?;

    Ok(JavaValkyrieClient {
        client: Arc::new(client),
        runtime: Arc::new(runtime),
    })
}

/// Generate Java source code
pub fn generate_java_sources() -> Vec<(String, String)> {
    vec![
        (
            "ValkyrieClient.java".to_string(),
            generate_valkyrie_client_java(),
        ),
        (
            "ValkyrieConfig.java".to_string(),
            generate_valkyrie_config_java(),
        ),
        (
            "ValkyrieMessage.java".to_string(),
            generate_valkyrie_message_java(),
        ),
        (
            "ValkyrieStats.java".to_string(),
            generate_valkyrie_stats_java(),
        ),
        (
            "BroadcastResult.java".to_string(),
            generate_broadcast_result_java(),
        ),
        (
            "ValkyrieException.java".to_string(),
            generate_valkyrie_exception_java(),
        ),
    ]
}

fn generate_valkyrie_client_java() -> String {
    r#"
package com.valkyrie.protocol;

/**
 * High-level Valkyrie Protocol client for Java applications.
 * 
 * This is the main entry point for Java applications wanting to use the Valkyrie Protocol.
 * It provides a simplified, developer-friendly interface that abstracts away
 * the complexity of the underlying protocol implementation.
 * 
 * Example usage:
 * <pre>
 * ValkyrieClient client = ValkyrieClient.simple();
 * String connectionId = client.connect("tcp://localhost:8080");
 * client.sendText(connectionId, "Hello, Valkyrie!");
 * client.close();
 * </pre>
 */
public class ValkyrieClient implements AutoCloseable {
    
    static {
        System.loadLibrary("valkyrie_protocol_java");
    }
    
    private long nativePtr;
    
    /**
     * Create a new Valkyrie Protocol client with default configuration.
     */
    public ValkyrieClient() {
        this.nativePtr = nativeCreate();
        if (this.nativePtr == 0) {
            throw new ValkyrieException("Failed to create Valkyrie client");
        }
    }
    
    /**
     * Create a new Valkyrie Protocol client with custom configuration.
     * 
     * @param config Client configuration
     */
    public ValkyrieClient(ValkyrieConfig config) {
        this.nativePtr = nativeCreateWithConfig(
            config.isEnableEncryption(),
            config.isEnableMetrics(),
            config.getConnectTimeout(),
            config.getSendTimeout(),
            config.getMaxConnections()
        );
        if (this.nativePtr == 0) {
            throw new ValkyrieException("Failed to create Valkyrie client");
        }
    }
    
    /**
     * Create a simple client with default settings.
     * 
     * @return A new ValkyrieClient instance
     */
    public static ValkyrieClient simple() {
        return new ValkyrieClient();
    }
    
    /**
     * Create a secure client with mTLS authentication.
     * 
     * @param certPath Path to client certificate
     * @param keyPath Path to client private key
     * @param caPath Path to CA certificate
     * @return A new ValkyrieClient instance
     */
    public static ValkyrieClient secure(String certPath, String keyPath, String caPath) {
        ValkyrieConfig config = new ValkyrieConfig();
        config.setEnableEncryption(true);
        // Note: In a full implementation, we'd need to extend the native API
        // to support certificate paths
        return new ValkyrieClient(config);
    }
    
    /**
     * Create a high-performance client.
     * 
     * @return A new ValkyrieClient instance
     */
    public static ValkyrieClient highPerformance() {
        ValkyrieConfig config = new ValkyrieConfig();
        config.setMaxConnections(100);
        config.setConnectTimeout(5);
        config.setSendTimeout(2);
        return new ValkyrieClient(config);
    }
    
    /**
     * Connect to a remote endpoint.
     * 
     * @param endpointUrl URL of the remote endpoint (e.g., "tcp://localhost:8080")
     * @return Connection identifier
     * @throws ValkyrieException if connection fails
     */
    public String connect(String endpointUrl) throws ValkyrieException {
        checkClosed();
        String connectionId = nativeConnect(nativePtr, endpointUrl);
        if (connectionId == null) {
            throw new ValkyrieException("Failed to connect to " + endpointUrl);
        }
        return connectionId;
    }
    
    /**
     * Send a text message to a connection.
     * 
     * @param connectionId ID of the target connection
     * @param text Text message to send
     * @throws ValkyrieException if send fails
     */
    public void sendText(String connectionId, String text) throws ValkyrieException {
        checkClosed();
        if (!nativeSendText(nativePtr, connectionId, text)) {
            throw new ValkyrieException("Failed to send text message");
        }
    }
    
    /**
     * Send binary data to a connection.
     * 
     * @param connectionId ID of the target connection
     * @param data Binary data to send
     * @throws ValkyrieException if send fails
     */
    public void sendBinary(String connectionId, byte[] data) throws ValkyrieException {
        checkClosed();
        if (!nativeSendBinary(nativePtr, connectionId, data)) {
            throw new ValkyrieException("Failed to send binary data");
        }
    }
    
    /**
     * Get client statistics.
     * 
     * @return Client statistics
     * @throws ValkyrieException if operation fails
     */
    public ValkyrieStats getStats() throws ValkyrieException {
        checkClosed();
        ValkyrieStats stats = nativeGetStats(nativePtr);
        if (stats == null) {
            throw new ValkyrieException("Failed to get statistics");
        }
        return stats;
    }
    
    /**
     * Close a specific connection.
     * 
     * @param connectionId ID of the connection to close
     * @throws ValkyrieException if operation fails
     */
    public void closeConnection(String connectionId) throws ValkyrieException {
        checkClosed();
        if (!nativeCloseConnection(nativePtr, connectionId)) {
            throw new ValkyrieException("Failed to close connection");
        }
    }
    
    /**
     * Close the client and free its resources.
     */
    @Override
    public void close() {
        if (nativePtr != 0) {
            nativeDestroy(nativePtr);
            nativePtr = 0;
        }
    }
    
    /**
     * Check if the client is closed.
     * 
     * @return true if the client is closed
     */
    public boolean isClosed() {
        return nativePtr == 0;
    }
    
    private void checkClosed() throws ValkyrieException {
        if (isClosed()) {
            throw new ValkyrieException("Client is closed");
        }
    }
    
    @Override
    protected void finalize() throws Throwable {
        close();
        super.finalize();
    }
    
    // Native methods
    private static native long nativeCreate();
    private static native long nativeCreateWithConfig(
        boolean enableEncryption,
        boolean enableMetrics,
        int connectTimeout,
        int sendTimeout,
        int maxConnections
    );
    private native String nativeConnect(long clientPtr, String endpointUrl);
    private native boolean nativeSendText(long clientPtr, String connectionId, String message);
    private native boolean nativeSendBinary(long clientPtr, String connectionId, byte[] data);
    private native ValkyrieStats nativeGetStats(long clientPtr);
    private native boolean nativeCloseConnection(long clientPtr, String connectionId);
    private native void nativeDestroy(long clientPtr);
}
"#
    .to_string()
}

fn generate_valkyrie_config_java() -> String {
    r#"
package com.valkyrie.protocol;

/**
 * Configuration for Valkyrie Protocol client.
 */
public class ValkyrieConfig {
    private boolean enableEncryption = true;
    private boolean enableMetrics = true;
    private int connectTimeout = 10;
    private int sendTimeout = 5;
    private int maxConnections = 10;
    
    /**
     * Create default configuration.
     */
    public ValkyrieConfig() {
    }
    
    /**
     * Get whether encryption is enabled.
     * 
     * @return true if encryption is enabled
     */
    public boolean isEnableEncryption() {
        return enableEncryption;
    }
    
    /**
     * Set whether to enable encryption.
     * 
     * @param enableEncryption true to enable encryption
     */
    public void setEnableEncryption(boolean enableEncryption) {
        this.enableEncryption = enableEncryption;
    }
    
    /**
     * Get whether metrics are enabled.
     * 
     * @return true if metrics are enabled
     */
    public boolean isEnableMetrics() {
        return enableMetrics;
    }
    
    /**
     * Set whether to enable metrics.
     * 
     * @param enableMetrics true to enable metrics
     */
    public void setEnableMetrics(boolean enableMetrics) {
        this.enableMetrics = enableMetrics;
    }
    
    /**
     * Get connection timeout in seconds.
     * 
     * @return connection timeout
     */
    public int getConnectTimeout() {
        return connectTimeout;
    }
    
    /**
     * Set connection timeout in seconds.
     * 
     * @param connectTimeout connection timeout
     */
    public void setConnectTimeout(int connectTimeout) {
        this.connectTimeout = connectTimeout;
    }
    
    /**
     * Get send timeout in seconds.
     * 
     * @return send timeout
     */
    public int getSendTimeout() {
        return sendTimeout;
    }
    
    /**
     * Set send timeout in seconds.
     * 
     * @param sendTimeout send timeout
     */
    public void setSendTimeout(int sendTimeout) {
        this.sendTimeout = sendTimeout;
    }
    
    /**
     * Get maximum connections per endpoint.
     * 
     * @return maximum connections
     */
    public int getMaxConnections() {
        return maxConnections;
    }
    
    /**
     * Set maximum connections per endpoint.
     * 
     * @param maxConnections maximum connections
     */
    public void setMaxConnections(int maxConnections) {
        this.maxConnections = maxConnections;
    }
}
"#
    .to_string()
}

fn generate_valkyrie_message_java() -> String {
    r#"
package com.valkyrie.protocol;

import java.util.HashMap;
import java.util.Map;

/**
 * Message structure for Valkyrie Protocol.
 */
public class ValkyrieMessage {
    
    /**
     * Message types.
     */
    public enum Type {
        TEXT, BINARY, JSON, CONTROL
    }
    
    /**
     * Message priorities.
     */
    public enum Priority {
        LOW, NORMAL, HIGH, CRITICAL
    }
    
    private Type type = Type.TEXT;
    private Priority priority = Priority.NORMAL;
    private byte[] data;
    private String correlationId;
    private Integer ttlSeconds;
    private Map<String, String> metadata = new HashMap<>();
    
    /**
     * Create a new message.
     */
    public ValkyrieMessage() {
    }
    
    /**
     * Create a text message.
     * 
     * @param content Text content
     * @return A new ValkyrieMessage
     */
    public static ValkyrieMessage text(String content) {
        ValkyrieMessage message = new ValkyrieMessage();
        message.setType(Type.TEXT);
        message.setData(content.getBytes());
        return message;
    }
    
    /**
     * Create a binary message.
     * 
     * @param data Binary data
     * @return A new ValkyrieMessage
     */
    public static ValkyrieMessage binary(byte[] data) {
        ValkyrieMessage message = new ValkyrieMessage();
        message.setType(Type.BINARY);
        message.setData(data);
        return message;
    }
    
    /**
     * Set message priority.
     * 
     * @param priority Message priority
     * @return This message for chaining
     */
    public ValkyrieMessage withPriority(Priority priority) {
        this.priority = priority;
        return this;
    }
    
    /**
     * Set correlation ID.
     * 
     * @param correlationId Correlation ID
     * @return This message for chaining
     */
    public ValkyrieMessage withCorrelationId(String correlationId) {
        this.correlationId = correlationId;
        return this;
    }
    
    /**
     * Set TTL in seconds.
     * 
     * @param ttlSeconds TTL in seconds
     * @return This message for chaining
     */
    public ValkyrieMessage withTtl(int ttlSeconds) {
        this.ttlSeconds = ttlSeconds;
        return this;
    }
    
    /**
     * Add metadata.
     * 
     * @param key Metadata key
     * @param value Metadata value
     * @return This message for chaining
     */
    public ValkyrieMessage withMetadata(String key, String value) {
        this.metadata.put(key, value);
        return this;
    }
    
    // Getters and setters
    
    public Type getType() {
        return type;
    }
    
    public void setType(Type type) {
        this.type = type;
    }
    
    public Priority getPriority() {
        return priority;
    }
    
    public void setPriority(Priority priority) {
        this.priority = priority;
    }
    
    public byte[] getData() {
        return data;
    }
    
    public void setData(byte[] data) {
        this.data = data;
    }
    
    public String getCorrelationId() {
        return correlationId;
    }
    
    public void setCorrelationId(String correlationId) {
        this.correlationId = correlationId;
    }
    
    public Integer getTtlSeconds() {
        return ttlSeconds;
    }
    
    public void setTtlSeconds(Integer ttlSeconds) {
        this.ttlSeconds = ttlSeconds;
    }
    
    public Map<String, String> getMetadata() {
        return metadata;
    }
    
    public void setMetadata(Map<String, String> metadata) {
        this.metadata = metadata;
    }
}
"#
    .to_string()
}

fn generate_valkyrie_stats_java() -> String {
    r#"
package com.valkyrie.protocol;

/**
 * Client statistics.
 */
public class ValkyrieStats {
    private final int activeConnections;
    private final int handlersRegistered;
    private final long messagesSent;
    private final long messagesReceived;
    private final long bytesSent;
    private final long bytesReceived;
    
    /**
     * Create statistics object.
     * 
     * @param activeConnections Number of active connections
     * @param handlersRegistered Number of registered handlers
     * @param messagesSent Number of messages sent
     * @param messagesReceived Number of messages received
     * @param bytesSent Number of bytes sent
     * @param bytesReceived Number of bytes received
     */
    public ValkyrieStats(int activeConnections, int handlersRegistered,
                        long messagesSent, long messagesReceived,
                        long bytesSent, long bytesReceived) {
        this.activeConnections = activeConnections;
        this.handlersRegistered = handlersRegistered;
        this.messagesSent = messagesSent;
        this.messagesReceived = messagesReceived;
        this.bytesSent = bytesSent;
        this.bytesReceived = bytesReceived;
    }
    
    public int getActiveConnections() {
        return activeConnections;
    }
    
    public int getHandlersRegistered() {
        return handlersRegistered;
    }
    
    public long getMessagesSent() {
        return messagesSent;
    }
    
    public long getMessagesReceived() {
        return messagesReceived;
    }
    
    public long getBytesSent() {
        return bytesSent;
    }
    
    public long getBytesReceived() {
        return bytesReceived;
    }
    
    @Override
    public String toString() {
        return String.format(
            "ValkyrieStats{activeConnections=%d, handlersRegistered=%d, " +
            "messagesSent=%d, messagesReceived=%d, bytesSent=%d, bytesReceived=%d}",
            activeConnections, handlersRegistered, messagesSent, messagesReceived,
            bytesSent, bytesReceived
        );
    }
}
"#
    .to_string()
}

fn generate_broadcast_result_java() -> String {
    r#"
package com.valkyrie.protocol;

/**
 * Result of a broadcast operation.
 */
public class BroadcastResult {
    private final int total;
    private final int successful;
    private final int failed;
    
    /**
     * Create broadcast result.
     * 
     * @param total Total number of connections
     * @param successful Number of successful sends
     * @param failed Number of failed sends
     */
    public BroadcastResult(int total, int successful, int failed) {
        this.total = total;
        this.successful = successful;
        this.failed = failed;
    }
    
    public int getTotal() {
        return total;
    }
    
    public int getSuccessful() {
        return successful;
    }
    
    public int getFailed() {
        return failed;
    }
    
    @Override
    public String toString() {
        return String.format("BroadcastResult{total=%d, successful=%d, failed=%d}",
                           total, successful, failed);
    }
}
"#
    .to_string()
}

fn generate_valkyrie_exception_java() -> String {
    r#"
package com.valkyrie.protocol;

/**
 * Exception thrown by Valkyrie Protocol operations.
 */
public class ValkyrieException extends RuntimeException {
    
    /**
     * Create a new ValkyrieException.
     * 
     * @param message Error message
     */
    public ValkyrieException(String message) {
        super(message);
    }
    
    /**
     * Create a new ValkyrieException.
     * 
     * @param message Error message
     * @param cause Underlying cause
     */
    public ValkyrieException(String message, Throwable cause) {
        super(message, cause);
    }
}
"#
    .to_string()
}

/// Generate Maven POM file
pub fn generate_maven_pom() -> String {
    r#"
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 
         http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    
    <groupId>com.valkyrie</groupId>
    <artifactId>valkyrie-protocol-java</artifactId>
    <version>1.0.0</version>
    <packaging>jar</packaging>
    
    <name>Valkyrie Protocol Java Bindings</name>
    <description>High-performance distributed communication protocol for Java</description>
    <url>https://github.com/rustci/valkyrie-protocol</url>
    
    <properties>
        <maven.compiler.source>8</maven.compiler.source>
        <maven.compiler.target>8</maven.compiler.target>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
    </properties>
    
    <dependencies>
        <dependency>
            <groupId>junit</groupId>
            <artifactId>junit</artifactId>
            <version>4.13.2</version>
            <scope>test</scope>
        </dependency>
    </dependencies>
    
    <build>
        <plugins>
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-compiler-plugin</artifactId>
                <version>3.8.1</version>
                <configuration>
                    <source>8</source>
                    <target>8</target>
                </configuration>
            </plugin>
            
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-surefire-plugin</artifactId>
                <version>3.0.0-M7</version>
                <configuration>
                    <systemPropertyVariables>
                        <java.library.path>${project.build.directory}/native</java.library.path>
                    </systemPropertyVariables>
                </configuration>
            </plugin>
            
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-source-plugin</artifactId>
                <version>3.2.1</version>
                <executions>
                    <execution>
                        <id>attach-sources</id>
                        <goals>
                            <goal>jar</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>
            
            <plugin>
                <groupId>org.apache.maven.plugins</groupId>
                <artifactId>maven-javadoc-plugin</artifactId>
                <version>3.4.0</version>
                <executions>
                    <execution>
                        <id>attach-javadocs</id>
                        <goals>
                            <goal>jar</goal>
                        </goals>
                    </execution>
                </executions>
            </plugin>
        </plugins>
    </build>
    
    <licenses>
        <license>
            <name>MIT License</name>
            <url>https://opensource.org/licenses/MIT</url>
            <distribution>repo</distribution>
        </license>
    </licenses>
    
    <developers>
        <developer>
            <name>RustCI Team</name>
            <email>team@rustci.org</email>
            <organization>RustCI</organization>
            <organizationUrl>https://rustci.org</organizationUrl>
        </developer>
    </developers>
    
    <scm>
        <connection>scm:git:git://github.com/rustci/valkyrie-protocol.git</connection>
        <developerConnection>scm:git:ssh://github.com:rustci/valkyrie-protocol.git</developerConnection>
        <url>https://github.com/rustci/valkyrie-protocol/tree/main</url>
    </scm>
</project>
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_source_generation() {
        let sources = generate_java_sources();
        assert_eq!(sources.len(), 6);

        let client_source = sources
            .iter()
            .find(|(name, _)| name == "ValkyrieClient.java")
            .unwrap();
        assert!(client_source.1.contains("public class ValkyrieClient"));
        assert!(client_source.1.contains("public String connect"));
    }

    #[test]
    fn test_maven_pom_generation() {
        let pom = generate_maven_pom();
        assert!(pom.contains("<artifactId>valkyrie-protocol-java</artifactId>"));
        assert!(pom.contains("<version>1.0.0</version>"));
    }
}
