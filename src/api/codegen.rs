//! Code Generation Tools for Valkyrie Protocol
//!
//! This module provides tools for generating language bindings, documentation,
//! and other artifacts from the Valkyrie Protocol API definitions.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Code generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGenConfig {
    /// Output directory for generated code
    pub output_dir: String,
    /// Languages to generate bindings for
    pub languages: Vec<Language>,
    /// Template directory
    pub template_dir: Option<String>,
    /// Custom generation options
    pub options: HashMap<String, serde_json::Value>,
}

/// Supported languages for code generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    C,
    Python,
    JavaScript,
    Go,
    Java,
    CSharp,
    Swift,
    Kotlin,
}

impl Language {
    /// Get the file extension for this language
    pub fn extension(&self) -> &'static str {
        match self {
            Language::Rust => "rs",
            Language::C => "h",
            Language::Python => "py",
            Language::JavaScript => "js",
            Language::Go => "go",
            Language::Java => "java",
            Language::CSharp => "cs",
            Language::Swift => "swift",
            Language::Kotlin => "kt",
        }
    }

    /// Get the package/module name for this language
    pub fn package_name(&self) -> &'static str {
        match self {
            Language::Rust => "valkyrie_protocol",
            Language::C => "valkyrie_protocol",
            Language::Python => "valkyrie_protocol",
            Language::JavaScript => "@valkyrie/protocol",
            Language::Go => "github.com/rustci/valkyrie-protocol-go",
            Language::Java => "com.valkyrie.protocol",
            Language::CSharp => "Valkyrie.Protocol",
            Language::Swift => "ValkyrieProtocol",
            Language::Kotlin => "com.valkyrie.protocol",
        }
    }
}

/// API definition for code generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiDefinition {
    /// API version
    pub version: String,
    /// API name
    pub name: String,
    /// API description
    pub description: String,
    /// Data types
    pub types: Vec<TypeDefinition>,
    /// Service definitions
    pub services: Vec<ServiceDefinition>,
    /// Error definitions
    pub errors: Vec<ErrorDefinition>,
}

/// Type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDefinition {
    /// Type name
    pub name: String,
    /// Type description
    pub description: String,
    /// Type kind
    pub kind: TypeKind,
    /// Type fields (for structs/objects)
    pub fields: Vec<FieldDefinition>,
    /// Type variants (for enums)
    pub variants: Vec<VariantDefinition>,
}

/// Type kinds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeKind {
    Struct,
    Enum,
    Union,
    Alias,
    Primitive,
}

/// Field definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// Field name
    pub name: String,
    /// Field description
    pub description: String,
    /// Field type
    pub field_type: String,
    /// Whether field is optional
    pub optional: bool,
    /// Default value
    pub default: Option<serde_json::Value>,
}

/// Variant definition (for enums)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantDefinition {
    /// Variant name
    pub name: String,
    /// Variant description
    pub description: String,
    /// Variant value
    pub value: Option<serde_json::Value>,
    /// Variant fields (for complex enums)
    pub fields: Vec<FieldDefinition>,
}

/// Service definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    /// Service name
    pub name: String,
    /// Service description
    pub description: String,
    /// Service methods
    pub methods: Vec<MethodDefinition>,
}

/// Method definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodDefinition {
    /// Method name
    pub name: String,
    /// Method description
    pub description: String,
    /// Input parameters
    pub parameters: Vec<ParameterDefinition>,
    /// Return type
    pub return_type: String,
    /// Whether method is async
    pub is_async: bool,
    /// Method errors
    pub errors: Vec<String>,
}

/// Parameter definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    /// Parameter name
    pub name: String,
    /// Parameter description
    pub description: String,
    /// Parameter type
    pub param_type: String,
    /// Whether parameter is optional
    pub optional: bool,
    /// Default value
    pub default: Option<serde_json::Value>,
}

/// Error definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDefinition {
    /// Error name
    pub name: String,
    /// Error description
    pub description: String,
    /// Error code
    pub code: i32,
    /// Error fields
    pub fields: Vec<FieldDefinition>,
}

/// Code generator
pub struct CodeGenerator {
    config: CodeGenConfig,
    api_def: ApiDefinition,
}

