use std::collections::HashMap;
use crate::{
    error::{AppError, Result},
    models::{PullRequestRequest, ProjectType},
};

#[derive(Debug, Clone)]
pub struct PullRequestBuilder {
    owner: Option<String>,
    repo: Option<String>,
    title: Option<String>,
    body: Option<String>,
    head: Option<String>,
    base: Option<String>,
    draft: bool,
    labels: Vec<String>,
    assignees: Vec<String>,
    reviewers: Vec<String>,
    milestone: Option<String>,
    template_variables: HashMap<String, String>,
}

impl PullRequestBuilder {
    pub fn new() -> Self {
        Self {
            owner: None,
            repo: None,
            title: None,
            body: None,
            head: None,
            base: None,
            draft: false,
            labels: Vec::new(),
            assignees: Vec::new(),
            reviewers: Vec::new(),
            milestone: None,
            template_variables: HashMap::new(),
        }
    }
    
    pub fn owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self
    }
    
    pub fn repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }
    
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
    
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }
    
    pub fn head_branch(mut self, branch: impl Into<String>) -> Self {
        self.head = Some(branch.into());
        self
    }
    
    pub fn base_branch(mut self, branch: impl Into<String>) -> Self {
        self.base = Some(branch.into());
        self
    }
    
    pub fn draft(mut self, is_draft: bool) -> Self {
        self.draft = is_draft;
        self
    }
    
    pub fn add_label(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self
    }
    
    pub fn labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }
    
    pub fn add_assignee(mut self, assignee: impl Into<String>) -> Self {
        self.assignees.push(assignee.into());
        self
    }
    
    pub fn assignees(mut self, assignees: Vec<String>) -> Self {
        self.assignees = assignees;
        self
    }
    
    pub fn add_reviewer(mut self, reviewer: impl Into<String>) -> Self {
        self.reviewers.push(reviewer.into());
        self
    }
    
    pub fn reviewers(mut self, reviewers: Vec<String>) -> Self {
        self.reviewers = reviewers;
        self
    }
    
    pub fn milestone(mut self, milestone: impl Into<String>) -> Self {
        self.milestone = Some(milestone.into());
        self
    }
    
    pub fn template_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.template_variables.insert(key.into(), value.into());
        self
    }
    
    pub fn template_variables(mut self, variables: HashMap<String, String>) -> Self {
        self.template_variables = variables;
        self
    }
    
    pub fn build(self) -> Result<PullRequestRequest> {
        let owner = self.owner.ok_or_else(|| {
            AppError::ValidationError("Owner is required for pull request".to_string())
        })?;
        
        let repo = self.repo.ok_or_else(|| {
            AppError::ValidationError("Repository name is required for pull request".to_string())
        })?;
        
        let title = self.title.ok_or_else(|| {
            AppError::ValidationError("Title is required for pull request".to_string())
        })?;
        
        let head = self.head.ok_or_else(|| {
            AppError::ValidationError("Head branch is required for pull request".to_string())
        })?;
        
        let base = self.base.unwrap_or_else(|| "main".to_string());
        let body = self.body.unwrap_or_default();
        
        Ok(PullRequestRequest {
            owner,
            repo,
            title,
            body,
            head,
            base,
            draft: self.draft,
        })
    }
}

impl Default for PullRequestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Specialized builders for different types of PRs
pub struct DockerfilePRBuilder {
    base_builder: PullRequestBuilder,
    project_type: Option<ProjectType>,
    dockerfile_content: Option<String>,
    validation_passed: bool,
}

impl DockerfilePRBuilder {
    pub fn new() -> Self {
        Self {
            base_builder: PullRequestBuilder::new(),
            project_type: None,
            dockerfile_content: None,
            validation_passed: false,
        }
    }
    
    pub fn owner(mut self, owner: impl Into<String>) -> Self {
        self.base_builder = self.base_builder.owner(owner);
        self
    }
    
    pub fn repo(mut self, repo: impl Into<String>) -> Self {
        self.base_builder = self.base_builder.repo(repo);
        self
    }
    
    pub fn project_type(mut self, project_type: ProjectType) -> Self {
        self.project_type = Some(project_type);
        self
    }
    
    pub fn dockerfile_content(mut self, content: impl Into<String>) -> Self {
        self.dockerfile_content = Some(content.into());
        self
    }
    
    pub fn validation_passed(mut self, passed: bool) -> Self {
        self.validation_passed = passed;
        self
    }
    
    pub fn draft(mut self, is_draft: bool) -> Self {
        self.base_builder = self.base_builder.draft(is_draft);
        self
    }
    
    pub fn base_branch(mut self, branch: impl Into<String>) -> Self {
        self.base_builder = self.base_builder.base_branch(branch);
        self
    }
    
