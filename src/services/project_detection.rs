use crate::{
    error::{AppError, Result},
    models::{GitHubContent, ProjectType},
};

pub trait ProjectTypeDetector: Send + Sync {
    fn detect(&self, files: &[GitHubContent]) -> Result<ProjectType>;
    fn confidence_score(&self) -> f32;
    fn supported_types(&self) -> Vec<ProjectType>;
}

pub struct ProjectTypeDetectorFactory;

impl ProjectTypeDetectorFactory {
    pub fn create_detector(project_type: ProjectType) -> Box<dyn ProjectTypeDetector> {
        match project_type {
            ProjectType::Rust => Box::new(RustProjectDetector::new()),
            ProjectType::Node => Box::new(NodeProjectDetector::new()),
            ProjectType::Python => Box::new(PythonProjectDetector::new()),
            ProjectType::Java => Box::new(JavaProjectDetector::new()),
            ProjectType::Go => Box::new(GoProjectDetector::new()),
            ProjectType::Unknown => Box::new(UnknownProjectDetector::new()),
        }
    }
    
    pub fn detect_project_type(files: &[GitHubContent]) -> Result<ProjectType> {
        let detectors = vec![
            Self::create_detector(ProjectType::Rust),
            Self::create_detector(ProjectType::Node),
            Self::create_detector(ProjectType::Python),
            Self::create_detector(ProjectType::Java),
            Self::create_detector(ProjectType::Go),
        ];
        
        let mut best_match = None;
        let mut highest_confidence = 0.0;
        
        for detector in detectors {
            if let Ok(detected_type) = detector.detect(files) {
                let confidence = detector.confidence_score();
                if confidence > highest_confidence {
                    highest_confidence = confidence;
                    best_match = Some(detected_type);
                }
            }
        }
        
        Ok(best_match.unwrap_or(ProjectType::Unknown))
    }
    
    pub fn get_all_detectors() -> Vec<Box<dyn ProjectTypeDetector>> {
        vec![
            Self::create_detector(ProjectType::Rust),
            Self::create_detector(ProjectType::Node),
            Self::create_detector(ProjectType::Python),
            Self::create_detector(ProjectType::Java),
            Self::create_detector(ProjectType::Go),
        ]
    }
}

// Rust Project Detector
pub struct RustProjectDetector;

impl RustProjectDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectTypeDetector for RustProjectDetector {
    fn detect(&self, files: &[GitHubContent]) -> Result<ProjectType> {
        let has_cargo_toml = files.iter().any(|f| f.name == "Cargo.toml");
        let has_src_dir = files.iter().any(|f| f.name == "src" && f.file_type == "dir");
        let has_rust_files = files.iter().any(|f| f.name.ends_with(".rs"));
        
        if has_cargo_toml && (has_src_dir || has_rust_files) {
            Ok(ProjectType::Rust)
        } else {
            Err(AppError::ProjectTypeDetectionFailed("Not a Rust project".to_string()))
        }
    }
    
    fn confidence_score(&self) -> f32 { 0.95 }
    fn supported_types(&self) -> Vec<ProjectType> { vec![ProjectType::Rust] }
}

// Node.js Project Detector
pub struct NodeProjectDetector;

impl NodeProjectDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectTypeDetector for NodeProjectDetector {
    fn detect(&self, files: &[GitHubContent]) -> Result<ProjectType> {
        let has_package_json = files.iter().any(|f| f.name == "package.json");
        let has_node_modules = files.iter().any(|f| f.name == "node_modules" && f.file_type == "dir");
        let has_js_files = files.iter().any(|f| f.name.ends_with(".js") || f.name.ends_with(".ts"));
        let has_yarn_lock = files.iter().any(|f| f.name == "yarn.lock");
        let has_package_lock = files.iter().any(|f| f.name == "package-lock.json");
        
        if has_package_json && (has_node_modules || has_js_files || has_yarn_lock || has_package_lock) {
            Ok(ProjectType::Node)
        } else {
            Err(AppError::ProjectTypeDetectionFailed("Not a Node.js project".to_string()))
        }
    }
    
    fn confidence_score(&self) -> f32 { 0.90 }
    fn supported_types(&self) -> Vec<ProjectType> { vec![ProjectType::Node] }
}

// Python Project Detector
pub struct PythonProjectDetector;

impl PythonProjectDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectTypeDetector for PythonProjectDetector {
    fn detect(&self, files: &[GitHubContent]) -> Result<ProjectType> {
        let has_requirements_txt = files.iter().any(|f| f.name == "requirements.txt");
        let has_setup_py = files.iter().any(|f| f.name == "setup.py");
        let has_pyproject_toml = files.iter().any(|f| f.name == "pyproject.toml");
        let has_pipfile = files.iter().any(|f| f.name == "Pipfile");
        let has_python_files = files.iter().any(|f| f.name.ends_with(".py"));
        let has_venv = files.iter().any(|f| (f.name == "venv" || f.name == ".venv") && f.file_type == "dir");
        
        if (has_requirements_txt || has_setup_py || has_pyproject_toml || has_pipfile || has_venv) && has_python_files {
            Ok(ProjectType::Python)
        } else if has_python_files {
            // Lower confidence if only Python files are present
            Ok(ProjectType::Python)
        } else {
            Err(AppError::ProjectTypeDetectionFailed("Not a Python project".to_string()))
        }
    }
    
