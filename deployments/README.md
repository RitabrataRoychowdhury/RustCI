# RustCI Deployment Configurations

This directory contains various deployment strategies and configurations for RustCI and Valkyrie.

## Directory Structure

```
deployments/
├── README.md                    # This file
├── vps/                        # VPS deployment configurations
│   ├── production.yaml         # Production VPS deployment
│   ├── staging.yaml           # Staging VPS deployment
│   └── development.yaml       # Development VPS deployment
├── strategies/                 # Deployment strategy templates
│   ├── blue-green.yaml        # Blue-green deployment
│   ├── rolling.yaml           # Rolling deployment
│   ├── canary.yaml            # Canary deployment
│   └── recreate.yaml          # Recreate deployment
├── environments/              # Environment-specific configs
│   ├── production.env         # Production environment variables
│   ├── staging.env           # Staging environment variables
│   └── development.env       # Development environment variables
└── scripts/                   # Deployment helper scripts
    ├── deploy.sh             # Main deployment script
    ├── rollback.sh           # Rollback script
    └── health-check.sh       # Health check script
```

## Deployment Strategies

### Blue-Green Deployment
- Zero-downtime deployment
- Full environment switch
- Easy rollback capability

### Rolling Deployment
- Gradual instance replacement
- Maintains service availability
- Resource efficient

### Canary Deployment
- Gradual traffic shifting
- Risk mitigation
- A/B testing capability

### Recreate Deployment
- Simple stop-and-start
- Suitable for development
- Brief downtime acceptable

## Usage

1. Choose your deployment strategy from `strategies/`
2. Configure environment variables in `environments/`
3. Use VPS-specific configurations from `vps/`
4. Execute deployment using scripts in `scripts/`

## VPS Information

- **Hostname**: ubuntu-ritabrata11092025
- **IP**: 46.37.122.118
- **Username**: root
- **Access**: SSH with password authentication