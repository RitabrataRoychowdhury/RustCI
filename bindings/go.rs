//! Go Language Bindings for Valkyrie Protocol
//!
//! This module provides Go bindings using CGO, enabling Go applications
//! to use the Valkyrie Protocol through a C-compatible interface.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::api::valkyrie::{ClientConfig, ValkyrieClient};
use crate::bindings::c::{
    ValkyrieClientHandle, ValkyrieConfigC, ValkyrieErrorCode, ValkyrieMessageC, ValkyrieStatsC,
};

// Re-export C bindings for Go compatibility
pub use crate::bindings::c::*;

/// Generate Go package content
pub fn generate_go_package() -> String {
    r#"
// Package valkyrie provides Go bindings for the Valkyrie Protocol
package valkyrie

/*
#cgo LDFLAGS: -lvalkyrie_protocol
#include "valkyrie_protocol.h"
#include <stdlib.h>
*/
import "C"
import (
    "errors"
    "runtime"
    "time"
    "unsafe"
)

// ErrorCode represents Valkyrie Protocol error codes
type ErrorCode int

const (
    Success ErrorCode = iota
    InvalidArgument
    ConnectionFailed
    AuthenticationFailed
    NetworkError
    InternalError
    NotFound
    PermissionDenied
    Timeout
    Unknown = 99
)

// Error returns the string representation of an error code
func (e ErrorCode) Error() string {
    switch e {
    case Success:
        return "success"
    case InvalidArgument:
        return "invalid argument"
    case ConnectionFailed:
        return "connection failed"
    case AuthenticationFailed:
        return "authentication failed"
    case NetworkError:
        return "network error"
    case InternalError:
        return "internal error"
    case NotFound:
        return "not found"
    case PermissionDenied:
        return "permission denied"
    case Timeout:
        return "timeout"
    default:
        return "unknown error"
    }
}

// Config represents client configuration
type Config struct {
    EnableEncryption      bool
    EnableMetrics         bool
    ConnectTimeoutSeconds uint32
    SendTimeoutSeconds    uint32
    MaxConnections        uint32
}

// DefaultConfig returns a default configuration
func DefaultConfig() *Config {
    return &Config{
        EnableEncryption:      true,
        EnableMetrics:         true,
        ConnectTimeoutSeconds: 10,
        SendTimeoutSeconds:    5,
        MaxConnections:        10,
    }
}

// Message represents a Valkyrie Protocol message
type Message struct {
    Type          MessageType
    Priority      MessagePriority
    Data          []byte
    CorrelationID string
    TTL           time.Duration
}

// MessageType represents the type of a message
type MessageType int

const (
    TextMessage MessageType = iota
    BinaryMessage
    JSONMessage
    ControlMessage
)

// MessagePriority represents the priority of a message
type MessagePriority int

const (
    LowPriority MessagePriority = iota
    NormalPriority
    HighPriority
    CriticalPriority
)

// Stats represents client statistics
type Stats struct {
    ActiveConnections uint32
    MessagesSent      uint32
    MessagesReceived  uint32
    BytesSent         uint32
    BytesReceived     uint32
}

// BroadcastResult represents the result of a broadcast operation
type BroadcastResult struct {
    Total      uint32
    Successful uint32
    Failed     uint32
}

// Client represents a Valkyrie Protocol client
type Client struct {
    handle *C.ValkyrieClientHandle
}

// NewClient creates a new Valkyrie Protocol client with default configuration
func NewClient() (*Client, error) {
    return NewClientWithConfig(DefaultConfig())
}

// NewClientWithConfig creates a new Valkyrie Protocol client with custom configuration
func NewClientWithConfig(config *Config) (*Client, error) {
    var cConfig C.ValkyrieConfigC
    if config != nil {
        cConfig.enable_encryption = boolToInt(config.EnableEncryption)
        cConfig.enable_metrics = boolToInt(config.EnableMetrics)
        cConfig.connect_timeout_seconds = C.uint(config.ConnectTimeoutSeconds)
        cConfig.send_timeout_seconds = C.uint(config.SendTimeoutSeconds)
        cConfig.max_connections = C.uint(config.MaxConnections)
    }

    handle := C.valkyrie_client_create_with_config(&cConfig)
    if handle == nil {
        return nil, errors.New("failed to create Valkyrie client")
    }

    client := &Client{handle: handle}
    runtime.SetFinalizer(client, (*Client).Close)
    return client, nil
}

// SimpleClient creates a simple client with default settings
func SimpleClient() (*Client, error) {
    return NewClient()
}

// SecureClient creates a secure client with mTLS authentication
func SecureClient(certPath, keyPath, caPath string) (*Client, error) {
    config := &Config{
        EnableEncryption:      true,
        EnableMetrics:         true,
        ConnectTimeoutSeconds: 10,
        SendTimeoutSeconds:    5,
        MaxConnections:        10,
    }
    
    // Note: In a full implementation, we'd need to extend the C API
    // to support certificate paths
    return NewClientWithConfig(config)
}

// HighPerformanceClient creates a high-performance client
func HighPerformanceClient() (*Client, error) {
    config := &Config{
        EnableEncryption:      true,
        EnableMetrics:         true,
        ConnectTimeoutSeconds: 5,
        SendTimeoutSeconds:    2,
        MaxConnections:        100,
    }
    return NewClientWithConfig(config)
}

// Connect connects to a remote endpoint
func (c *Client) Connect(endpointURL string) (string, error) {
    if c.handle == nil {
        return "", errors.New("client is closed")
    }

    cURL := C.CString(endpointURL)
    defer C.free(unsafe.Pointer(cURL))

    var connectionID *C.char
    result := C.valkyrie_client_connect(c.handle, cURL, &connectionID)
    
    if result != C.VALKYRIE_SUCCESS {
        return "", ErrorCode(result)
    }

    if connectionID == nil {
        return "", errors.New("received null connection ID")
    }

    goConnectionID := C.GoString(connectionID)
    C.valkyrie_free_connection_id(connectionID)
    
    return goConnectionID, nil
}

// SendText sends a text message to a connection
func (c *Client) SendText(connectionID, text string) error {
    if c.handle == nil {
        return errors.New("client is closed")
    }

    cConnectionID := C.CString(connectionID)
    defer C.free(unsafe.Pointer(cConnectionID))

    cText := C.CString(text)
    defer C.free(unsafe.Pointer(cText))

    result := C.valkyrie_client_send_text(c.handle, cConnectionID, cText)
    if result != C.VALKYRIE_SUCCESS {
        return ErrorCode(result)
    }

    return nil
}

// SendBinary sends binary data to a connection
func (c *Client) SendBinary(connectionID string, data []byte) error {
    if c.handle == nil {
        return errors.New("client is closed")
    }

    cConnectionID := C.CString(connectionID)
    defer C.free(unsafe.Pointer(cConnectionID))

    var dataPtr unsafe.Pointer
    if len(data) > 0 {
        dataPtr = unsafe.Pointer(&data[0])
    }

    result := C.valkyrie_client_send_binary(
        c.handle,
        cConnectionID,
        dataPtr,
        C.uint(len(data)),
    )
    
    if result != C.VALKYRIE_SUCCESS {
        return ErrorCode(result)
    }

    return nil
}

// SendMessage sends a custom message to a connection
func (c *Client) SendMessage(connectionID string, message *Message) error {
    if c.handle == nil {
        return errors.New("client is closed")
    }

    cConnectionID := C.CString(connectionID)
    defer C.free(unsafe.Pointer(cConnectionID))

    cMessage := messageToCMessage(message)
    defer freeCMessage(&cMessage)

    result := C.valkyrie_client_send_message(c.handle, cConnectionID, &cMessage)
    if result != C.VALKYRIE_SUCCESS {
        return ErrorCode(result)
    }

    return nil
}

// GetStats returns client statistics
func (c *Client) GetStats() (*Stats, error) {
    if c.handle == nil {
        return nil, errors.New("client is closed")
    }

    var cStats C.ValkyrieStatsC
    result := C.valkyrie_client_get_stats(c.handle, &cStats)
    if result != C.VALKYRIE_SUCCESS {
        return nil, ErrorCode(result)
    }

    return &Stats{
        ActiveConnections: uint32(cStats.active_connections),
        MessagesSent:      uint32(cStats.messages_sent),
        MessagesReceived:  uint32(cStats.messages_received),
        BytesSent:         uint32(cStats.bytes_sent),
        BytesReceived:     uint32(cStats.bytes_received),
    }, nil
}

// CloseConnection closes a specific connection
func (c *Client) CloseConnection(connectionID string) error {
    if c.handle == nil {
        return errors.New("client is closed")
    }

    cConnectionID := C.CString(connectionID)
    defer C.free(unsafe.Pointer(cConnectionID))

    result := C.valkyrie_client_close_connection(c.handle, cConnectionID)
    if result != C.VALKYRIE_SUCCESS {
        return ErrorCode(result)
    }

    return nil
}

// Close closes the client and frees its resources
func (c *Client) Close() error {
    if c.handle != nil {
        C.valkyrie_client_destroy(c.handle)
        c.handle = nil
        runtime.SetFinalizer(c, nil)
    }
    return nil
}

// Helper functions

func boolToInt(b bool) C.int {
    if b {
        return 1
    }
    return 0
}

func messageToCMessage(msg *Message) C.ValkyrieMessageC {
    var cMessage C.ValkyrieMessageC
    
    cMessage.message_type = C.int(msg.Type)
    cMessage.priority = C.int(msg.Priority)
    
    if len(msg.Data) > 0 {
        cMessage.data = (*C.char)(unsafe.Pointer(&msg.Data[0]))
        cMessage.data_len = C.uint(len(msg.Data))
    }
    
    if msg.CorrelationID != "" {
        cMessage.correlation_id = C.CString(msg.CorrelationID)
    }
    
    if msg.TTL > 0 {
        cMessage.ttl_seconds = C.uint(msg.TTL.Seconds())
    }
    
    return cMessage
}

func freeCMessage(cMessage *C.ValkyrieMessageC) {
    if cMessage.correlation_id != nil {
        C.free(unsafe.Pointer(cMessage.correlation_id))
    }
}

// Message builder functions

// TextMessage creates a text message
func TextMessage(content string) *Message {
    return &Message{
        Type:     TextMessage,
        Priority: NormalPriority,
        Data:     []byte(content),
    }
}

// BinaryMessage creates a binary message
func BinaryMessage(data []byte) *Message {
    return &Message{
        Type:     BinaryMessage,
        Priority: NormalPriority,
        Data:     data,
    }
}

// WithPriority sets the message priority
func (m *Message) WithPriority(priority MessagePriority) *Message {
    m.Priority = priority
    return m
}

// WithCorrelationID sets the correlation ID
func (m *Message) WithCorrelationID(correlationID string) *Message {
    m.CorrelationID = correlationID
    return m
}

// WithTTL sets the time-to-live
func (m *Message) WithTTL(ttl time.Duration) *Message {
    m.TTL = ttl
    return m
}
"#
    .to_string()
}