    fn confidence_score(&self) -> f32 { 0.85 }
    fn supported_types(&self) -> Vec<ProjectType> { vec![ProjectType::Python] }
}

// Java Project Detector
pub struct JavaProjectDetector;

impl JavaProjectDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectTypeDetector for JavaProjectDetector {
    fn detect(&self, files: &[GitHubContent]) -> Result<ProjectType> {
        let has_pom_xml = files.iter().any(|f| f.name == "pom.xml");
        let has_build_gradle = files.iter().any(|f| f.name == "build.gradle" || f.name == "build.gradle.kts");
        let has_java_files = files.iter().any(|f| f.name.ends_with(".java"));
        let has_src_main = files.iter().any(|f| f.path.contains("src/main"));
        let has_gradle_wrapper = files.iter().any(|f| f.name == "gradlew");
        
        if (has_pom_xml || has_build_gradle || has_gradle_wrapper) && (has_java_files || has_src_main) {
            Ok(ProjectType::Java)
        } else {
            Err(AppError::ProjectTypeDetectionFailed("Not a Java project".to_string()))
        }
    }
    
    fn confidence_score(&self) -> f32 { 0.88 }
    fn supported_types(&self) -> Vec<ProjectType> { vec![ProjectType::Java] }
}

// Go Project Detector
pub struct GoProjectDetector;

impl GoProjectDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectTypeDetector for GoProjectDetector {
    fn detect(&self, files: &[GitHubContent]) -> Result<ProjectType> {
        let has_go_mod = files.iter().any(|f| f.name == "go.mod");
        let has_go_sum = files.iter().any(|f| f.name == "go.sum");
        let has_go_files = files.iter().any(|f| f.name.ends_with(".go"));
        let has_main_go = files.iter().any(|f| f.name == "main.go");
        
        if has_go_mod && (has_go_files || has_main_go) {
            Ok(ProjectType::Go)
        } else if has_go_files && has_main_go {
            // Lower confidence for older Go projects without modules
            Ok(ProjectType::Go)
        } else {
            Err(AppError::ProjectTypeDetectionFailed("Not a Go project".to_string()))
        }
    }
    
    fn confidence_score(&self) -> f32 { 0.92 }
    fn supported_types(&self) -> Vec<ProjectType> { vec![ProjectType::Go] }
}

// Unknown Project Detector (fallback)
pub struct UnknownProjectDetector;

impl UnknownProjectDetector {
    pub fn new() -> Self {
        Self
    }
}

impl ProjectTypeDetector for UnknownProjectDetector {
    fn detect(&self, _files: &[GitHubContent]) -> Result<ProjectType> {
        Ok(ProjectType::Unknown)
    }
    
    fn confidence_score(&self) -> f32 { 0.0 }
    fn supported_types(&self) -> Vec<ProjectType> { vec![ProjectType::Unknown] }
}

// Helper function to analyze project files and return detailed information
pub fn analyze_project_files(files: &[GitHubContent]) -> ProjectAnalysis {
    let mut analysis = ProjectAnalysis::new();
    
    for file in files {
        match file.name.as_str() {
            "Cargo.toml" => analysis.rust_indicators.push("Cargo.toml found".to_string()),
            "package.json" => analysis.node_indicators.push("package.json found".to_string()),
            "requirements.txt" => analysis.python_indicators.push("requirements.txt found".to_string()),
            "pom.xml" => analysis.java_indicators.push("pom.xml found".to_string()),
            "go.mod" => analysis.go_indicators.push("go.mod found".to_string()),
            _ => {}
        }
        
        if file.name.ends_with(".rs") {
            analysis.rust_indicators.push(format!("Rust file: {}", file.name));
        } else if file.name.ends_with(".js") || file.name.ends_with(".ts") {
            analysis.node_indicators.push(format!("JavaScript/TypeScript file: {}", file.name));
        } else if file.name.ends_with(".py") {
            analysis.python_indicators.push(format!("Python file: {}", file.name));
        } else if file.name.ends_with(".java") {
            analysis.java_indicators.push(format!("Java file: {}", file.name));
        } else if file.name.ends_with(".go") {
            analysis.go_indicators.push(format!("Go file: {}", file.name));
        }
    }
    
    analysis
}

#[derive(Debug, Clone)]
pub struct ProjectAnalysis {
    pub rust_indicators: Vec<String>,
    pub node_indicators: Vec<String>,
    pub python_indicators: Vec<String>,
    pub java_indicators: Vec<String>,
    pub go_indicators: Vec<String>,
}

