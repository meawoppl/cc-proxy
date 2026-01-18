# Deployment Guide

This guide covers deploying claude-code-portal to production.

## Prerequisites

- **PostgreSQL Database**
  - [NeonDB](https://neon.tech) (recommended, serverless)
  - Or any PostgreSQL 12+ instance

- **Google OAuth Credentials** (see [Google OAuth Setup](#google-oauth-setup) below)

## Environment Variables

Create a `.env` file or set these environment variables:

```bash
# Database Connection (required)
DATABASE_URL=postgresql://user:password@host:5432/database?sslmode=require

# Google OAuth (required for production)
GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-client-secret
GOOGLE_REDIRECT_URI=https://your-domain.com/auth/google/callback

# Server Configuration
HOST=0.0.0.0
PORT=3000

# Security (required for production)
SESSION_SECRET=generate-a-random-32-char-secret-here

# Frontend Path (usually auto-detected)
FRONTEND_DIST=frontend/dist
```

## Docker Deployment (Recommended)

```bash
# Build images
docker-compose build

# Start services
docker-compose up -d

# View logs
docker-compose logs -f backend

# Stop services
docker-compose down
```

See [DOCKER.md](DOCKER.md) for detailed Docker deployment instructions.

## Manual Deployment

### 1. Set up PostgreSQL database

Create a database and note the connection string.

### 2. Configure environment

```bash
export DATABASE_URL="postgresql://..."
export GOOGLE_CLIENT_ID="..."
export GOOGLE_CLIENT_SECRET="..."
export GOOGLE_REDIRECT_URI="https://yourdomain.com/auth/google/callback"
export SESSION_SECRET="$(openssl rand -base64 32)"
```

### 3. Build frontend

```bash
cd frontend
trunk build --release
cd ..
```

### 4. Run migrations

```bash
cd backend
diesel migration run
cd ..
```

### 5. Start backend

```bash
cargo run --release -p backend
```

### 6. Distribute proxy binary

```bash
cargo build --release -p proxy
# Copy target/release/claude-portal to dev machines
```

## Backend Command-Line Options

```bash
cargo run -p backend -- [OPTIONS]

Options:
  --dev-mode              Enable development mode (bypasses OAuth)
  --frontend-dist <PATH>  Path to frontend dist directory [default: frontend/dist]
  -h, --help              Print help
```

## Proxy Command-Line Options

```bash
claude-portal [OPTIONS] -- [CLAUDE_ARGS]

Options:
  --backend-url <URL>     Backend WebSocket URL [default: ws://localhost:3000]
  --session-name <NAME>   Session name [default: hostname]
  --auth-token <TOKEN>    Authentication token (skips OAuth)
  --reauth                Force re-authentication
  --logout                Remove cached credentials

  # All other arguments are forwarded to claude CLI
```

## Admin Setup

To grant admin privileges to a user:

```bash
# Open a database shell
./scripts/db-shell.sh

# Or connect directly with psql
psql $DATABASE_URL
```

```sql
-- Grant admin privileges to a user
UPDATE users SET is_admin = true WHERE email = 'your@email.com';
```

Admins can access the admin dashboard at `/admin` which provides:
- System statistics (users, sessions, spend)
- User management (enable/disable, grant/revoke admin)
- Session management (view all sessions, force delete)

## Security Considerations

- **OAuth Tokens**: Stored securely in database, never exposed to frontend
- **WebSocket Auth**: All WebSocket connections require valid auth tokens
- **Session Isolation**: Users can only access their own sessions
- **HTTPS**: Use HTTPS in production (handled by reverse proxy)
- **Environment Secrets**: Never commit `.env` to version control
- **Database**: Use SSL/TLS for database connections in production

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux (x86_64) | Tested | Primary development platform |
| macOS (Apple Silicon) | Untested | Builds in CI, PRs welcome |
| macOS (Intel) | Untested | Builds in CI, PRs welcome |
| Windows (x86_64) | Untested | Builds in CI, PRs welcome |

Pre-built binaries for all platforms are available from [GitHub Releases](https://github.com/meawoppl/claude-code-portal/releases/latest).

## Troubleshooting

See [TROUBLESHOOTING.md](../TROUBLESHOOTING.md) for common issues and solutions.

## Google OAuth Setup

To deploy your own instance, you need Google OAuth credentials.

### 1. Create a Google Cloud Project

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select an existing one
3. Enable the **Google+ API** (for user info)

### 2. Configure OAuth Consent Screen

1. Navigate to **APIs & Services > OAuth consent screen**
2. Choose **External** (or **Internal** for Google Workspace orgs)
3. Fill in the required fields:
   - App name: Your portal name
   - User support email: Your email
   - Developer contact: Your email
4. Add scopes: `email`, `profile`, `openid`
5. Add test users if in testing mode

### 3. Create OAuth Credentials

1. Navigate to **APIs & Services > Credentials**
2. Click **Create Credentials > OAuth client ID**
3. Application type: **Web application**
4. Add authorized redirect URIs:
   - `https://your-domain.com/auth/google/callback`
   - `http://localhost:3000/auth/google/callback` (for development)
5. Save the **Client ID** and **Client Secret**

### 4. Configure Environment

Add to your `.env` file:

```bash
GOOGLE_CLIENT_ID=your-client-id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your-client-secret
GOOGLE_REDIRECT_URI=https://your-domain.com/auth/google/callback
```

### Access Control Options

Control who can access your portal:

| Variable | Effect |
|----------|--------|
| *(none)* | Any Google account can sign in |
| `ALLOWED_EMAIL_DOMAIN=company.com` | Only `@company.com` emails allowed |
| `ALLOWED_EMAILS=a@x.com,b@y.com` | Only listed emails allowed |

Example configurations:

```bash
# Single user (personal server)
ALLOWED_EMAILS=your.email@gmail.com

# Organization (team/company)
ALLOWED_EMAIL_DOMAIN=yourcompany.com

# Public access (like txcl.io)
# Don't set either variable
```
