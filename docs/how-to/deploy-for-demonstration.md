# How to Deploy for University Demonstration

This guide covers deploying Always Together for live demonstration during university assessment or internship review.

## Quick Deployment Options

### Option 1: Local Demo (Recommended for Grading)

For in-person demonstrations to evaluators, run everything locally:

```bash
# Start all services
cd docker/
docker-compose up -d

# Run migrations
docker-compose exec fastapi-backend python -m alembic upgrade head

# Seed demo data
docker-compose exec fastapi-backend python scripts/seed_dev_data.py

# Launch mobile app
cd ../mobile/
flutter run
```

**Advantages:**
- No internet dependency
- Full control over demo data
- Can show backend logs in real-time
- No deployment costs

### Option 2: Cloud Deployment (For Remote Review)

Deploy to a cloud provider for remote access by evaluators.

#### Prerequisites

- Docker Hub account (for container registry)
- Cloud provider account (AWS, GCP, or DigitalOcean)
- Domain name (optional, for HTTPS)

#### Step 1: Build and Push Containers

```bash
# Build backend image
docker build -t your-dockerhub/always-together-backend:latest ./backend

# Push to registry
docker push your-dockerhub/always-together-backend:latest
```

#### Step 2: Deploy to Cloud

**DigitalOcean App Platform (Simplest):**

1. Create new App Platform application
2. Connect GitHub repository
3. Configure services:
   - Backend: `docker-compose.yml`
   - Database: Managed PostgreSQL
   - Cache: Managed Redis
4. Set environment variables:
   ```
   DATABASE_URL=postgresql://...
   REDIS_URL=redis://...
   JWT_SECRET=<generate-random-secret>
   ENVIRONMENT=production
   ```
5. Deploy

**AWS ECS (More Control):**

```bash
# Use AWS Copilot for easy deployment
copilot init --app always-together \
  --type "Backend Service" \
  --dockerfile "./backend/Dockerfile" \
  --port 8000

copilot env deploy --name production
copilot svc deploy --name api --env production
```

#### Step 3: Configure Mobile App

Update Flutter environment for production API:

```bash
cd mobile/
cat > .env << EOF
API_BASE_URL=https://api.always-together-demo.example.com/api/v1
WEBSOCKET_URL=wss://api.always-together-demo.example.com/ws
OAUTH_CLIENT_ID=mobile-prod-client
ENVIRONMENT=production
EOF
```

Build release APK/IPA:

```bash
# Android
flutter build apk --release

# iOS
flutter build ios --release
```

## Pre-Demonstration Checklist

Before the demo, verify:

- [ ] All services are healthy (`/api/v1/health` returns 200)
- [ ] Database is seeded with demo accounts
- [ ] At least 3 location shares are active with different expiry times
- [ ] Test login works for all demo accounts
- [ ] Map displays friend locations correctly
- [ ] Check-in creation and completion works
- [ ] Friendship request flow works end-to-end
- [ ] Privacy settings are visible and functional
- [ ] Backend logs are accessible for showing audit trail
- [ ] Internet connection is stable (for cloud demo)

## Demo Accounts

Pre-configured accounts for demonstration:

| Username | Password | Scenario |
|----------|----------|----------|
| `demo_parent` | `Demo123!` | Parent monitoring child's location |
| `demo_child` | `Demo123!` | Child sharing location with parent |
| `demo_friend1` | `Demo123!` | Friend with approximate location sharing |
| `demo_friend2` | `Demo123!` | Friend with exact location sharing |
| `demo_blocked` | `Demo123!` | Demonstrating block functionality |

## Troubleshooting During Demo

### Issue: Map shows no locations

**Quick Fix:** Force location update:
```bash
curl -X POST http://localhost:8000/api/v1/admin/force_location_update \
  -H "Authorization: Bearer <admin-token>" \
  -d '{"user_id": "usr_demo123", "lat": 40.7128, "lng": -74.0060}'
```

### Issue: WebSocket connection fails

**Quick Fix:** Restart WebSocket service:
```bash
docker-compose restart websocket
```

### Issue: OAuth login hangs

**Quick Fix:** Use direct token for demo:
```bash
curl http://localhost:8000/api/v1/admin/demo_token?user=demo_parent
# Returns pre-generated access token
```

## Post-Demo Cleanup

For temporary demos, clean up resources:

```bash
# Local demo
docker-compose down -v

# Cloud demo (DigitalOcean)
doctl apps delete <app-id>

# Cloud demo (AWS)
copilot app delete --name always-together
```

## Security Note for Demos

**Important:** Demo deployments should NOT use real user data. All demo accounts are fictional and isolated from production systems. Never demonstrate with actual personal location data during assessment.

---

*For local development setup, see [Local Development Setup](../tutorials/local-development-setup.md).*  
*For production deployment architecture, see [Deployment View](../architecture/deployment-view.md).*