    pub fn build(self) -> Result<PullRequestRequest> {
        let project_type = self.project_type.unwrap_or(ProjectType::Unknown);
        
        // Generate appropriate title
        let title = format!("feat: Add auto-generated Dockerfile for {} project", 
            self.format_project_type(&project_type));
        
        // Generate comprehensive body
        let body = self.generate_pr_body(&project_type)?;
        
        // Set up the branch name
        let head_branch = "rustci/dockerfile-autogen".to_string();
        
        // Add appropriate labels
        let labels = self.generate_labels(&project_type);
        
        self.base_builder
            .title(title)
            .body(body)
            .head_branch(head_branch)
            .labels(labels)
            .build()
    }
    
    fn format_project_type(&self, project_type: &ProjectType) -> String {
        match project_type {
            ProjectType::Rust => "Rust".to_string(),
            ProjectType::Node => "Node.js".to_string(),
            ProjectType::Python => "Python".to_string(),
            ProjectType::Java => "Java".to_string(),
            ProjectType::Go => "Go".to_string(),
            ProjectType::Unknown => "Unknown".to_string(),
        }
    }
    
    fn generate_pr_body(&self, project_type: &ProjectType) -> Result<String> {
        let mut body = String::new();
        
        body.push_str("## 🐳 Auto-generated Dockerfile\n\n");
        body.push_str(&format!("This PR adds an auto-generated Dockerfile for this {} project.\n\n", 
            self.format_project_type(project_type)));
        
        body.push_str("### 📋 What's included:\n\n");
        body.push_str("- ✅ Multi-stage build for optimal image size\n");
        body.push_str("- ✅ Security best practices (non-root user)\n");
        body.push_str("- ✅ Health check configuration\n");
        body.push_str("- ✅ Proper dependency caching\n");
        body.push_str("- ✅ Production-ready configuration\n\n");
        
        if self.validation_passed {
            body.push_str("### ✅ Validation Status\n\n");
            body.push_str("- ✅ **Build Test**: Passed\n");
            body.push_str("- ✅ **Run Test**: Passed\n");
            body.push_str("- ✅ **Security Scan**: Passed\n\n");
        } else {
            body.push_str("### ⚠️ Validation Status\n\n");
            body.push_str("- ❌ **Build Test**: Failed or not run\n");
            body.push_str("- ❓ **Run Test**: Pending\n");
            body.push_str("- ❓ **Security Scan**: Pending\n\n");
            body.push_str("Please review the Dockerfile and make necessary adjustments.\n\n");
        }
        
        body.push_str("### 🔧 Project-specific optimizations:\n\n");
        match project_type {
            ProjectType::Rust => {
                body.push_str("- Cargo dependency caching for faster builds\n");
                body.push_str("- Minimal runtime image (Debian slim)\n");
                body.push_str("- Optimized for release builds\n");
            }
            ProjectType::Node => {
                body.push_str("- npm/yarn dependency caching\n");
                body.push_str("- Alpine-based images for smaller size\n");
                body.push_str("- Proper signal handling with dumb-init\n");
            }
            ProjectType::Python => {
                body.push_str("- pip dependency caching\n");
                body.push_str("- Python-specific environment variables\n");
                body.push_str("- Support for requirements.txt and pyproject.toml\n");
            }
            ProjectType::Java => {
                body.push_str("- Maven dependency caching\n");
                body.push_str("- JRE runtime optimization\n");
                body.push_str("- JAR file execution setup\n");
            }
            ProjectType::Go => {
                body.push_str("- Go module caching\n");
                body.push_str("- Static binary compilation\n");
                body.push_str("- Minimal Alpine runtime\n");
            }
            ProjectType::Unknown => {
                body.push_str("- Generic Ubuntu-based setup\n");
                body.push_str("- Basic security configurations\n");
                body.push_str("- Customization required for specific needs\n");
            }
        }
        
        body.push_str("\n### 🚀 Next Steps:\n\n");
        body.push_str("1. Review the generated Dockerfile\n");
        body.push_str("2. Test the build locally: `docker build -t my-app .`\n");
        body.push_str("3. Test the container: `docker run -p 8080:8080 my-app`\n");
        body.push_str("4. Customize as needed for your specific requirements\n");
        body.push_str("5. Merge when ready!\n\n");
        
        body.push_str("---\n");
        body.push_str("*This Dockerfile was automatically generated by RustCI. ");
        body.push_str("If you have any issues or suggestions, please let us know!*\n");
        
        Ok(body)
    }
    