impl CodeGenerator {
    /// Create a new code generator
    pub fn new(config: CodeGenConfig, api_def: ApiDefinition) -> Self {
        Self { config, api_def }
    }

    /// Generate code for all configured languages
    pub fn generate_all(&self) -> Result<()> {
        for language in &self.config.languages {
            self.generate_language(*language)?;
        }
        Ok(())
    }

    /// Generate code for a specific language
    pub fn generate_language(&self, language: Language) -> Result<()> {
        match language {
            Language::Rust => self.generate_rust(),
            Language::C => self.generate_c(),
            Language::Python => self.generate_python(),
            Language::JavaScript => self.generate_javascript(),
            Language::Go => self.generate_go(),
            Language::Java => self.generate_java(),
            Language::CSharp => self.generate_csharp(),
            Language::Swift => self.generate_swift(),
            Language::Kotlin => self.generate_kotlin(),
        }
    }

    /// Generate Rust bindings
    fn generate_rust(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("rust");
        fs::create_dir_all(&output_dir)?;

        // Generate lib.rs
        let lib_content = self.generate_rust_lib()?;
        fs::write(output_dir.join("lib.rs"), lib_content)?;

        // Generate types
        let types_content = self.generate_rust_types()?;
        fs::write(output_dir.join("types.rs"), types_content)?;

        // Generate client
        let client_content = self.generate_rust_client()?;
        fs::write(output_dir.join("client.rs"), client_content)?;

        // Generate Cargo.toml
        let cargo_content = self.generate_cargo_toml()?;
        fs::write(output_dir.join("Cargo.toml"), cargo_content)?;

        Ok(())
    }

    /// Generate C bindings
    fn generate_c(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("c");
        fs::create_dir_all(&output_dir)?;

        // Use existing C bindings generation (if available)
        #[cfg(any(feature = "python-bindings", feature = "javascript-bindings", feature = "java-bindings"))]
        {
            let header_content = crate::bindings::c::generate_c_header();
            fs::write(output_dir.join("valkyrie_protocol.h"), header_content)?;
        }
        #[cfg(not(any(feature = "python-bindings", feature = "javascript-bindings", feature = "java-bindings")))]
        {
            // Generate basic C header without bindings module
            let header_content = "// Valkyrie Protocol C Header\n// Generated without language bindings\n";
            fs::write(output_dir.join("valkyrie_protocol.h"), header_content)?;
        }

        Ok(())
    }

    /// Generate Python bindings
    fn generate_python(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("python");
        fs::create_dir_all(&output_dir)?;

        // Use existing Python bindings generation (if available)
        #[cfg(feature = "python-bindings")]
        {
            let stubs_content = crate::bindings::python::generate_python_stubs();
            fs::write(output_dir.join("valkyrie_protocol.pyi"), stubs_content)?;
        }
        #[cfg(not(feature = "python-bindings"))]
        {
            let stubs_content = "# Valkyrie Protocol Python Stubs\n# Generated without Python bindings\n";
            fs::write(output_dir.join("valkyrie_protocol.pyi"), stubs_content)?;
        }

        // Generate setup.py
        let setup_content = self.generate_python_setup()?;
        fs::write(output_dir.join("setup.py"), setup_content)?;

        Ok(())
    }

    /// Generate JavaScript bindings
    fn generate_javascript(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("javascript");
        fs::create_dir_all(&output_dir)?;

        // Use existing JavaScript bindings generation (if available)
        #[cfg(feature = "javascript-bindings")]
        {
            let ts_defs = crate::bindings::javascript::generate_typescript_definitions();
            fs::write(output_dir.join("index.d.ts"), ts_defs)?;

            let package_json = crate::bindings::javascript::generate_package_json();
            fs::write(output_dir.join("package.json"), package_json)?;
        }
        #[cfg(not(feature = "javascript-bindings"))]
        {
            let ts_defs = "// Valkyrie Protocol TypeScript Definitions\n// Generated without JavaScript bindings\n";
            fs::write(output_dir.join("index.d.ts"), ts_defs)?;
            
            let package_json = r#"{"name": "valkyrie-protocol", "version": "1.0.0"}"#;
            fs::write(output_dir.join("package.json"), package_json)?;
        }

        Ok(())
    }

