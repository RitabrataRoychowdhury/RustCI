# Documentation Reorganization Plan

## Current Issues
- Inconsistent terminology and references throughout documentation
- Outdated directory structure that doesn't match current project layout
- Scattered documentation with unclear organization
- Legacy summaries and cleanup documents that should be archived
- Inconsistent naming conventions and file organization

## Proposed New Structure

### 1. User Documentation (`docs/user/`)
```
docs/user/
├── README.md                    # User documentation index
├── getting-started/
│   ├── installation.md         # Installation guide
│   ├── quick-start.md          # Quick start tutorial
│   └── first-pipeline.md       # Creating your first pipeline
├── guides/
│   ├── pipeline-configuration.md
│   ├── runner-setup.md
│   ├── security-configuration.md
│   └── performance-tuning.md
└── examples/
    ├── basic-pipelines/
    ├── advanced-configurations/
    └── integration-examples/
```

### 2. API Documentation (`docs/api/`)
```
docs/api/
├── README.md                    # API documentation index
├── reference/
│   ├── openapi-spec.json       # OpenAPI specification
│   ├── endpoints.md            # Endpoint documentation
│   └── authentication.md       # Auth documentation
├── guides/
│   ├── api-usage-guide.md      # How to use the API
│   └── sdk-integration.md      # SDK integration guide
└── examples/
    ├── curl-examples.md
    └── client-libraries.md
```

### 3. Architecture Documentation (`docs/architecture/`)
```
docs/architecture/
├── README.md                    # Architecture overview
├── system-design.md            # High-level system design
├── components/
│   ├── control-plane.md        # Control plane architecture
│   ├── runners.md              # Runner architecture
│   ├── messaging.md            # Message queue architecture
│   └── storage.md              # Data storage architecture
├── protocols/
│   ├── node-communication.md   # Inter-node communication
│   └── api-protocols.md        # API communication protocols
├── security/
│   ├── security-model.md       # Security architecture
│   ├── authentication.md       # Auth mechanisms
│   └── encryption.md           # Encryption strategies
└── adrs/                       # Architecture Decision Records
    ├── README.md
    └── [numbered ADRs]
```

### 4. Deployment Documentation (`docs/deployment/`)
```
docs/deployment/
├── README.md                    # Deployment overview
├── environments/
│   ├── local-development.md    # Local setup
│   ├── staging.md              # Staging deployment
│   └── production.md           # Production deployment
├── platforms/
│   ├── kubernetes.md           # Kubernetes deployment
│   ├── docker.md               # Docker deployment
│   └── bare-metal.md           # Bare metal deployment
├── configuration/
│   ├── environment-variables.md
│   ├── config-files.md
│   └── secrets-management.md
└── monitoring/
    ├── observability-setup.md
    ├── metrics.md
    └── logging.md
```

### 5. Development Documentation (`docs/development/`)
```
docs/development/
├── README.md                    # Development overview
├── setup/
│   ├── environment-setup.md    # Dev environment setup
│   ├── building.md             # Build instructions
│   └── testing.md              # Testing guide
├── contributing/
│   ├── code-style.md           # Code style guide
│   ├── pull-requests.md        # PR guidelines
│   └── release-process.md      # Release process
├── architecture/
│   ├── code-organization.md    # Code structure
│   ├── design-patterns.md      # Design patterns used
│   └── performance.md          # Performance considerations
└── troubleshooting/
    ├── common-issues.md
    └── debugging.md
```

## Migration Actions

### Phase 1: Archive Legacy Content
1. Move all summaries to `docs/archive/summaries/`
2. Move cleanup documents to `docs/archive/cleanup/`
3. Archive legacy protocol documentation to `docs/archive/valkyrie/`

### Phase 2: Reorganize Existing Content
1. Restructure current documentation into new hierarchy
2. Update all references to use consistent "RustCI" terminology
3. Consolidate duplicate content
4. Update cross-references and links

### Phase 3: Create Missing Documentation
1. Create comprehensive user guides
2. Develop API documentation from current state
3. Document deployment procedures for all platforms
4. Create development onboarding documentation

### Phase 4: Update Navigation
1. Create main documentation index
2. Update README files in each section
3. Ensure consistent navigation structure
4. Add cross-references between sections

## Content Updates Required

### Global Changes
- Replace inconsistent references with "RustCI" terminology
- Update project URLs and repository references
- Standardize terminology and naming conventions
- Update version numbers and compatibility information

### Specific Files to Update
- All pipeline examples need RustCI branding
- API documentation needs current endpoint information
- Deployment guides need current configuration examples
- Architecture documents need current system design

## Implementation Priority
1. **High Priority**: User documentation (getting started, guides)
2. **Medium Priority**: API and deployment documentation
3. **Low Priority**: Archive organization and legacy content cleanup

This reorganization will create a clear, navigable documentation structure that matches the current project state and provides comprehensive coverage for all user types.