    fn generate_labels(&self, project_type: &ProjectType) -> Vec<String> {
        let mut labels = vec![
            "dockerfile".to_string(),
            "auto-generated".to_string(),
            "docker".to_string(),
        ];
        
        match project_type {
            ProjectType::Rust => labels.push("rust".to_string()),
            ProjectType::Node => labels.push("nodejs".to_string()),
            ProjectType::Python => labels.push("python".to_string()),
            ProjectType::Java => labels.push("java".to_string()),
            ProjectType::Go => labels.push("golang".to_string()),
            ProjectType::Unknown => labels.push("needs-review".to_string()),
        }
        
        if self.validation_passed {
            labels.push("validated".to_string());
        } else {
            labels.push("needs-validation".to_string());
        }
        
        labels
    }
}

impl Default for DockerfilePRBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Template-based PR builder for more complex scenarios
pub struct TemplatePRBuilder {
    base_builder: PullRequestBuilder,
    template_name: Option<String>,
    template_data: HashMap<String, String>,
}

impl TemplatePRBuilder {
    pub fn new() -> Self {
        Self {
            base_builder: PullRequestBuilder::new(),
            template_name: None,
            template_data: HashMap::new(),
        }
    }
    
    pub fn owner(mut self, owner: impl Into<String>) -> Self {
        self.base_builder = self.base_builder.owner(owner);
        self
    }
    
    pub fn repo(mut self, repo: impl Into<String>) -> Self {
        self.base_builder = self.base_builder.repo(repo);
        self
    }
    
    pub fn template(mut self, template_name: impl Into<String>) -> Self {
        self.template_name = Some(template_name.into());
        self
    }
    
    pub fn template_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.template_data.insert(key.into(), value.into());
        self
    }
    
    pub fn template_data_map(mut self, data: HashMap<String, String>) -> Self {
        self.template_data = data;
        self
    }
    
    pub fn draft(mut self, is_draft: bool) -> Self {
        self.base_builder = self.base_builder.draft(is_draft);
        self
    }
    
    pub fn build(self) -> Result<PullRequestRequest> {
        let template_name = self.template_name.ok_or_else(|| {
            AppError::ValidationError("Template name is required".to_string())
        })?;
        
        let (title, body) = self.render_template(&template_name)?;
        
        self.base_builder
            .title(title)
            .body(body)
            .build()
    }
    
    fn render_template(&self, template_name: &str) -> Result<(String, String)> {
        match template_name {
            "dockerfile" => self.render_dockerfile_template(),
            "feature" => self.render_feature_template(),
            "bugfix" => self.render_bugfix_template(),
            "security" => self.render_security_template(),
            _ => Err(AppError::ValidationError(format!("Unknown template: {}", template_name))),
        }
    }
    
    fn render_dockerfile_template(&self) -> Result<(String, String)> {
        let project_type = self.template_data.get("project_type").unwrap_or(&"Unknown".to_string());
        let validation_status = self.template_data.get("validation_status").unwrap_or(&"unknown".to_string());
        
        let title = format!("feat: Add Dockerfile for {} project", project_type);
        let body = format!(
            "## Dockerfile Addition\n\nAdds a Dockerfile for {} project.\n\nValidation Status: {}\n",
            project_type, validation_status
        );
        
        Ok((title, body))
    }
    
    fn render_feature_template(&self) -> Result<(String, String)> {
        let feature_name = self.template_data.get("feature_name")
            .ok_or_else(|| AppError::ValidationError("feature_name is required for feature template".to_string()))?;
        
        let title = format!("feat: {}", feature_name);
        let body = format!("## New Feature\n\n{}\n\n### Changes\n\n- Added {}\n", feature_name, feature_name);
        
        Ok((title, body))
    }
    
    fn render_bugfix_template(&self) -> Result<(String, String)> {
        let bug_description = self.template_data.get("bug_description")
            .ok_or_else(|| AppError::ValidationError("bug_description is required for bugfix template".to_string()))?;
        
        let title = format!("fix: {}", bug_description);
        let body = format!("## Bug Fix\n\nFixes: {}\n\n### Changes\n\n- Fixed {}\n", bug_description, bug_description);
        
        Ok((title, body))
    }
    
    fn render_security_template(&self) -> Result<(String, String)> {
        let security_issue = self.template_data.get("security_issue")
            .ok_or_else(|| AppError::ValidationError("security_issue is required for security template".to_string()))?;
        
        let title = format!("security: {}", security_issue);
        let body = format!("## Security Fix\n\n⚠️ **Security Issue**: {}\n\n### Changes\n\n- Addressed security vulnerability\n", security_issue);
        
        Ok((title, body))
    }
}

impl Default for TemplatePRBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Factory for creating different types of PR builders
pub struct PRBuilderFactory;

impl PRBuilderFactory {
    pub fn dockerfile_pr() -> DockerfilePRBuilder {
        DockerfilePRBuilder::new()
    }
    
    pub fn template_pr() -> TemplatePRBuilder {
        TemplatePRBuilder::new()
    }
    
    pub fn basic_pr() -> PullRequestBuilder {
        PullRequestBuilder::new()
    }
    
