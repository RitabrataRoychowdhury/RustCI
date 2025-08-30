# RustCI & Valkyrie Testing Scripts

This directory contains comprehensive testing scripts for all RustCI and Valkyrie functionalities. These scripts provide automated testing, performance validation, and security assessment capabilities.

## 📋 Available Scripts

### 🚀 Comprehensive Testing

#### `test-all-functionalities.sh`
**Complete system testing script covering all implemented features**

```bash
# Run all tests with interactive setup
./scripts/test-all-functionalities.sh

# Run all tests without authentication
./scripts/test-all-functionalities.sh --skip-setup

# Test against production server
./scripts/test-all-functionalities.sh --url https://prod.example.com

# Use specific JWT token
./scripts/test-all-functionalities.sh --token "eyJ..."
```

**Features Tested:**
- ✅ Error handling system with correlation tracking
- ✅ Configuration management and hot-reload
- ✅ Database operations and connection pooling
- ✅ Performance optimization (auto-scaling, load balancing)
- ✅ API robustness (versioning, rate limiting, authentication)
- ✅ Resource management and lifecycle
- ✅ Deployment capabilities (blue-green, circuit breakers)
- ✅ Testing framework and quality gates
- ✅ Valkyrie protocol optimization
- ✅ Self-healing system
- ✅ Complete pipeline workflows

### 🔍 Individual Component Testing

#### `test-individual-components.sh`
**Test specific components in isolation for detailed analysis**

```bash
# Test specific component
./scripts/test-individual-components.sh valkyrie
./scripts/test-individual-components.sh performance
./scripts/test-individual-components.sh security

# Test with authentication
./scripts/test-individual-components.sh --token "eyJ..." api

# Test all components
./scripts/test-individual-components.sh all
```

**Available Components:**
- `error-handling` - Error handling system
- `configuration` - Configuration management
- `database` - Database operations
- `performance` - Performance optimization
- `api` - API robustness
- `resources` - Resource management
- `deployment` - Deployment capabilities
- `valkyrie` - Valkyrie protocol
- `self-healing` - Self-healing system
- `testing` - Testing framework
- `pipeline` - Pipeline workflow
- `all` - All components

### ⚡ Valkyrie Performance Testing

#### `test-valkyrie-performance.sh`
**Comprehensive Valkyrie protocol performance validation**

```bash
# Run all performance tests
./scripts/test-valkyrie-performance.sh

# Test specific performance aspect
./scripts/test-valkyrie-performance.sh latency
./scripts/test-valkyrie-performance.sh throughput

# Custom configuration
./scripts/test-valkyrie-performance.sh --iterations 50000 --clients 20 --duration 60
```

**Performance Tests:**
- **Latency Benchmark** - Sub-millisecond validation
- **Throughput Benchmark** - High-performance validation  
- **Connection Pool Test** - Scalability validation
- **Batch Optimization** - Processing efficiency
- **Memory Efficiency** - Zero-copy validation

**Performance Criteria:**
- P50 latency < 100μs (Excellent)
- P95 latency < 300μs (Excellent)
- P99 latency < 500μs (Excellent)
- >99.5% sub-millisecond requests
- >50% very fast (<100μs) requests
- Throughput >100,000 ops/sec

### 🔒 Security Testing

#### `test-security-features.sh`
**Comprehensive security assessment and vulnerability testing**

```bash
# Run all security tests
./scripts/test-security-features.sh

# Test specific security aspect
./scripts/test-security-features.sh authentication
./scripts/test-security-features.sh input-validation

# Test against HTTPS endpoint
./scripts/test-security-features.sh --url https://secure.example.com
```

**Security Tests:**
- **Authentication** - OAuth, JWT, token validation
- **Input Validation** - SQL injection, XSS, command injection
- **Rate Limiting** - API throttling and abuse prevention
- **TLS Security** - HTTPS, certificate validation
- **Security Headers** - CORS, CSP, security headers
- **Session Management** - Cookie security, session timeout
- **API Security** - Method validation, path traversal

### 📊 Existing Scripts (Enhanced)

#### `test-api-endpoints.sh`
**Enhanced API endpoint testing with comprehensive coverage**

#### `test-pipeline-execution.sh`
**Complete pipeline lifecycle testing**

#### `valkyrie-test.sh`
**Valkyrie protocol validation suite**

#### `simple-valkyrie-performance-test.sh`
**Quick Valkyrie performance validation**

## 🛠️ Usage Examples

### Quick Start Testing

```bash
# 1. Start RustCI server
cargo run --bin RustAutoDevOps

# 2. Run comprehensive tests (in another terminal)
./scripts/test-all-functionalities.sh

# 3. Test Valkyrie performance
./scripts/test-valkyrie-performance.sh

# 4. Security assessment
./scripts/test-security-features.sh
```

### Production Testing