    /// Generate Go bindings
    fn generate_go(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("go");
        fs::create_dir_all(&output_dir)?;

        // Use existing Go bindings generation (if available)
        #[cfg(feature = "go-bindings")]
        {
            let go_package = crate::bindings::go::generate_go_package();
            fs::write(output_dir.join("valkyrie.go"), go_package)?;

            let go_mod = crate::bindings::go::generate_go_mod();
            fs::write(output_dir.join("go.mod"), go_mod)?;

            let go_test = crate::bindings::go::generate_go_test();
            fs::write(output_dir.join("valkyrie_test.go"), go_test)?;
        }
        #[cfg(not(feature = "go-bindings"))]
        {
            let go_package = "// Valkyrie Protocol Go Package\n// Generated without Go bindings\n";
            fs::write(output_dir.join("valkyrie.go"), go_package)?;
            
            let go_mod = "module valkyrie-protocol\n\ngo 1.19\n";
            fs::write(output_dir.join("go.mod"), go_mod)?;
            
            let go_test = "// Valkyrie Protocol Go Tests\n// Generated without Go bindings\n";
            fs::write(output_dir.join("valkyrie_test.go"), go_test)?;
        }

        Ok(())
    }

    /// Generate Java bindings
    fn generate_java(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("java");
        fs::create_dir_all(&output_dir.join("src/main/java/com/valkyrie/protocol"))?;

        // Use existing Java bindings generation (if available)
        #[cfg(feature = "java-bindings")]
        {
            let java_sources = crate::bindings::java::generate_java_sources();
            for (filename, content) in java_sources {
                let file_path = output_dir.join("src/main/java/com/valkyrie/protocol").join(filename);
                fs::write(file_path, content)?;
            }

            let pom_xml = crate::bindings::java::generate_maven_pom();
            fs::write(output_dir.join("pom.xml"), pom_xml)?;
        }
        #[cfg(not(feature = "java-bindings"))]
        {
            let java_content = "// Valkyrie Protocol Java Sources\n// Generated without Java bindings\n";
            fs::write(output_dir.join("src/main/java/com/valkyrie/protocol/ValkyrieProtocol.java"), java_content)?;
            
            let pom_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.valkyrie</groupId>
    <artifactId>valkyrie-protocol</artifactId>
    <version>1.0.0</version>
</project>"#;
            fs::write(output_dir.join("pom.xml"), pom_xml)?;
        }

        Ok(())
    }

    /// Generate C# bindings
    fn generate_csharp(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("csharp");
        fs::create_dir_all(&output_dir)?;

        let csharp_content = self.generate_csharp_client()?;
        fs::write(output_dir.join("ValkyrieClient.cs"), csharp_content)?;

        let csproj_content = self.generate_csproj()?;
        fs::write(output_dir.join("Valkyrie.Protocol.csproj"), csproj_content)?;

        Ok(())
    }

    /// Generate Swift bindings
    fn generate_swift(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("swift");
        fs::create_dir_all(&output_dir)?;

        let swift_content = self.generate_swift_client()?;
        fs::write(output_dir.join("ValkyrieClient.swift"), swift_content)?;

        let package_swift = self.generate_swift_package()?;
        fs::write(output_dir.join("Package.swift"), package_swift)?;

        Ok(())
    }

    /// Generate Kotlin bindings
    fn generate_kotlin(&self) -> Result<()> {
        let output_dir = Path::new(&self.config.output_dir).join("kotlin");
        fs::create_dir_all(&output_dir.join("src/main/kotlin/com/valkyrie/protocol"))?;

        let kotlin_content = self.generate_kotlin_client()?;
        fs::write(
            output_dir.join("src/main/kotlin/com/valkyrie/protocol/ValkyrieClient.kt"),
            kotlin_content
        )?;

        let build_gradle = self.generate_gradle_build()?;
        fs::write(output_dir.join("build.gradle.kts"), build_gradle)?;

        Ok(())
    }

    // Helper methods for generating specific language content