/// Generate Go module file
pub fn generate_go_mod() -> String {
    r#"
module github.com/rustci/valkyrie-protocol-go

go 1.19

require ()
"#
    .to_string()
}

/// Generate Go test file
pub fn generate_go_test() -> String {
    r#"
package valkyrie

import (
    "testing"
    "time"
)

func TestClientCreation(t *testing.T) {
    client, err := NewClient()
    if err != nil {
        t.Fatalf("Failed to create client: %v", err)
    }
    defer client.Close()

    stats, err := client.GetStats()
    if err != nil {
        t.Fatalf("Failed to get stats: %v", err)
    }

    if stats.ActiveConnections != 0 {
        t.Errorf("Expected 0 active connections, got %d", stats.ActiveConnections)
    }
}

func TestConfigCreation(t *testing.T) {
    config := DefaultConfig()
    if !config.EnableEncryption {
        t.Error("Expected encryption to be enabled by default")
    }
    if !config.EnableMetrics {
        t.Error("Expected metrics to be enabled by default")
    }
}

func TestMessageCreation(t *testing.T) {
    msg := TextMessage("Hello, Go!")
    if msg.Type != TextMessage {
        t.Errorf("Expected TextMessage type, got %v", msg.Type)
    }
    if msg.Priority != NormalPriority {
        t.Errorf("Expected NormalPriority, got %v", msg.Priority)
    }
    if string(msg.Data) != "Hello, Go!" {
        t.Errorf("Expected 'Hello, Go!', got %s", string(msg.Data))
    }
}

func TestMessageBuilder(t *testing.T) {
    msg := TextMessage("Test").
        WithPriority(HighPriority).
        WithCorrelationID("test-123").
        WithTTL(30 * time.Second)

    if msg.Priority != HighPriority {
        t.Errorf("Expected HighPriority, got %v", msg.Priority)
    }
    if msg.CorrelationID != "test-123" {
        t.Errorf("Expected 'test-123', got %s", msg.CorrelationID)
    }
    if msg.TTL != 30*time.Second {
        t.Errorf("Expected 30s TTL, got %v", msg.TTL)
    }
}

func TestSimpleClient(t *testing.T) {
    client, err := SimpleClient()
    if err != nil {
        t.Fatalf("Failed to create simple client: %v", err)
    }
    defer client.Close()
}

func TestHighPerformanceClient(t *testing.T) {
    client, err := HighPerformanceClient()
    if err != nil {
        t.Fatalf("Failed to create high-performance client: %v", err)
    }
    defer client.Close()
}

func BenchmarkMessageCreation(b *testing.B) {
    for i := 0; i < b.N; i++ {
        _ = TextMessage("Benchmark message")
    }
}

func BenchmarkClientCreation(b *testing.B) {
    for i := 0; i < b.N; i++ {
        client, err := NewClient()
        if err != nil {
            b.Fatalf("Failed to create client: %v", err)
        }
        client.Close()
    }
}
"#
    .to_string()
}