    pub fn create_dockerfile_pr(
        owner: &str,
        repo: &str,
        project_type: ProjectType,
        validation_passed: bool,
    ) -> Result<PullRequestRequest> {
        Self::dockerfile_pr()
            .owner(owner)
            .repo(repo)
            .project_type(project_type)
            .validation_passed(validation_passed)
            .build()
    }
    
    pub fn create_template_pr(
        owner: &str,
        repo: &str,
        template_name: &str,
        template_data: HashMap<String, String>,
    ) -> Result<PullRequestRequest> {
        Self::template_pr()
            .owner(owner)
            .repo(repo)
            .template(template_name)
            .template_data_map(template_data)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_pr_builder() {
        let pr_request = PullRequestBuilder::new()
            .owner("testuser")
            .repo("testrepo")
            .title("Test PR")
            .body("This is a test PR")
            .head_branch("feature/test")
            .base_branch("main")
            .draft(false)
            .build()
            .unwrap();
        
        assert_eq!(pr_request.owner, "testuser");
        assert_eq!(pr_request.repo, "testrepo");
        assert_eq!(pr_request.title, "Test PR");
        assert_eq!(pr_request.body, "This is a test PR");
        assert_eq!(pr_request.head, "feature/test");
        assert_eq!(pr_request.base, "main");
        assert!(!pr_request.draft);
    }
    
    #[test]
    fn test_pr_builder_validation() {
        let result = PullRequestBuilder::new()
            .title("Test PR")
            .build();
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Owner is required"));
    }
    
    #[test]
    fn test_dockerfile_pr_builder() {
        let pr_request = DockerfilePRBuilder::new()
            .owner("testuser")
            .repo("testrepo")
            .project_type(ProjectType::Rust)
            .validation_passed(true)
            .build()
            .unwrap();
        
        assert_eq!(pr_request.owner, "testuser");
        assert_eq!(pr_request.repo, "testrepo");
        assert!(pr_request.title.contains("Rust"));
        assert!(pr_request.body.contains("Auto-generated Dockerfile"));
        assert!(pr_request.body.contains("Build Test**: Passed"));
        assert_eq!(pr_request.head, "rustci/dockerfile-autogen");
    }
    
    #[test]
    fn test_dockerfile_pr_builder_failed_validation() {
        let pr_request = DockerfilePRBuilder::new()
            .owner("testuser")
            .repo("testrepo")
            .project_type(ProjectType::Python)
            .validation_passed(false)
            .build()
            .unwrap();
        
        assert!(pr_request.body.contains("Build Test**: Failed"));
        assert!(pr_request.body.contains("Please review"));
    }
    
    #[test]
    fn test_template_pr_builder() {
        let mut template_data = HashMap::new();
        template_data.insert("project_type".to_string(), "Node.js".to_string());
        template_data.insert("validation_status".to_string(), "passed".to_string());
        
        let pr_request = TemplatePRBuilder::new()
            .owner("testuser")
            .repo("testrepo")
            .template("dockerfile")
            .template_data_map(template_data)
            .build()
            .unwrap();
        
        assert!(pr_request.title.contains("Node.js"));
        assert!(pr_request.body.contains("passed"));
    }
    
    #[test]
    fn test_pr_builder_factory() {
        let pr_request = PRBuilderFactory::create_dockerfile_pr(
            "testuser",
            "testrepo",
            ProjectType::Go,
            true,
        ).unwrap();
        
        assert_eq!(pr_request.owner, "testuser");
        assert_eq!(pr_request.repo, "testrepo");
        assert!(pr_request.title.contains("Go"));
    }
    
    #[test]
    fn test_template_factory() {
        let mut template_data = HashMap::new();
        template_data.insert("feature_name".to_string(), "User Authentication".to_string());
        
        let pr_request = PRBuilderFactory::create_template_pr(
            "testuser",
            "testrepo",
            "feature",
            template_data,
        ).unwrap();
        
        assert!(pr_request.title.contains("User Authentication"));
        assert!(pr_request.body.contains("New Feature"));
    }
    
    #[test]
    fn test_builder_fluent_interface() {
        let pr_request = PullRequestBuilder::new()
            .owner("testuser")
            .repo("testrepo")
            .title("Test")
            .add_label("enhancement")
            .add_label("docker")
            .add_assignee("developer1")
            .add_reviewer("reviewer1")
            .milestone("v1.0")
            .draft(true)
            .build()
            .unwrap();
        
        assert_eq!(pr_request.owner, "testuser");
        assert!(pr_request.draft);
    }
    
    #[test]
    fn test_default_base_branch() {
        let pr_request = PullRequestBuilder::new()
            .owner("testuser")
            .repo("testrepo")
            .title("Test")
            .head_branch("feature/test")
            // No base branch specified - should default to "main"
            .build()
            .unwrap();
        
        assert_eq!(pr_request.base, "main");
    }
}