    fn generate_rust_lib(&self) -> Result<String> {
        Ok(format!(
            r#"//! {} - {}
//!
//! Version: {}

pub mod types;
pub mod client;

pub use types::*;
pub use client::*;
"#,
            self.api_def.name, self.api_def.description, self.api_def.version
        ))
    }

    fn generate_rust_types(&self) -> Result<String> {
        let mut content = String::new();
        content.push_str("//! Type definitions for Valkyrie Protocol\n\n");
        content.push_str("use serde::{Deserialize, Serialize};\n\n");

        for type_def in &self.api_def.types {
            match type_def.kind {
                TypeKind::Struct => {
                    content.push_str(&format!(
                        "/// {}\n#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct {} {{\n",
                        type_def.description, type_def.name
                    ));
                    for field in &type_def.fields {
                        let field_type = if field.optional {
                            format!("Option<{}>", field.field_type)
                        } else {
                            field.field_type.clone()
                        };
                        content.push_str(&format!(
                            "    /// {}\n    pub {}: {},\n",
                            field.description, field.name, field_type
                        ));
                    }
                    content.push_str("}\n\n");
                }
                TypeKind::Enum => {
                    content.push_str(&format!(
                        "/// {}\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\npub enum {} {{\n",
                        type_def.description, type_def.name
                    ));
                    for variant in &type_def.variants {
                        content.push_str(&format!(
                            "    /// {}\n    {},\n",
                            variant.description, variant.name
                        ));
                    }
                    content.push_str("}\n\n");
                }
                _ => {} // Handle other type kinds as needed
            }
        }

        Ok(content)
    }

    fn generate_rust_client(&self) -> Result<String> {
        let mut content = String::new();
        content.push_str("//! Client implementation for Valkyrie Protocol\n\n");
        content.push_str("use crate::types::*;\n");
        content.push_str("use async_trait::async_trait;\n\n");

        for service in &self.api_def.services {
            content.push_str(&format!(
                "/// {}\n#[async_trait]\npub trait {} {{\n",
                service.description, service.name
            ));
            for method in &service.methods {
                let params: Vec<String> = method.parameters.iter()
                    .map(|p| format!("{}: {}", p.name, p.param_type))
                    .collect();
                let async_keyword = if method.is_async { "async " } else { "" };
                content.push_str(&format!(
                    "    /// {}\n    {}fn {}(&self, {}) -> Result<{}>;\n",
                    method.description, async_keyword, method.name,
                    params.join(", "), method.return_type
                ));
            }
            content.push_str("}\n\n");
        }

        Ok(content)
    }

    fn generate_cargo_toml(&self) -> Result<String> {
        Ok(format!(
            r#"[package]
name = "{}"
version = "{}"
edition = "2021"
description = "{}"

[dependencies]
serde = {{ version = "1.0", features = ["derive"] }}
async-trait = "0.1"
tokio = {{ version = "1.0", features = ["full"] }}
"#,
            self.api_def.name.to_lowercase().replace(' ', "_"),
            self.api_def.version,
            self.api_def.description
        ))
    }

    fn generate_python_setup(&self) -> Result<String> {
        Ok(format!(
            r#"from setuptools import setup, find_packages

setup(
    name="{}",
    version="{}",
    description="{}",
    packages=find_packages(),
    install_requires=[
        "pyo3>=0.18.0",
    ],
    python_requires=">=3.7",
    classifiers=[
        "Development Status :: 4 - Beta",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.7",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
    ],
)
"#,
            self.api_def.name.to_lowercase().replace(' ', "_"),
            self.api_def.version,
            self.api_def.description
        ))
    }

