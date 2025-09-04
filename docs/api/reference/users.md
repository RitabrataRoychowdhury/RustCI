# Users API Reference

The Users API provides endpoints for user management and authentication.

## Authentication

All user endpoints require authentication via JWT token:
```
Authorization: Bearer <your-jwt-token>
```

## Endpoints

### Get Current User

Get information about the currently authenticated user.

```http
GET /api/users/me
```

#### Headers
- `Authorization: Bearer <token>` (required)

#### Response
```json
{
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "username": "johndoe",
    "email": "john@example.com",
    "github_id": "123456",
    "roles": ["user"],
    "created_at": "2025-01-09T10:00:00Z",
    "last_login": "2025-01-09T12:00:00Z",
    "profile": {
      "display_name": "John Doe",
      "avatar_url": "https://avatars.githubusercontent.com/u/123456",
      "bio": "Software Developer"
    }
  },
  "meta": {
    "timestamp": "2025-01-09T14:00:00Z",
    "version": "1.0.0"
  }
}
```

#### Status Codes
- `200` - Success
- `401` - Unauthorized (invalid or expired token)

### Update Current User

Update information for the currently authenticated user.

```http
PUT /api/users/me
```

#### Headers
- `Authorization: Bearer <token>` (required)
- `Content-Type: application/json`

#### Request Body
```json
{
  "email": "newemail@example.com",
  "profile": {
    "display_name": "John Smith",
    "bio": "Senior Software Developer"
  }
}
```

#### Response
```json
{
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "username": "johndoe",
    "email": "newemail@example.com",
    "github_id": "123456",
    "roles": ["user"],
    "created_at": "2025-01-09T10:00:00Z",
    "updated_at": "2025-01-09T14:00:00Z",
    "profile": {
      "display_name": "John Smith",
      "avatar_url": "https://avatars.githubusercontent.com/u/123456",
      "bio": "Senior Software Developer"
    }
  },
  "meta": {
    "timestamp": "2025-01-09T14:00:00Z",
    "version": "1.0.0"
  }
}
```

#### Status Codes
- `200` - Success
- `400` - Bad Request (invalid data)
- `401` - Unauthorized
- `422` - Unprocessable Entity (validation errors)

## Data Models

### User Object
```json
{
  "id": "string (UUID)",
  "username": "string",
  "email": "string (email format)",
  "github_id": "string",
  "roles": ["string"],
  "created_at": "string (ISO 8601)",
  "updated_at": "string (ISO 8601)",
  "last_login": "string (ISO 8601)",
  "profile": {
    "display_name": "string",
    "avatar_url": "string (URL)",
    "bio": "string"
  }
}
```

### User Profile Object
```json
{
  "display_name": "string (max 100 chars)",
  "avatar_url": "string (URL, read-only)",
  "bio": "string (max 500 chars)"
}
```

## Examples

### cURL Examples

#### Get Current User
```bash
curl -H "Authorization: Bearer $RUSTCI_TOKEN" \
  https://api.rustci.dev/v1/api/users/me
```

#### Update User Profile
```bash
curl -X PUT \
  -H "Authorization: Bearer $RUSTCI_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "email": "newemail@example.com",
    "profile": {
      "display_name": "John Smith",
      "bio": "Senior Software Developer"
    }
  }' \
  https://api.rustci.dev/v1/api/users/me
```

### JavaScript Examples

```javascript
const client = new RustCIClient({
  token: process.env.RUSTCI_TOKEN
});

// Get current user
const user = await client.users.me();
console.log(`Hello, ${user.profile.display_name}!`);

// Update user profile
const updatedUser = await client.users.updateMe({
  email: 'newemail@example.com',
  profile: {
    display_name: 'John Smith',
    bio: 'Senior Software Developer'
  }
});
```

### Python Examples

```python
import rustci

client = rustci.Client(token=os.environ['RUSTCI_TOKEN'])

# Get current user
user = client.users.me()
print(f"Hello, {user['profile']['display_name']}!")

# Update user profile
updated_user = client.users.update_me({
    'email': 'newemail@example.com',
    'profile': {
        'display_name': 'John Smith',
        'bio': 'Senior Software Developer'
    }
})
```

## Error Handling

### Common Error Responses

#### Unauthorized (401)
```json
{
  "error": {
    "type": "AuthenticationError",
    "message": "Invalid or expired authentication token",
    "timestamp": "2025-01-09T14:00:00Z",
    "correlation_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

#### Validation Error (422)
```json
{
  "error": {
    "type": "ValidationError",
    "message": "Invalid user data",
    "details": {
      "email": ["Invalid email format"],
      "profile.display_name": ["Display name is too long (maximum 100 characters)"]
    },
    "timestamp": "2025-01-09T14:00:00Z",
    "correlation_id": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

## Related Endpoints

- **[Authentication](../README.md#authentication)** - OAuth login flow
- **[Workspaces](workspaces.md)** - User's workspaces
- **[Executions](executions.md)** - User's pipeline executions

## Notes

- User profiles are automatically synced with GitHub data during login
- Avatar URLs are managed by GitHub and cannot be updated via the API
- Username changes are not supported (linked to GitHub username)
- Email updates require verification (implementation pending)