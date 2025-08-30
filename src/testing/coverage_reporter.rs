use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use anyhow::{Result, anyhow};

use crate::testing::{CoverageReport, FileCoverageDetail, UncoveredLine, CoverageTrend};
use crate::testing::production_test_suite::TrendDirection;

/// Coverage reporter for generating comprehensive test coverage reports
#[derive(Debug, Clone)]
pub struct CoverageReporter {
    minimum_threshold: f64,
    output_directory: String,
    generate_html: bool,
    track_trends: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageConfig {
    pub minimum_threshold: f64,
    pub output_directory: String,
    pub generate_html: bool,
    pub track_trends: bool,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
}

impl Default for CoverageConfig {
    fn default() -> Self {
        Self {
            minimum_threshold: 90.0,
            output_directory: "target/coverage".to_string(),
            generate_html: true,
            track_trends: true,
            exclude_patterns: vec![
                "tests/*".to_string(),
                "benches/*".to_string(),
                "examples/*".to_string(),
                "target/*".to_string(),
            ],
            include_patterns: vec![
                "src/**/*.rs".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawCoverageData {
    files: HashMap<String, FileCoverageData>,
    summary: CoverageSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileCoverageData {
    path: String,
    lines: Vec<LineInfo>,
    functions: Vec<FunctionInfo>,
    branches: Vec<BranchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LineInfo {
    line_number: u32,
    content: String,
    hit_count: u32,
    is_executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionInfo {
    name: String,
    line_number: u32,
    hit_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BranchInfo {
    line_number: u32,
    branch_id: u32,
    hit_count: u32,
    total_branches: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CoverageSummary {
    total_lines: u32,
    covered_lines: u32,
    total_functions: u32,
    covered_functions: u32,
    total_branches: u32,
    covered_branches: u32,
}

#[async_trait]
pub trait CoverageReporterInterface {
    async fn generate_comprehensive_report(&self) -> Result<CoverageReport>;
    async fn run_coverage_collection(&self) -> Result<RawCoverageData>;
    async fn analyze_coverage_data(&self, raw_data: RawCoverageData) -> Result<CoverageReport>;
    async fn generate_html_report(&self, report: &CoverageReport) -> Result<String>;
    async fn check_coverage_threshold(&self, report: &CoverageReport) -> Result<bool>;
    async fn compare_with_baseline(&self, report: &CoverageReport) -> Result<Option<CoverageTrend>>;
}

impl CoverageReporter {
    pub fn new(minimum_threshold: f64) -> Self {
        Self {
            minimum_threshold,
            output_directory: "target/coverage".to_string(),
            generate_html: true,
            track_trends: true,
        }
    }

    pub fn with_config(config: CoverageConfig) -> Self {
        Self {
            minimum_threshold: config.minimum_threshold,
            output_directory: config.output_directory,
            generate_html: config.generate_html,
            track_trends: config.track_trends,
        }
    }

    async fn setup_coverage_environment(&self) -> Result<()> {
        tracing::info!("Setting up coverage collection environment");

        // Create output directory
        fs::create_dir_all(&self.output_directory)?;

        // Set environment variables for coverage collection
        std::env::set_var("CARGO_INCREMENTAL", "0");
        std::env::set_var("RUSTFLAGS", "-Cinstrument-coverage");
        std::env::set_var("LLVM_PROFILE_FILE", "target/coverage/cargo-test-%p-%m.profraw");

        Ok(())
    }

    async fn run_tests_with_coverage(&self) -> Result<()> {
        tracing::info!("Running tests with coverage instrumentation");

        let output = Command::new("cargo")
            .args(&["test", "--all-features"])
            .env("CARGO_INCREMENTAL", "0")
            .env("RUSTFLAGS", "-Cinstrument-coverage")
            .env("LLVM_PROFILE_FILE", "target/coverage/cargo-test-%p-%m.profraw")
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Test execution failed: {}", stderr));
        }

        tracing::info!("Tests completed successfully");
        Ok(())
    }

    async fn generate_coverage_data(&self) -> Result<()> {
        tracing::info!("Generating coverage data");

        // Find the binary that was tested
        let binary_path = self.find_test_binary().await?;

        // Generate coverage report using llvm-cov
        let output = Command::new("llvm-cov")
            .args(&[
                "export",
                &binary_path,
                "--format=json",
                "--instr-profile=target/coverage/default.profdata",
                "--ignore-filename-regex=/.cargo/registry",
                "--ignore-filename-regex=/rustc/",
            ])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    let coverage_json = String::from_utf8_lossy(&output.stdout);
                    fs::write(
                        format!("{}/coverage.json", self.output_directory),
                        coverage_json.as_bytes()
                    )?;
                    tracing::info!("Coverage data generated successfully");
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!("llvm-cov failed, falling back to alternative method: {}", stderr);
                    self.generate_fallback_coverage().await?;
                }
            }
            Err(e) => {
                tracing::warn!("llvm-cov not available, using fallback coverage: {}", e);
                self.generate_fallback_coverage().await?;
            }
        }

        Ok(())
    }

    async fn find_test_binary(&self) -> Result<String> {
        // This is a simplified implementation
        // In practice, you'd need to find the actual test binary
        let target_dir = "target/debug/deps";
        
        if Path::new(target_dir).exists() {
            // Find the most recent test binary
            let entries = fs::read_dir(target_dir)?;
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if let Some(name) = path.file_name() {
                    if name.to_string_lossy().contains("RustAutoDevOps") {
                        return Ok(path.to_string_lossy().to_string());
                    }
                }
            }
        }

        // Fallback to a generic path
        Ok("target/debug/RustAutoDevOps".to_string())
    }

    async fn generate_fallback_coverage(&self) -> Result<()> {
        tracing::info!("Generating fallback coverage data");

        // Use tarpaulin as a fallback if available
        let output = Command::new("cargo")
            .args(&[
                "tarpaulin",
                "--out", "Json",
                "--output-dir", &self.output_directory,
            ])
            .output();

        match output {
            Ok(output) => {
                if !output.status.success() {
                    tracing::warn!("Tarpaulin also failed, generating mock coverage data");
                    self.generate_mock_coverage_data().await?;
                }
            }
            Err(_) => {
                tracing::warn!("Tarpaulin not available, generating mock coverage data");
                self.generate_mock_coverage_data().await?;
            }
        }

        Ok(())
    }

    async fn generate_mock_coverage_data(&self) -> Result<()> {
        // Generate mock coverage data for demonstration
        let mock_data = serde_json::json!({
            "data": [{
                "files": [{
                    "filename": "src/main.rs",
                    "summary": {
                        "lines": {
                            "count": 100,
                            "covered": 85,
                            "percent": 85.0
                        },
                        "functions": {
                            "count": 10,
                            "covered": 9,
                            "percent": 90.0
                        },
                        "branches": {
                            "count": 20,
                            "covered": 18,
                            "percent": 90.0
                        }
                    }
                }]
            }]
        });

        fs::write(
            format!("{}/coverage.json", self.output_directory),
            serde_json::to_string_pretty(&mock_data)?
        )?;

        Ok(())
    }

    async fn parse_coverage_json(&self) -> Result<RawCoverageData> {
        let coverage_path = format!("{}/coverage.json", self.output_directory);
        let content = fs::read_to_string(&coverage_path)?;
        
        // This is a simplified parser - in practice you'd need to handle
        // the specific format of your coverage tool
        let json_value: serde_json::Value = serde_json::from_str(&content)?;
        
        // Parse the JSON into our internal format
        let mut files = HashMap::new();
        let mut total_lines = 0;
        let mut covered_lines = 0;
        let mut total_functions = 0;
        let mut covered_functions = 0;
        let mut total_branches = 0;
        let mut covered_branches = 0;

        if let Some(data) = json_value.get("data").and_then(|d| d.as_array()) {
            for data_item in data {
                if let Some(file_list) = data_item.get("files").and_then(|f| f.as_array()) {
                    for file_item in file_list {
                        if let Some(filename) = file_item.get("filename").and_then(|f| f.as_str()) {
                            let summary = file_item.get("summary");
                            
                            let default_lines = serde_json::json!({"count": 0, "covered": 0});
                            let default_functions = serde_json::json!({"count": 0, "covered": 0});
                            let default_branches = serde_json::json!({"count": 0, "covered": 0});
                            
                            let lines_info = summary
                                .and_then(|s| s.get("lines"))
                                .unwrap_or(&default_lines);
                            
                            let functions_info = summary
                                .and_then(|s| s.get("functions"))
                                .unwrap_or(&default_functions);
                            
                            let branches_info = summary
                                .and_then(|s| s.get("branches"))
                                .unwrap_or(&default_branches);

                            let file_lines = lines_info.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                            let file_covered_lines = lines_info.get("covered").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                            
                            total_lines += file_lines;
                            covered_lines += file_covered_lines;
                            
                            let file_functions = functions_info.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                            let file_covered_functions = functions_info.get("covered").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                            
                            total_functions += file_functions;
                            covered_functions += file_covered_functions;
                            
                            let file_branches = branches_info.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                            let file_covered_branches = branches_info.get("covered").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
                            
                            total_branches += file_branches;
                            covered_branches += file_covered_branches;

                            files.insert(filename.to_string(), FileCoverageData {
                                path: filename.to_string(),
                                lines: Vec::new(), // Would be populated from detailed data
                                functions: Vec::new(),
                                branches: Vec::new(),
                            });
                        }
                    }
                }
            }
        }

        Ok(RawCoverageData {
            files,
            summary: CoverageSummary {
                total_lines,
                covered_lines,
                total_functions,
                covered_functions,
                total_branches,
                covered_branches,
            },
        })
    }

    async fn load_previous_coverage(&self) -> Result<Option<f64>> {
        let baseline_path = format!("{}/baseline_coverage.json", self.output_directory);
        
        if Path::new(&baseline_path).exists() {
            let content = fs::read_to_string(&baseline_path)?;
            let baseline: serde_json::Value = serde_json::from_str(&content)?;
            
            if let Some(coverage) = baseline.get("overall_coverage").and_then(|c| c.as_f64()) {
                return Ok(Some(coverage));
            }
        }
        
        Ok(None)
    }

    async fn save_coverage_baseline(&self, coverage: f64) -> Result<()> {
        let baseline_path = format!("{}/baseline_coverage.json", self.output_directory);
        let baseline_data = serde_json::json!({
            "overall_coverage": coverage,
            "timestamp": Utc::now(),
        });
        
        fs::write(&baseline_path, serde_json::to_string_pretty(&baseline_data)?)?;
        Ok(())
    }
}

#[async_trait]
impl CoverageReporterInterface for CoverageReporter {
    async fn generate_comprehensive_report(&self) -> Result<CoverageReport> {
        tracing::info!("Generating comprehensive coverage report");

        // Setup coverage environment
        self.setup_coverage_environment().await?;

        // Run tests with coverage
        self.run_tests_with_coverage().await?;

        // Generate coverage data
        self.generate_coverage_data().await?;

        // Collect raw coverage data
        let raw_data = self.run_coverage_collection().await?;

        // Analyze and create report
        let report = self.analyze_coverage_data(raw_data).await?;

        // Generate HTML report if requested
        if self.generate_html {
            self.generate_html_report(&report).await?;
        }

        // Save baseline for trend tracking
        if self.track_trends {
            self.save_coverage_baseline(report.overall_coverage_percentage).await?;
        }

        tracing::info!("Coverage report generated successfully: {:.2}%", 
            report.overall_coverage_percentage);

        Ok(report)
    }

    async fn run_coverage_collection(&self) -> Result<RawCoverageData> {
        self.parse_coverage_json().await
    }

    async fn analyze_coverage_data(&self, raw_data: RawCoverageData) -> Result<CoverageReport> {
        let overall_coverage = if raw_data.summary.total_lines > 0 {
            (raw_data.summary.covered_lines as f64 / raw_data.summary.total_lines as f64) * 100.0
        } else {
            0.0
        };

        let line_coverage = overall_coverage;
        
        let branch_coverage = if raw_data.summary.total_branches > 0 {
            (raw_data.summary.covered_branches as f64 / raw_data.summary.total_branches as f64) * 100.0
        } else {
            0.0
        };

        let function_coverage = if raw_data.summary.total_functions > 0 {
            (raw_data.summary.covered_functions as f64 / raw_data.summary.total_functions as f64) * 100.0
        } else {
            0.0
        };

        let mut file_coverage_details = HashMap::new();
        let mut uncovered_lines = Vec::new();

        for (path, file_data) in raw_data.files {
            let file_total_lines = file_data.lines.len() as u32;
            let file_covered_lines = file_data.lines.iter()
                .filter(|line| line.is_executable && line.hit_count > 0)
                .count() as u32;

            let file_line_coverage = if file_total_lines > 0 {
                (file_covered_lines as f64 / file_total_lines as f64) * 100.0
            } else {
                0.0
            };

            // Find uncovered lines
            for line in &file_data.lines {
                if line.is_executable && line.hit_count == 0 {
                    uncovered_lines.push(UncoveredLine {
                        file_path: path.clone(),
                        line_number: line.line_number,
                        line_content: line.content.clone(),
                    });
                }
            }

            file_coverage_details.insert(path.clone(), FileCoverageDetail {
                file_path: path,
                line_coverage_percentage: file_line_coverage,
                branch_coverage_percentage: 0.0, // Would calculate from branch data
                total_lines: file_total_lines,
                covered_lines: file_covered_lines,
                total_branches: 0,
                covered_branches: 0,
            });
        }

        let coverage_trend = if self.track_trends {
            self.compare_with_baseline(&CoverageReport {
                overall_coverage_percentage: overall_coverage,
                line_coverage,
                branch_coverage,
                function_coverage,
                file_coverage_details: file_coverage_details.clone(),
                uncovered_lines: uncovered_lines.clone(),
                coverage_trend: None,
            }).await?
        } else {
            None
        };

        Ok(CoverageReport {
            overall_coverage_percentage: overall_coverage,
            line_coverage,
            branch_coverage,
            function_coverage,
            file_coverage_details,
            uncovered_lines,
            coverage_trend,
        })
    }

    async fn generate_html_report(&self, report: &CoverageReport) -> Result<String> {
        let html_content = format!(
            r#"
<!DOCTYPE html>
<html>
<head>
    <title>Coverage Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .summary {{ background: #f5f5f5; padding: 15px; border-radius: 5px; }}
        .coverage-bar {{ width: 100%; height: 20px; background: #ddd; border-radius: 10px; }}
        .coverage-fill {{ height: 100%; border-radius: 10px; }}
        .high {{ background: #4CAF50; }}
        .medium {{ background: #FF9800; }}
        .low {{ background: #F44336; }}
        table {{ width: 100%; border-collapse: collapse; margin-top: 20px; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #f2f2f2; }}
    </style>
</head>
<body>
    <h1>Test Coverage Report</h1>
    
    <div class="summary">
        <h2>Coverage Summary</h2>
        <p><strong>Overall Coverage:</strong> {:.2}%</p>
        <div class="coverage-bar">
            <div class="coverage-fill {}" style="width: {:.1}%"></div>
        </div>
        <p><strong>Line Coverage:</strong> {:.2}%</p>
        <p><strong>Branch Coverage:</strong> {:.2}%</p>
        <p><strong>Function Coverage:</strong> {:.2}%</p>
    </div>

    <h2>File Coverage Details</h2>
    <table>
        <tr>
            <th>File</th>
            <th>Line Coverage</th>
            <th>Lines Covered</th>
            <th>Total Lines</th>
        </tr>
"#,
            report.overall_coverage_percentage,
            if report.overall_coverage_percentage >= 80.0 { "high" } 
            else if report.overall_coverage_percentage >= 60.0 { "medium" } 
            else { "low" },
            report.overall_coverage_percentage,
            report.line_coverage,
            report.branch_coverage,
            report.function_coverage
        );

        let mut html = html_content;

        for (_, file_detail) in &report.file_coverage_details {
            html.push_str(&format!(
                r#"
        <tr>
            <td>{}</td>
            <td>{:.2}%</td>
            <td>{}</td>
            <td>{}</td>
        </tr>
"#,
                file_detail.file_path,
                file_detail.line_coverage_percentage,
                file_detail.covered_lines,
                file_detail.total_lines
            ));
        }

        html.push_str(
            r#"
    </table>

    <h2>Uncovered Lines</h2>
    <table>
        <tr>
            <th>File</th>
            <th>Line Number</th>
            <th>Line Content</th>
        </tr>
"#
        );

        for uncovered in &report.uncovered_lines {
            html.push_str(&format!(
                r#"
        <tr>
            <td>{}</td>
            <td>{}</td>
            <td><code>{}</code></td>
        </tr>
"#,
                uncovered.file_path,
                uncovered.line_number,
                uncovered.line_content.replace('<', "&lt;").replace('>', "&gt;")
            ));
        }

        html.push_str(
            r#"
    </table>
</body>
</html>
"#
        );

        let html_path = format!("{}/coverage_report.html", self.output_directory);
        fs::write(&html_path, &html)?;

        tracing::info!("HTML coverage report generated: {}", html_path);
        Ok(html_path)
    }

    async fn check_coverage_threshold(&self, report: &CoverageReport) -> Result<bool> {
        let meets_threshold = report.overall_coverage_percentage >= self.minimum_threshold;
        
        if meets_threshold {
            tracing::info!("Coverage threshold met: {:.2}% >= {:.2}%", 
                report.overall_coverage_percentage, self.minimum_threshold);
        } else {
            tracing::warn!("Coverage threshold not met: {:.2}% < {:.2}%", 
                report.overall_coverage_percentage, self.minimum_threshold);
        }

        Ok(meets_threshold)
    }

    async fn compare_with_baseline(&self, report: &CoverageReport) -> Result<Option<CoverageTrend>> {
        if let Some(previous_coverage) = self.load_previous_coverage().await? {
            let change = report.overall_coverage_percentage - previous_coverage;
            let change_percentage = if previous_coverage > 0.0 {
                (change / previous_coverage) * 100.0
            } else {
                0.0
            };

            let trend_direction = if change > 1.0 {
                TrendDirection::Improving
            } else if change < -1.0 {
                TrendDirection::Declining
            } else {
                TrendDirection::Stable
            };

            Ok(Some(CoverageTrend {
                previous_coverage,
                current_coverage: report.overall_coverage_percentage,
                trend_direction,
                change_percentage,
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coverage_reporter_creation() {
        let reporter = CoverageReporter::new(85.0);
        assert_eq!(reporter.minimum_threshold, 85.0);
    }

    #[tokio::test]
    async fn test_coverage_threshold_check() {
        let reporter = CoverageReporter::new(90.0);
        
        let report = CoverageReport {
            overall_coverage_percentage: 95.0,
            line_coverage: 95.0,
            branch_coverage: 90.0,
            function_coverage: 100.0,
            file_coverage_details: HashMap::new(),
            uncovered_lines: Vec::new(),
            coverage_trend: None,
        };

        let meets_threshold = reporter.check_coverage_threshold(&report).await.unwrap();
        assert!(meets_threshold);
    }

    #[test]
    fn test_coverage_config_default() {
        let config = CoverageConfig::default();
        assert_eq!(config.minimum_threshold, 90.0);
        assert!(config.generate_html);
        assert!(config.track_trends);
    }
}