    fn generate_csharp_client(&self) -> Result<String> {
        Ok(format!(
            r#"using System;
using System.Runtime.InteropServices;

namespace Valkyrie.Protocol
{{
    /// <summary>
    /// {}
    /// </summary>
    public class ValkyrieClient : IDisposable
    {{
        private IntPtr nativePtr;
        
        public ValkyrieClient()
        {{
            nativePtr = NativeMethods.CreateClient();
            if (nativePtr == IntPtr.Zero)
                throw new ValkyrieException("Failed to create client");
        }}
        
        public string Connect(string endpointUrl)
        {{
            var result = NativeMethods.Connect(nativePtr, endpointUrl);
            if (result == null)
                throw new ValkyrieException("Connection failed");
            return result;
        }}
        
        public void SendText(string connectionId, string text)
        {{
            if (!NativeMethods.SendText(nativePtr, connectionId, text))
                throw new ValkyrieException("Send failed");
        }}
        
        public void Dispose()
        {{
            if (nativePtr != IntPtr.Zero)
            {{
                NativeMethods.DestroyClient(nativePtr);
                nativePtr = IntPtr.Zero;
            }}
        }}
    }}
    
    internal static class NativeMethods
    {{
        [DllImport("valkyrie_protocol")]
        internal static extern IntPtr CreateClient();
        
        [DllImport("valkyrie_protocol")]
        internal static extern string Connect(IntPtr client, string endpointUrl);
        
        [DllImport("valkyrie_protocol")]
        internal static extern bool SendText(IntPtr client, string connectionId, string text);
        
        [DllImport("valkyrie_protocol")]
        internal static extern void DestroyClient(IntPtr client);
    }}
    
    public class ValkyrieException : Exception
    {{
        public ValkyrieException(string message) : base(message) {{ }}
    }}
}}
"#,
            self.api_def.description
        ))
    }

    fn generate_csproj(&self) -> Result<String> {
        Ok(format!(
            r#"<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net6.0</TargetFramework>
    <Version>{}</Version>
    <Description>{}</Description>
    <PackageId>Valkyrie.Protocol</PackageId>
    <Authors>RustCI Team</Authors>
    <Company>RustCI</Company>
    <PackageLicenseExpression>MIT</PackageLicenseExpression>
  </PropertyGroup>
</Project>
"#,
            self.api_def.version, self.api_def.description
        ))
    }

