# Local Development Setup

This tutorial walks through setting up a complete local development environment for Always Together. Follow these steps to run the application locally for development or demonstration purposes.

## Prerequisites

Before starting, ensure you have:

- **Docker Desktop** (v20.10+) installed and running
- **Python 3.11+** installed locally
- **Flutter SDK** (v3.13+) installed and configured
- **Git** for version control
- **Node.js 18+** (optional, for some tooling)

## Step 1: Clone the Repository

```bash
git clone https://github.com/your-org/always-together.git
cd always-together
```

## Step 2: Start Backend Services with Docker

The backend requires PostgreSQL, Redis, and FastAPI. Use Docker Compose to start all services:

```bash
cd docker/
docker-compose up -d
```

This starts:
- **PostgreSQL** on port 5432
- **Redis** on port 6379
- **FastAPI backend** on port 8000

Verify services are running:

```bash
docker-compose ps
```

Expected output:
```
NAME                    STATUS         PORTS
postgres               Up (healthy)   5432/tcp
redis                  Up (healthy)   6379/tcp
fastapi-backend        Up             0.0.0.0:8000->8000/tcp
```

## Step 3: Run Database Migrations

```bash
docker-compose exec fastapi-backend python -m alembic upgrade head
```

This creates all required tables including:
- `users` — User accounts and profiles
- `friendships` — Bidirectional friend relationships
- `location_shares` — Directional consent records
- `location_updates` — Encrypted location data
- `check_ins` — Safety check-in records
- `audit_logs` — Privacy-preserving access logs

## Step 4: Seed Development Data (Optional)

For demonstration purposes, seed the database with test users:

```bash
docker-compose exec fastapi-backend python scripts/seed_dev_data.py
```

This creates:
- 5 test user accounts with pre-configured friendships
- Sample location shares with various expiry times
- Mock location updates for map visualization

## Step 5: Configure Mobile Environment

Create a Flutter environment configuration file:

```bash
cd mobile/
cat > .env << EOF
API_BASE_URL=http://localhost:8000/api/v1
WEBSOCKET_URL=ws://localhost:8000/ws
OAUTH_CLIENT_ID=mobile-dev-client
ENVIRONMENT=development
EOF
```

## Step 6: Run the Mobile App

### For Android:

```bash
cd mobile/
flutter pub get
flutter run
```

### For iOS:

```bash
cd mobile/
flutter pub get
cd ios/
pod install
cd ..
flutter run
```

## Step 7: Verify the Setup

Test the API is responding:

```bash
curl http://localhost:8000/api/v1/health
```

Expected response:
```json
{
  "status": "healthy",
  "database": "connected",
  "redis": "connected",
  "version": "0.1.0-mvp"
}
```

## Common Issues and Solutions

### Issue: Docker containers fail to start

**Solution:** Check if ports 5432, 6379, or 8000 are already in use:

```bash
lsof -i :5432
lsof -i :6379
lsof -i :8000
```

Stop conflicting services or change ports in `docker-compose.yml`.

### Issue: Flutter cannot connect to backend

**Solution:** For Android emulator, use `10.0.2.2` instead of `localhost`:

```bash
cat > .env << EOF
API_BASE_URL=http://10.0.2.2:8000/api/v1
WEBSOCKET_URL=ws://10.0.2.2:8000/ws
EOF
```

For iOS simulator, `localhost` works correctly.

### Issue: Database migration fails

**Solution:** Drop and recreate the database:

```bash
docker-compose down -v
docker-compose up -d
docker-compose exec fastapi-backend python -m alembic upgrade head
```

## Next Steps

After setup is complete:

1. **Try your first location share** — See [Your First Location Share](./your-first-location-share.md)
2. **Understand consent flows** — See [Understanding Consent Flows](./understanding-consent-flows.md)
3. **Configure privacy settings** — See [Privacy Settings Walkthrough](./privacy-settings-walkthrough.md)

## Teardown

To stop all services:

```bash
docker-compose down
```

To stop and remove all data:

```bash
docker-compose down -v
```

---

*For production deployment instructions, see [How to Deploy](../how-to/how-to-deploy.md).*
