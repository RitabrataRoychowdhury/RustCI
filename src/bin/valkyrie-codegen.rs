//! Valkyrie Protocol Code Generation CLI Tool
//!
//! This tool generates language bindings and documentation for the Valkyrie Protocol.

use std::collections::HashMap;
use std::path::PathBuf;
use clap::{Arg, Command};
use serde_json;

use RustAutoDevOps::api::codegen::{CodeGenerator, CodeGenConfig, Language, extract_api_definition};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("valkyrie-codegen")
        .version("1.0.0")
        .author("RustCI Team")
        .about("Generate language bindings for Valkyrie Protocol")
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("DIR")
                .help("Output directory for generated code")
                .default_value("./generated")
        )
        .arg(
            Arg::new("languages")
                .short('l')
                .long("languages")
                .value_name("LANGS")
                .help("Comma-separated list of languages to generate")
                .default_value("rust,c,python,javascript,go,java")
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file (JSON)")
        )
        .arg(
            Arg::new("template-dir")
                .short('t')
                .long("template-dir")
                .value_name("DIR")
                .help("Template directory for custom templates")
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(clap::ArgAction::SetTrue)
        )
        .get_matches();

    let output_dir = matches.get_one::<String>("output").unwrap().to_string();
    let languages_str = matches.get_one::<String>("languages").unwrap();
    let verbose = matches.get_flag("verbose");

    // Parse languages
    let languages: Result<Vec<Language>, _> = languages_str
        .split(',')
        .map(|lang| match lang.trim().to_lowercase().as_str() {
            "rust" => Ok(Language::Rust),
            "c" => Ok(Language::C),
            "python" => Ok(Language::Python),
            "javascript" | "js" => Ok(Language::JavaScript),
            "go" => Ok(Language::Go),
            "java" => Ok(Language::Java),
            "csharp" | "c#" => Ok(Language::CSharp),
            "swift" => Ok(Language::Swift),
            "kotlin" => Ok(Language::Kotlin),
            _ => Err(format!("Unsupported language: {}", lang)),
        })
        .collect();

    let languages = languages?;

    if verbose {
        println!("Generating bindings for languages: {:?}", languages);
        println!("Output directory: {}", output_dir);
    }

    // Load configuration
    let config = if let Some(config_file) = matches.get_one::<String>("config") {
        let config_content = std::fs::read_to_string(config_file)?;
        let mut config: CodeGenConfig = serde_json::from_str(&config_content)?;
        config.output_dir = output_dir;
        config.languages = languages;
        if let Some(template_dir) = matches.get_one::<String>("template-dir") {
            config.template_dir = Some(template_dir.to_string());
        }
        config
    } else {
        CodeGenConfig {
            output_dir,
            languages,
            template_dir: matches.get_one::<String>("template-dir").map(|s| s.to_string()),
            options: HashMap::new(),
        }
    };

    if verbose {
        println!("Configuration: {:#?}", config);
    }

    // Extract API definition
    if verbose {
        println!("Extracting API definition...");
    }
    let api_def = extract_api_definition();

    // Generate code
    if verbose {
        println!("Generating code...");
    }
    let generator = CodeGenerator::new(config, api_def);
    generator.generate_all()?;

    println!("Code generation completed successfully!");

    // Print summary
    println!("\nCode generation completed for all configured languages.");

    Ok(())
}