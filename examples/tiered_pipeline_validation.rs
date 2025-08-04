use RustAutoDevOps::ci::config::{CIPipeline, PipelineType, ServerConfig, SimpleStep};
use RustAutoDevOps::config::validation_engine::TieredPipelineValidator;

fn main() {
    println!("🔍 Tiered Pipeline Validation Examples");
    println!("=====================================\n");

    let validator = TieredPipelineValidator::new();

    // Example 1: Minimal Pipeline
    println!("1. Minimal Pipeline Validation:");
    let minimal_yaml = r#"
name: "Deploy App"
repo: https://github.com/user/app.git
server: user@deploy.example.com
branch: main
"#;
    
    match CIPipeline::from_yaml(minimal_yaml) {
        Ok(pipeline) => {
            println!("   Detected type: {:?}", pipeline.get_pipeline_type());
            match validator.validate(&pipeline) {
                Ok(report) => {
                    println!("   ✅ Validation: {}", if report.is_valid() { "PASSED" } else { "FAILED" });
                    if !report.errors.is_empty() {
                        println!("   Errors: {:?}", report.errors);
                    }
                    if !report.warnings.is_empty() {
                        println!("   Warnings: {:?}", report.warnings);
                    }
                }
                Err(e) => println!("   ❌ Validation error: {}", e),
            }
        }
        Err(e) => println!("   ❌ Parse error: {}", e),
    }

    // Example 2: Simple Pipeline
    println!("\n2. Simple Pipeline Validation:");
    let simple_yaml = r#"
name: "Build and Test"
repo: https://github.com/user/app.git
steps:
  - run: cargo build --release
  - run: cargo test
  - name: "Deploy"
    run: docker build -t app:latest .
"#;
    
    match CIPipeline::from_yaml(simple_yaml) {
        Ok(pipeline) => {
            println!("   Detected type: {:?}", pipeline.get_pipeline_type());
            match validator.validate(&pipeline) {
                Ok(report) => {
                    println!("   ✅ Validation: {}", if report.is_valid() { "PASSED" } else { "FAILED" });
                    if !report.errors.is_empty() {
                        println!("   Errors: {:?}", report.errors);
                    }
                    if !report.warnings.is_empty() {
                        println!("   Warnings: {:?}", report.warnings);
                    }
                }
                Err(e) => println!("   ❌ Validation error: {}", e),
            }
        }
        Err(e) => println!("   ❌ Parse error: {}", e),
    }

    // Example 3: Advanced Pipeline
    println!("\n3. Advanced Pipeline Validation:");
    let advanced_yaml = r#"
name: "Advanced CI/CD"
repo: https://github.com/user/app.git
variables:
  RUST_VERSION: "1.70"
  BUILD_ENV: "production"
jobs:
  build:
    script: 
      - cargo build --release
      - cargo test
  deploy:
    script: docker build -t app:latest .
matrix:
  rust: ["1.70", "1.71"]
  os: ["ubuntu", "alpine"]
cache:
  paths: ["target/", "~/.cargo/"]
  key: "rust-cache"
"#;
    
    match CIPipeline::from_yaml(advanced_yaml) {
        Ok(pipeline) => {
            println!("   Detected type: {:?}", pipeline.get_pipeline_type());
            match validator.validate(&pipeline) {
                Ok(report) => {
                    println!("   ✅ Validation: {}", if report.is_valid() { "PASSED" } else { "FAILED" });
                    if !report.errors.is_empty() {
                        println!("   Errors: {:?}", report.errors);
                    }
                    if !report.warnings.is_empty() {
                        println!("   Warnings: {:?}", report.warnings);
                    }
                }
                Err(e) => println!("   ❌ Validation error: {}", e),
            }
        }
        Err(e) => println!("   ❌ Parse error: {}", e),
    }

    // Example 4: Invalid Pipeline (missing required fields)
    println!("\n4. Invalid Pipeline Validation:");
    let invalid_yaml = r#"
name: "Invalid Pipeline"
type: minimal
# Missing required repo and server fields
"#;
    
    match CIPipeline::from_yaml(invalid_yaml) {
        Ok(pipeline) => {
            println!("   Detected type: {:?}", pipeline.get_pipeline_type());
            match validator.validate(&pipeline) {
                Ok(report) => {
                    println!("   ✅ Validation: {}", if report.is_valid() { "PASSED" } else { "FAILED" });
                    if !report.errors.is_empty() {
                        println!("   Errors: {:?}", report.errors);
                    }
                    if !report.warnings.is_empty() {
                        println!("   Warnings: {:?}", report.warnings);
                    }
                }
                Err(e) => println!("   ❌ Validation error: {}", e),
            }
        }
        Err(e) => println!("   ❌ Parse error: {}", e),
    }

    println!("\n🎉 Tiered Pipeline Validation Examples Complete!");
}