# Contributing to RustCI

Thank you for your interest in contributing to RustCI! This document provides guidelines and information for contributors.

## 📋 Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Contributing Guidelines](#contributing-guidelines)
- [Pull Request Process](#pull-request-process)
- [Code Style](#code-style)
- [Testing](#testing)
- [Documentation](#documentation)
- [Issue Reporting](#issue-reporting)

## 🤝 Code of Conduct

This project adheres to a code of conduct that we expect all contributors to follow. Please read and follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## 🚀 Getting Started

### Prerequisites

- Rust 1.70+ (Edition 2021)
- Docker and Docker Compose
- Git
- MongoDB (for local development)

### Development Setup

1. **Fork and Clone**
   ```bash
   git clone https://github.com/your-username/rustci.git
   cd rustci
   ```

2. **Set up Environment**
   ```bash
   cp .env.example .env
   # Edit .env with your configuration
   ```

3. **Install Dependencies**
   ```bash
   # Start MongoDB
   docker-compose up -d mongodb
   
   # Install Rust tools
   rustup component add clippy rustfmt
   ```

4. **Build and Test**
   ```bash
   cargo build
   cargo test
   ```

## 🛠️ Contributing Guidelines

### Types of Contributions

We welcome several types of contributions:

- **Bug Fixes**: Fix issues in the codebase
- **Features**: Add new functionality
- **Documentation**: Improve or add documentation
- **Performance**: Optimize existing code
- **Tests**: Add or improve test coverage
- **Refactoring**: Improve code structure without changing functionality

### Before You Start

1. **Check Existing Issues**: Look for existing issues or discussions
2. **Create an Issue**: For significant changes, create an issue first
3. **Discuss**: Engage with maintainers about your proposed changes
4. **Fork**: Fork the repository to your GitHub account

### Branch Naming

Use descriptive branch names:

- `feature/add-kubernetes-runner`
- `fix/memory-leak-in-job-queue`
- `docs/update-api-documentation`
- `perf/optimize-database-queries`
- `test/add-integration-tests`

## 🔄 Pull Request Process

### 1. Preparation

```bash
# Create feature branch
git checkout -b feature/your-feature-name

# Make your changes
# ... code changes ...

# Run tests
cargo test

# Run linting
cargo clippy -- -D warnings

# Format code
cargo fmt
```

### 2. Commit Guidelines

Follow conventional commit format:

```
type(scope): description

[optional body]

[optional footer]
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `perf`: Performance improvements
- `chore`: Maintenance tasks

**Examples:**
```
feat(runner): add Kubernetes runner support

Add support for executing jobs in Kubernetes pods with
configurable resource limits and node selection.

Closes #123
```

```
fix(auth): resolve JWT token expiration issue

Fix issue where JWT tokens were not being properly refreshed,
causing authentication failures after extended sessions.

Fixes #456
```

### 3. Pull Request Template

When creating a PR, include:

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring

## Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual testing completed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] Tests added/updated
- [ ] No breaking changes (or documented)
```

### 4. Review Process

1. **Automated Checks**: CI/CD pipeline runs automatically
2. **Code Review**: Maintainers review your code
3. **Feedback**: Address any requested changes
4. **Approval**: Get approval from maintainers
5. **Merge**: Maintainers merge your PR

## 🎨 Code Style

### Rust Style Guidelines

Follow standard Rust conventions:

```rust
// Use descriptive names
fn calculate_job_execution_time(start: Instant, end: Instant) -> Duration {
    end.duration_since(start)
}

// Prefer explicit error handling
fn load_configuration() -> Result<Config, ConfigError> {
    let config_path = std::env::var("CONFIG_PATH")
        .map_err(|_| ConfigError::MissingConfigPath)?;
    
    Config::from_file(&config_path)
        .map_err(ConfigError::InvalidFormat)
}

// Use appropriate visibility
pub struct JobRunner {
    id: Uuid,
    status: JobStatus,
    // Private fields by default
}

impl JobRunner {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            status: JobStatus::Idle,
        }
    }
    
    // Public interface methods
    pub fn execute_job(&mut self, job: Job) -> Result<JobResult, JobError> {
        // Implementation
    }
}
```

### Formatting

Use `cargo fmt` with default settings:

```bash
# Format all code
cargo fmt

# Check formatting without changing files
cargo fmt -- --check
```

### Linting

Use `cargo clippy` with strict settings:

```bash
# Run clippy with warnings as errors
cargo clippy -- -D warnings

# Run clippy on all targets
cargo clippy --all-targets -- -D warnings
```

### Documentation

Document public APIs:

```rust
/// Executes a CI/CD job with the specified configuration.
///
/// # Arguments
///
/// * `job` - The job configuration to execute
/// * `runner_config` - Configuration for the job runner
///
/// # Returns
///
/// Returns `Ok(JobResult)` on successful execution, or `Err(JobError)`
/// if the job fails or encounters an error.
///
/// # Examples
///
/// ```rust
/// use rustci::{Job, RunnerConfig, execute_job};
///
/// let job = Job::new("test-job");
/// let config = RunnerConfig::default();
/// let result = execute_job(job, config)?;
/// ```
pub fn execute_job(job: Job, runner_config: RunnerConfig) -> Result<JobResult, JobError> {
    // Implementation
}
```

## 🧪 Testing

### Test Categories

1. **Unit Tests**: Test individual functions and modules
2. **Integration Tests**: Test component interactions
3. **Performance Tests**: Benchmark critical paths
4. **End-to-End Tests**: Test complete workflows

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_job_execution_success() {
        let job = Job::new("test-job");
        let runner = JobRunner::new();
        
        let result = runner.execute(job).unwrap();
        
        assert_eq!(result.status, JobStatus::Completed);
        assert!(result.duration > Duration::from_secs(0));
    }
    
    #[tokio::test]
    async fn test_async_job_execution() {
        let job = Job::new("async-test-job");
        let runner = AsyncJobRunner::new().await;
        
        let result = runner.execute(job).await.unwrap();
        
        assert_eq!(result.status, JobStatus::Completed);
    }
}
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_job_execution

# Run tests with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration

# Run performance tests
cargo test --test performance --release
```

### Test Coverage

Maintain high test coverage:

```bash
# Install coverage tool
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html

# View coverage report
open tarpaulin-report.html
```

## 📚 Documentation

### Types of Documentation

1. **Code Documentation**: Inline comments and doc comments
2. **API Documentation**: Generated from doc comments
3. **User Documentation**: README, guides, tutorials
4. **Developer Documentation**: Architecture, design decisions

### Writing Documentation

- Use clear, concise language
- Include examples where helpful
- Keep documentation up-to-date with code changes
- Use proper markdown formatting

### Generating Documentation

```bash
# Generate API documentation
cargo doc --open

# Generate documentation for all dependencies
cargo doc --document-private-items --open
```

## 🐛 Issue Reporting

### Bug Reports

When reporting bugs, include:

1. **Description**: Clear description of the issue
2. **Steps to Reproduce**: Detailed steps to reproduce the bug
3. **Expected Behavior**: What you expected to happen
4. **Actual Behavior**: What actually happened
5. **Environment**: OS, Rust version, RustCI version
6. **Logs**: Relevant log output or error messages

### Feature Requests

When requesting features, include:

1. **Use Case**: Why is this feature needed?
2. **Description**: Detailed description of the feature
3. **Alternatives**: Alternative solutions considered
4. **Implementation**: Suggestions for implementation (if any)

### Issue Templates

Use the provided issue templates:

- **Bug Report**: For reporting bugs
- **Feature Request**: For requesting new features
- **Documentation**: For documentation improvements
- **Performance**: For performance-related issues

## 🏷️ Labels and Milestones

### Labels

- `bug`: Something isn't working
- `enhancement`: New feature or request
- `documentation`: Improvements or additions to documentation
- `good first issue`: Good for newcomers
- `help wanted`: Extra attention is needed
- `performance`: Performance-related issues
- `security`: Security-related issues

### Milestones

- **v1.1.0**: Next minor release
- **v2.0.0**: Next major release
- **Backlog**: Future considerations

## 🎯 Development Priorities

### High Priority

1. **Security**: Security fixes and improvements
2. **Performance**: Critical performance issues
3. **Stability**: Bug fixes affecting core functionality
4. **Documentation**: Critical documentation gaps

### Medium Priority

1. **Features**: New functionality
2. **Refactoring**: Code quality improvements
3. **Testing**: Test coverage improvements
4. **Developer Experience**: Tooling and workflow improvements

### Low Priority

1. **Nice-to-have Features**: Non-critical enhancements
2. **Code Style**: Minor style improvements
3. **Documentation Polish**: Minor documentation improvements

## 🤝 Community

### Communication Channels

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: General discussions and questions
- **Pull Requests**: Code contributions and reviews

### Getting Help

- Check existing documentation
- Search existing issues and discussions
- Create a new issue or discussion
- Tag maintainers if urgent

## 📄 License

By contributing to RustCI, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to RustCI! Your contributions help make this project better for everyone.