    fn generate_swift_client(&self) -> Result<String> {
        Ok(format!(
            r#"import Foundation

/// {}
public class ValkyrieClient {{
    private var nativePtr: OpaquePointer?
    
    public init() throws {{
        nativePtr = valkyrie_client_create()
        guard nativePtr != nil else {{
            throw ValkyrieError.clientCreationFailed
        }}
    }}
    
    deinit {{
        if let ptr = nativePtr {{
            valkyrie_client_destroy(ptr)
        }}
    }}
    
    public func connect(endpointUrl: String) throws -> String {{
        guard let ptr = nativePtr else {{
            throw ValkyrieError.clientClosed
        }}
        
        var connectionId: UnsafeMutablePointer<CChar>?
        let result = valkyrie_client_connect(ptr, endpointUrl, &connectionId)
        
        guard result == VALKYRIE_SUCCESS, let connId = connectionId else {{
            throw ValkyrieError.connectionFailed
        }}
        
        let connectionString = String(cString: connId)
        valkyrie_free_connection_id(connId)
        return connectionString
    }}
    
    public func sendText(connectionId: String, text: String) throws {{
        guard let ptr = nativePtr else {{
            throw ValkyrieError.clientClosed
        }}
        
        let result = valkyrie_client_send_text(ptr, connectionId, text)
        guard result == VALKYRIE_SUCCESS else {{
            throw ValkyrieError.sendFailed
        }}
    }}
}}

public enum ValkyrieError: Error {{
    case clientCreationFailed
    case clientClosed
    case connectionFailed
    case sendFailed
}}

// C function declarations
@_silgen_name("valkyrie_client_create")
func valkyrie_client_create() -> OpaquePointer?

@_silgen_name("valkyrie_client_destroy")
func valkyrie_client_destroy(_ client: OpaquePointer)

@_silgen_name("valkyrie_client_connect")
func valkyrie_client_connect(_ client: OpaquePointer, _ endpointUrl: UnsafePointer<CChar>, _ connectionId: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> Int32

@_silgen_name("valkyrie_client_send_text")
func valkyrie_client_send_text(_ client: OpaquePointer, _ connectionId: UnsafePointer<CChar>, _ text: UnsafePointer<CChar>) -> Int32

@_silgen_name("valkyrie_free_connection_id")
func valkyrie_free_connection_id(_ connectionId: UnsafeMutablePointer<CChar>)

let VALKYRIE_SUCCESS: Int32 = 0
"#,
            self.api_def.description
        ))
    }

    fn generate_swift_package(&self) -> Result<String> {
        Ok(format!(
            r#"// swift-tools-version:5.5
import PackageDescription

let package = Package(
    name: "ValkyrieProtocol",
    platforms: [
        .macOS(.v10_15),
        .iOS(.v13),
    ],
    products: [
        .library(
            name: "ValkyrieProtocol",
            targets: ["ValkyrieProtocol"]
        ),
    ],
    targets: [
        .target(
            name: "ValkyrieProtocol",
            dependencies: []
        ),
        .testTarget(
            name: "ValkyrieProtocolTests",
            dependencies: ["ValkyrieProtocol"]
        ),
    ]
)
"#
        ))
    }

    fn generate_kotlin_client(&self) -> Result<String> {
        Ok(format!(
            r#"package com.valkyrie.protocol

/**
 * {}
 */
class ValkyrieClient : AutoCloseable {{
    private var nativePtr: Long = 0
    
    init {{
        System.loadLibrary("valkyrie_protocol_kotlin")
        nativePtr = nativeCreate()
        if (nativePtr == 0L) {{
            throw ValkyrieException("Failed to create client")
        }}
    }}
    
    fun connect(endpointUrl: String): String {{
        checkClosed()
        val connectionId = nativeConnect(nativePtr, endpointUrl)
        return connectionId ?: throw ValkyrieException("Connection failed")
    }}
    
    fun sendText(connectionId: String, text: String) {{
        checkClosed()
        if (!nativeSendText(nativePtr, connectionId, text)) {{
            throw ValkyrieException("Send failed")
        }}
    }}
    
    override fun close() {{
        if (nativePtr != 0L) {{
            nativeDestroy(nativePtr)
            nativePtr = 0
        }}
    }}
    
    private fun checkClosed() {{
        if (nativePtr == 0L) {{
            throw ValkyrieException("Client is closed")
        }}
    }}
    
    // Native methods
    private external fun nativeCreate(): Long
    private external fun nativeConnect(clientPtr: Long, endpointUrl: String): String?
    private external fun nativeSendText(clientPtr: Long, connectionId: String, text: String): Boolean
    private external fun nativeDestroy(clientPtr: Long)
}}

class ValkyrieException(message: String) : RuntimeException(message)
"#,
            self.api_def.description
        ))
    }

    fn generate_gradle_build(&self) -> Result<String> {
        Ok(format!(
            r#"plugins {{
    kotlin("jvm") version "1.8.0"
    `maven-publish`
}}

group = "com.valkyrie"
version = "{}"

repositories {{
    mavenCentral()
}}

dependencies {{
    testImplementation(kotlin("test"))
}}

tasks.test {{
    useJUnitPlatform()
}}

kotlin {{
    jvmToolchain(8)
}}

publishing {{
    publications {{
        create<MavenPublication>("maven") {{
            from(components["java"])
            
            pom {{
                name.set("Valkyrie Protocol Kotlin Bindings")
                description.set("{}")
                url.set("https://github.com/rustci/valkyrie-protocol")
                
                licenses {{
                    license {{
                        name.set("MIT License")
                        url.set("https://opensource.org/licenses/MIT")
                    }}
                }}
                
                developers {{
                    developer {{
                        name.set("RustCI Team")
                        email.set("team@rustci.org")
                    }}
                }}
            }}
        }}
    }}
}}
"#,
            self.api_def.version, self.api_def.description
        ))
    }
}