impl ProjectAnalysis {
    pub fn new() -> Self {
        Self {
            rust_indicators: Vec::new(),
            node_indicators: Vec::new(),
            python_indicators: Vec::new(),
            java_indicators: Vec::new(),
            go_indicators: Vec::new(),
        }
    }
    
    pub fn get_strongest_indicators(&self) -> (ProjectType, usize) {
        let counts = vec![
            (ProjectType::Rust, self.rust_indicators.len()),
            (ProjectType::Node, self.node_indicators.len()),
            (ProjectType::Python, self.python_indicators.len()),
            (ProjectType::Java, self.java_indicators.len()),
            (ProjectType::Go, self.go_indicators.len()),
        ];
        
        counts.into_iter()
            .max_by_key(|(_, count)| *count)
            .unwrap_or((ProjectType::Unknown, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GitHubContent;
    
    fn create_test_file(name: &str, file_type: &str) -> GitHubContent {
        GitHubContent {
            name: name.to_string(),
            path: name.to_string(),
            sha: "test_sha".to_string(),
            size: 100,
            file_type: file_type.to_string(),
            download_url: None,
            content: None,
            encoding: None,
        }
    }
    
    #[test]
    fn test_rust_project_detection() {
        let files = vec![
            create_test_file("Cargo.toml", "file"),
            create_test_file("src", "dir"),
            create_test_file("main.rs", "file"),
        ];
        
        let detector = RustProjectDetector::new();
        let result = detector.detect(&files).unwrap();
        
        assert_eq!(result, ProjectType::Rust);
        assert_eq!(detector.confidence_score(), 0.95);
    }
    
    #[test]
    fn test_node_project_detection() {
        let files = vec![
            create_test_file("package.json", "file"),
            create_test_file("index.js", "file"),
            create_test_file("node_modules", "dir"),
        ];
        
        let detector = NodeProjectDetector::new();
        let result = detector.detect(&files).unwrap();
        
        assert_eq!(result, ProjectType::Node);
        assert_eq!(detector.confidence_score(), 0.90);
    }
    
    #[test]
    fn test_python_project_detection() {
        let files = vec![
            create_test_file("requirements.txt", "file"),
            create_test_file("main.py", "file"),
            create_test_file("setup.py", "file"),
        ];
        
        let detector = PythonProjectDetector::new();
        let result = detector.detect(&files).unwrap();
        
        assert_eq!(result, ProjectType::Python);
        assert_eq!(detector.confidence_score(), 0.85);
    }
    
    #[test]
    fn test_java_project_detection() {
        let files = vec![
            create_test_file("pom.xml", "file"),
            create_test_file("Main.java", "file"),
            create_test_file("src/main", "dir"),
        ];
        
        let detector = JavaProjectDetector::new();
        let result = detector.detect(&files).unwrap();
        
        assert_eq!(result, ProjectType::Java);
        assert_eq!(detector.confidence_score(), 0.88);
    }
    
    #[test]
    fn test_go_project_detection() {
        let files = vec![
            create_test_file("go.mod", "file"),
            create_test_file("main.go", "file"),
            create_test_file("go.sum", "file"),
        ];
        
        let detector = GoProjectDetector::new();
        let result = detector.detect(&files).unwrap();
        
        assert_eq!(result, ProjectType::Go);
        assert_eq!(detector.confidence_score(), 0.92);
    }
    
    #[test]
    fn test_factory_pattern() {
        let rust_detector = ProjectTypeDetectorFactory::create_detector(ProjectType::Rust);
        let node_detector = ProjectTypeDetectorFactory::create_detector(ProjectType::Node);
        
        assert_eq!(rust_detector.confidence_score(), 0.95);
        assert_eq!(node_detector.confidence_score(), 0.90);
    }
    
    #[test]
    fn test_project_type_detection_with_factory() {
        let files = vec![
            create_test_file("Cargo.toml", "file"),
            create_test_file("src", "dir"),
        ];
        
        let detected_type = ProjectTypeDetectorFactory::detect_project_type(&files).unwrap();
        assert_eq!(detected_type, ProjectType::Rust);
    }
    
    #[test]
    fn test_project_analysis() {
        let files = vec![
            create_test_file("Cargo.toml", "file"),
            create_test_file("main.rs", "file"),
            create_test_file("package.json", "file"),
            create_test_file("index.js", "file"),
        ];
        
        let analysis = analyze_project_files(&files);
        
        assert!(!analysis.rust_indicators.is_empty());
        assert!(!analysis.node_indicators.is_empty());
        
        let (strongest_type, count) = analysis.get_strongest_indicators();
        assert!(count > 0);
        assert!(strongest_type == ProjectType::Rust || strongest_type == ProjectType::Node);
    }
    
    #[test]
    fn test_unknown_project_fallback() {
        let files = vec![
            create_test_file("README.md", "file"),
            create_test_file("LICENSE", "file"),
        ];
        
        let detected_type = ProjectTypeDetectorFactory::detect_project_type(&files).unwrap();
        assert_eq!(detected_type, ProjectType::Unknown);
    }
}