# DevOps CI - GitHub OAuth Integration with MongoDB

A modern Rust-based DevOps CI/CD tool with GitHub OAuth authentication and MongoDB integration.

## 🚀 Features

- ✅ GitHub OAuth authentication
- ✅ MongoDB database integration
- ✅ JWT token-based session management
- ✅ User profile management
- ✅ Secure cookie handling
- ✅ Comprehensive error handling and logging
- ✅ Modern web UI for authentication
- 🚧 CI/CD pipeline integration (coming soon)

## 🛠️ Setup

### Prerequisites

- Rust 1.70+ installed
- MongoDB Atlas account or local MongoDB instance
- GitHub OAuth App configured

### 1. Create GitHub OAuth App

1. Go to GitHub Settings → Developer settings → OAuth Apps
2. Click "New OAuth App"
3. Fill in the details:
   - **Application name**: DevOps CI
   - **Homepage URL**: `http://localhost:8000`
   - **Authorization callback URL**: `http://localhost:8000/api/sessions/oauth/github/callback`
4. Save the Client ID and Client Secret

### 2. MongoDB Setup

You can use either:
- **MongoDB Atlas** (recommended for production)
- **Local MongoDB** instance

Your MongoDB connection details are already configured for:
- **URI**: `mongodb+srv://ritabrataroychowdhury:ritabrata676@cluster0.vlzfl.mongodb.net/`
- **Database**: `dqms`

### 3. Environment Configuration

Create a `.env` file in the project root:

```env
# MongoDB Configuration
MONGODB_URI=mongodb+srv://ritabrataroychowdhury:ritabrata676@cluster0.vlzfl.mongodb.net/
MONGODB_DATABASE=dqms

# JWT Configuration
JWT_SECRET=your_super_secret_jwt_key_here_make_it_very_long_and_secure
JWT_EXPIRED_IN=60m
JWT_MAXAGE=60

# GitHub OAuth Configuration
GITHUB_OAUTH_CLIENT_ID=your_github_client_id_here
GITHUB_OAUTH_CLIENT_SECRET=your_github_client_secret_here
GITHUB_OAUTH_REDIRECT_URL=http://localhost:8000/api/sessions/oauth/github/callback

# Server Configuration
CLIENT_ORIGIN=http://localhost:3000
PORT=8000

# Environment
RUST_LOG=info
```

### 4. Run the Application

```bash
# Install dependencies and run
cargo run
```

The server will start on `http://localhost:8000`

## 📡 API Endpoints

### Authentication
- `GET /api/sessions/oauth/google` - OAuth login page
- `GET /api/sessions/oauth/github` - Initiate GitHub OAuth flow
- `GET /api/sessions/oauth/github/callback` - Handle OAuth callback
- `GET /api/sessions/me` - Get current user info (requires authentication)
- `GET /api/sessions/logout` - Logout user

### System
- `GET /api/healthchecker` - Health check with MongoDB status

## 🔧 Usage Examples

### 1. Authenticate with GitHub

Visit `http://localhost:8000/api/sessions/oauth/google` in your browser to see the login page, then click "Login with GitHub".

### 2. Get User Info (Protected Route)

```bash
# Using cookie (after login)
curl -b "token=YOUR_JWT_TOKEN" "http://localhost:8000/api/sessions/me"

# Using Authorization header
curl -H "Authorization: Bearer YOUR_JWT_TOKEN" "http://localhost:8000/api/sessions/me"
```

### 3. Health Check

```bash
curl "http://localhost:8000/api/healthchecker"
```

## 🏗️ Architecture

```
src/
├── config.rs           # Configuration management
├── database.rs         # MongoDB connection and operations
├── error.rs           # Error handling
├── token.rs           # JWT token management
├── models/            # Data models
│   ├── mod.rs
│   └── user.rs        # User model with MongoDB support
├── middleware/        # HTTP middleware
│   ├── mod.rs
│   └── auth.rs        # Authentication middleware
├── handlers/          # HTTP request handlers
│   ├── mod.rs
│   └── auth.rs        # Authentication handlers
└── routes/            # Route definitions
    ├── mod.rs
    └── auth.rs        # Authentication routes
```

## 🗄️ Database Schema

### Users Collection

```javascript
{
  "_id": ObjectId("..."),
  "id": "uuid-v4-string",
  "name": "User Name",
  "email": "user@example.com",
  "photo": "https://avatar-url.com/image.jpg",
  "verified": true,
  "provider": "GitHub",
  "role": "user",
  "created_at": ISODate("..."),
  "updated_at": ISODate("...")
}
```

## 🔐 Security Features

- OAuth 2.0 flow with state parameter validation
- JWT token-based authentication
- Secure HTTP-only cookies
- MongoDB connection with authentication
- CORS configuration
- Request timeout and rate limiting
- Comprehensive error handling
- Structured logging

## 📊 Logging

The application uses structured logging with different levels:
- `INFO` - General application flow
- `DEBUG` - Detailed debugging information  
- `ERROR` - Error conditions
- `WARN` - Warning conditions

Set `RUST_LOG=debug` for verbose logging.

## 🚧 Roadmap

- [ ] User role-based access control
- [ ] Repository webhook integration
- [ ] CI/CD pipeline triggers
- [ ] Multi-provider support (GitLab, Bitbucket)
- [ ] User session management with Redis
- [ ] Rate limiting and caching
- [ ] API documentation with OpenAPI
- [ ] Docker containerization
- [ ] Kubernetes deployment manifests

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## 📄 License

This project is licensed under CC0 1.0 Universal - see the [LICENSE](LICENSE) file for details.

## 🔧 Development

### Running in Development Mode

```bash
# With debug logging
RUST_LOG=debug cargo run

# With auto-reload (install cargo-watch first)
cargo install cargo-watch
cargo watch -x run
```

### Testing

```bash
# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture
```

### Building for Production

```bash
# Optimized build
cargo build --release

# Run optimized binary
./target/release/RustAutoDevOps
```