/// Generate Go example file
pub fn generate_go_example() -> String {
    r#"
package main

import (
    "fmt"
    "log"
    "time"

    "github.com/rustci/valkyrie-protocol-go"
)

func main() {
    // Create a simple client
    client, err := valkyrie.SimpleClient()
    if err != nil {
        log.Fatalf("Failed to create client: %v", err)
    }
    defer client.Close()

    fmt.Println("Valkyrie Protocol Go Example")

    // Get initial stats
    stats, err := client.GetStats()
    if err != nil {
        log.Fatalf("Failed to get stats: %v", err)
    }
    fmt.Printf("Initial stats: %+v\n", stats)

    // Example of connecting to a server (would fail without a running server)
    fmt.Println("Attempting to connect to tcp://localhost:8080...")
    connectionID, err := client.Connect("tcp://localhost:8080")
    if err != nil {
        fmt.Printf("Connection failed (expected): %v\n", err)
    } else {
        fmt.Printf("Connected with ID: %s\n", connectionID)

        // Send a text message
        err = client.SendText(connectionID, "Hello from Go!")
        if err != nil {
            fmt.Printf("Send failed: %v\n", err)
        } else {
            fmt.Println("Message sent successfully")
        }

        // Send a binary message
        binaryData := []byte{0x01, 0x02, 0x03, 0x04}
        err = client.SendBinary(connectionID, binaryData)
        if err != nil {
            fmt.Printf("Binary send failed: %v\n", err)
        } else {
            fmt.Println("Binary message sent successfully")
        }

        // Send a custom message
        message := valkyrie.TextMessage("Custom message").
            WithPriority(valkyrie.HighPriority).
            WithCorrelationID("example-123").
            WithTTL(30 * time.Second)

        err = client.SendMessage(connectionID, message)
        if err != nil {
            fmt.Printf("Custom message send failed: %v\n", err)
        } else {
            fmt.Println("Custom message sent successfully")
        }

        // Close the connection
        err = client.CloseConnection(connectionID)
        if err != nil {
            fmt.Printf("Close connection failed: %v\n", err)
        } else {
            fmt.Println("Connection closed successfully")
        }
    }

    // Get final stats
    stats, err = client.GetStats()
    if err != nil {
        log.Fatalf("Failed to get final stats: %v", err)
    }
    fmt.Printf("Final stats: %+v\n", stats)

    fmt.Println("Example completed")
}
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_package_generation() {
        let package = generate_go_package();
        assert!(package.contains("package valkyrie"));
        assert!(package.contains("type Client struct"));
        assert!(package.contains("func NewClient()"));
    }

    #[test]
    fn test_go_mod_generation() {
        let mod_file = generate_go_mod();
        assert!(mod_file.contains("module github.com/rustci/valkyrie-protocol-go"));
        assert!(mod_file.contains("go 1.19"));
    }

    #[test]
    fn test_go_test_generation() {
        let test_file = generate_go_test();
        assert!(test_file.contains("func TestClientCreation"));
        assert!(test_file.contains("func BenchmarkMessageCreation"));
    }
}