```bash
# Test against production environment
export BASE_URL="https://rustci.production.com"
export JWT_TOKEN="your-production-jwt-token"

./scripts/test-all-functionalities.sh --skip-setup
./scripts/test-security-features.sh
./scripts/test-valkyrie-performance.sh --iterations 100000
```

### CI/CD Integration

```bash
# Automated testing in CI pipeline
./scripts/test-all-functionalities.sh --skip-setup --url http://localhost:8000
./scripts/test-valkyrie-performance.sh --iterations 10000 --duration 30
./scripts/test-security-features.sh --url http://localhost:8000
```

## 📈 Test Output

All scripts generate comprehensive output including:

### 📁 Test Results Directory
```
test-results-YYYYMMDD-HHMMSS/
├── comprehensive_test_report.md     # Main test report
├── health_check.json               # Server health status
├── error_handling_test.json        # Error handling results
├── performance_metrics.json        # Performance data
├── security_test_report.md         # Security assessment
├── valkyrie_performance_report.md  # Valkyrie performance
└── ... (individual test files)
```

### 📋 Reports Generated

1. **Comprehensive Test Report** - Complete system analysis
2. **Performance Report** - Valkyrie protocol validation
3. **Security Report** - Vulnerability assessment
4. **Individual Component Reports** - Detailed component analysis

## 🔧 Configuration

### Environment Variables

```bash
# Server configuration
export BASE_URL="http://localhost:8000"
export JWT_TOKEN="your-jwt-token"

# Test configuration
export VERBOSE=true
export SKIP_SETUP=true

# Performance test configuration
export ITERATIONS=50000
export CONCURRENT_CLIENTS=20
export TEST_DURATION=60
```

### Prerequisites

- **Rust & Cargo** - For compiling performance tests
- **curl** - For API requests
- **jq** - For JSON processing (optional but recommended)
- **Docker** - For containerized testing (optional)

## 🎯 Test Coverage

### Foundation Systems (✅ Completed)
- Error handling with correlation tracking
- Configuration validation and hot-reload
- Database connection pooling and transactions
- Structured error reporting

### Performance Systems (✅ Completed)
- Auto-scaling and load balancing
- Intelligent caching strategies
- Resource management and quotas
- Performance monitoring and alerting

### Security & Reliability (✅ Completed)
- API authentication and rate limiting
- Circuit breaker patterns
- Self-healing mechanisms
- Blue-green deployments

### Valkyrie Protocol (✅ Completed)
- Sub-millisecond job dispatch
- Connection pooling optimization
- SIMD and zero-copy processing
- Performance metrics collection

### Testing Framework (✅ Completed)
- Production test suite
- Integration test manager
- Performance test runner
- Security test suite
- Quality gates and CI integration

## 🚨 Security Testing Features

### Authentication Testing
- OAuth flow validation
- JWT token security
- Invalid token handling
- Authorization bypass attempts

### Input Validation Testing
- SQL injection prevention
- XSS payload sanitization
- Command injection blocking
- Oversized payload handling

### API Security Testing
- HTTP method validation
- Path traversal prevention
- API versioning security
- Rate limiting effectiveness

## ⚡ Performance Validation

### Valkyrie Protocol Claims
- **Sub-millisecond latency** - P99 < 1ms
- **High throughput** - >50,000 ops/sec
- **Zero-copy optimization** - Memory efficiency
- **Connection pooling** - Resource optimization
- **Batch processing** - Throughput improvement

### Performance Scoring
- **90-100**: Outstanding performance
- **75-89**: Excellent performance
- **60-74**: Good performance
- **45-59**: Acceptable performance
- **<45**: Needs improvement

## 🔄 Continuous Integration

### GitHub Actions Integration

```yaml
name: RustCI Testing
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
      - name: Start RustCI
        run: cargo run --bin RustAutoDevOps &
      - name: Run Tests
        run: |
          ./scripts/test-all-functionalities.sh --skip-setup
          ./scripts/test-valkyrie-performance.sh --iterations 10000
          ./scripts/test-security-features.sh
```

## 📞 Support

For issues or questions about the testing scripts:

1. Check the generated test reports for detailed analysis
2. Review individual test output files
3. Ensure all prerequisites are installed
4. Verify server is running and accessible
5. Check authentication configuration

## 🎉 Success Criteria

### All Tests Pass ✅
- Server health check successful
- Authentication properly configured
- All API endpoints responding correctly
- Performance benchmarks meet criteria
- Security tests show no critical vulnerabilities
- Valkyrie protocol validates sub-millisecond claims

### Performance Validation ✅
- P50 latency < 200μs
- P95 latency < 500μs
- P99 latency < 1ms
- Throughput > 25,000 ops/sec
- >95% sub-millisecond requests

### Security Validation ✅
- Authentication required for protected endpoints
- Input validation prevents injection attacks
- Rate limiting prevents abuse
- Security headers properly configured
- Session management secure

---

**Note:** These scripts test the implemented features based on the completed tasks in `tasks.md`. Some advanced features may require additional configuration or may be placeholders pending full implementation.