/// Extract API definition from Valkyrie Protocol
pub fn extract_api_definition() -> ApiDefinition {
    ApiDefinition {
        version: "1.0.0".to_string(),
        name: "Valkyrie Protocol".to_string(),
        description: "High-performance distributed communication protocol".to_string(),
        types: vec![
            TypeDefinition {
                name: "ValkyrieMessage".to_string(),
                description: "Core message structure".to_string(),
                kind: TypeKind::Struct,
                fields: vec![
                    FieldDefinition {
                        name: "message_type".to_string(),
                        description: "Type of the message".to_string(),
                        field_type: "MessageType".to_string(),
                        optional: false,
                        default: None,
                    },
                    FieldDefinition {
                        name: "priority".to_string(),
                        description: "Message priority".to_string(),
                        field_type: "MessagePriority".to_string(),
                        optional: false,
                        default: Some(serde_json::Value::String("Normal".to_string())),
                    },
                    FieldDefinition {
                        name: "payload".to_string(),
                        description: "Message payload".to_string(),
                        field_type: "Vec<u8>".to_string(),
                        optional: false,
                        default: None,
                    },
                ],
                variants: vec![],
            },
            TypeDefinition {
                name: "MessageType".to_string(),
                description: "Message type enumeration".to_string(),
                kind: TypeKind::Enum,
                fields: vec![],
                variants: vec![
                    VariantDefinition {
                        name: "Data".to_string(),
                        description: "Data message".to_string(),
                        value: None,
                        fields: vec![],
                    },
                    VariantDefinition {
                        name: "Control".to_string(),
                        description: "Control message".to_string(),
                        value: None,
                        fields: vec![],
                    },
                ],
            },
        ],
        services: vec![
            ServiceDefinition {
                name: "ValkyrieClient".to_string(),
                description: "Main client interface".to_string(),
                methods: vec![
                    MethodDefinition {
                        name: "connect".to_string(),
                        description: "Connect to a remote endpoint".to_string(),
                        parameters: vec![
                            ParameterDefinition {
                                name: "endpoint_url".to_string(),
                                description: "URL of the endpoint".to_string(),
                                param_type: "String".to_string(),
                                optional: false,
                                default: None,
                            },
                        ],
                        return_type: "String".to_string(),
                        is_async: true,
                        errors: vec!["ConnectionError".to_string()],
                    },
                    MethodDefinition {
                        name: "send_text".to_string(),
                        description: "Send a text message".to_string(),
                        parameters: vec![
                            ParameterDefinition {
                                name: "connection_id".to_string(),
                                description: "Connection identifier".to_string(),
                                param_type: "String".to_string(),
                                optional: false,
                                default: None,
                            },
                            ParameterDefinition {
                                name: "text".to_string(),
                                description: "Text to send".to_string(),
                                param_type: "String".to_string(),
                                optional: false,
                                default: None,
                            },
                        ],
                        return_type: "()".to_string(),
                        is_async: true,
                        errors: vec!["SendError".to_string()],
                    },
                ],
            },
        ],
        errors: vec![
            ErrorDefinition {
                name: "ConnectionError".to_string(),
                description: "Connection-related error".to_string(),
                code: 1001,
                fields: vec![
                    FieldDefinition {
                        name: "endpoint".to_string(),
                        description: "Failed endpoint".to_string(),
                        field_type: "String".to_string(),
                        optional: false,
                        default: None,
                    },
                ],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_api_definition_extraction() {
        let api_def = extract_api_definition();
        assert_eq!(api_def.name, "Valkyrie Protocol");
        assert!(!api_def.types.is_empty());
        assert!(!api_def.services.is_empty());
    }

    #[test]
    fn test_language_properties() {
        assert_eq!(Language::Rust.extension(), "rs");
        assert_eq!(Language::Python.package_name(), "valkyrie_protocol");
        assert_eq!(Language::JavaScript.package_name(), "@valkyrie/protocol");
    }

    #[test]
    fn test_code_generation() {
        let temp_dir = TempDir::new().unwrap();
        let config = CodeGenConfig {
            output_dir: temp_dir.path().to_string_lossy().to_string(),
            languages: vec![Language::Rust],
            template_dir: None,
            options: HashMap::new(),
        };

        let api_def = extract_api_definition();
        let generator = CodeGenerator::new(config, api_def);
        
        assert!(generator.generate_language(Language::Rust).is_ok());
        
        // Check that files were created
        let rust_dir = temp_dir.path().join("rust");
        assert!(rust_dir.join("lib.rs").exists());
        assert!(rust_dir.join("types.rs").exists());
        assert!(rust_dir.join("client.rs").exists());
        assert!(rust_dir.join("Cargo.toml").exists());
    }
}