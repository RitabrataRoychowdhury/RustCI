# DevOps CI - GitHub OAuth Integration

A Rust-based DevOps CI/CD tool with GitHub OAuth authentication and webhook integration.

## 🚀 Features

- ✅ GitHub OAuth authentication
- ✅ User profile management  
- ✅ Repository access and management
- ✅ Webhook creation and management
- ✅ Comprehensive error handling and logging
- 🚧 CI/CD pipeline integration (coming soon)

## 🛠️ Setup

### Prerequisites

- Rust 1.70+ installed
- GitHub OAuth App configured

### 1. Create GitHub OAuth App

1. Go to GitHub Settings → Developer settings → OAuth Apps
2. Click "New OAuth App"
3. Fill in the details:
   - **Application name**: DevOps CI
   - **Homepage URL**: `http://localhost:3000`
   - **Authorization callback URL**: `http://localhost:3000/auth/github/callback`
4. Save the Client ID and Client Secret

### 2. Environment Configuration

Create a `.env` file in the project root:

```env
# GitHub OAuth Configuration
GITHUB_CLIENT_ID=your_github_client_id_here
GITHUB_CLIENT_SECRET=your_github_client_secret_here
GITHUB_REDIRECT_URI=http://localhost:3000/auth/github/callback

# Server Configuration
SERVER_URL=http://localhost:3000
SERVER_PORT=3000

# Environment
RUST_ENV=development
RUST_LOG=info,tower_http=debug
```

### 3. Run the Application

```bash
# Install dependencies and run
cargo run
```

The server will start on `http://localhost:3000`

## 📡 API Endpoints

### Authentication
- `GET /auth/github` - Initiate GitHub OAuth flow
- `GET /auth/github/callback` - Handle OAuth callback
- `GET /auth/user?access_token=TOKEN` - Get current user info

### Repository Management
- `GET /api/repos?access_token=TOKEN` - Get user repositories
- `GET /api/repos/{owner}/{repo}?access_token=TOKEN` - Get specific repository info

### System
- `GET /` - Health check
- `GET /health` - Detailed health check with system info

## 🔧 Usage Examples

### 1. Authenticate with GitHub

Visit `http://localhost:3000/auth/github` in your browser to start the OAuth flow.

### 2. Get User Info

```bash
curl "http://localhost:3000/auth/user?access_token=YOUR_ACCESS_TOKEN"
```

### 3. List Repositories

```bash
curl "http://localhost:3000/api/repos?access_token=YOUR_ACCESS_TOKEN&per_page=10"
```

### 4. Get Repository Info

```bash
curl "http://localhost:3000/api/repos/owner/repo-name?access_token=YOUR_ACCESS_TOKEN"
```

## 🏗️ Architecture

```
src/
├── config/          # Configuration management
├── domain/          # Domain models (User, etc.)
├── dto/             # Data Transfer Objects
├── handlers/        # HTTP request handlers
├── infrastructure/  # External service clients (GitHub API)
├── services/        # Business logic services
└── utils.rs         # Utility functions
```

## 🔐 Security Features

- OAuth 2.0 flow with state parameter validation
- Secure token handling
- CORS configuration
- Request timeout and rate limiting
- Comprehensive error handling

## 📊 Logging

The application uses structured logging with different levels:
- `INFO` - General application flow
- `DEBUG` - Detailed debugging information  
- `ERROR` - Error conditions
- `WARN` - Warning conditions

Set `RUST_LOG=debug` for verbose logging.

## 🚧 Roadmap

- [ ] JWT token management
- [ ] Webhook event processing
- [ ] CI/CD pipeline triggers
- [ ] Multi-provider support (GitLab, Bitbucket)
- [ ] Database integration
- [ ] User session management
- [ ] Rate limiting and caching

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## 📄 License

This project is licensed under CC0 1.0 Universal - see the [LICENSE](LICENSE) file for details.