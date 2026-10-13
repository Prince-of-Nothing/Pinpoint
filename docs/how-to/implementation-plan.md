# Implementation Plan: Always Together MVP

## Executive Summary

This document outlines the implementation roadmap for the Always Together MVP based on completed architectural documentation. The project has comprehensive specifications but requires actual development to meet internship deliverables.

**Current Status:** Documentation complete (Tasks 1-2), Implementation pending (Tasks 3-9)

**Timeline:** 8 weeks for functional MVP with security controls

---

## Phase 1: Backend Foundation (Weeks 1-2)

### Week 1: Project Setup and Database Layer

#### Day 1-2: Repository Structure and Docker Environment
- [ ] Create backend directory structure following modular monolith pattern
- [ ] Initialize FastAPI project with SQLAlchemy and Alembic
- [ ] Configure Docker Compose for PostgreSQL, Redis, and API service
- [ ] Set up environment variable management (.env templates)
- [ ] Configure GitHub Actions CI/CD pipeline

#### Day 3-4: Database Schema and Migrations
- [ ] Implement core tables: `users`, `user_handles`, `profiles`, `devices`, `sessions`
- [ ] Implement relationship tables: `friend_relationships`, `consent_grants`, `location_shares`
- [ ] Implement location tables: `location_updates`, `check_ins`
- [ ] Implement audit table: `audit_events`
- [ ] Create Alembic migration scripts with rollback capability
- [ ] Write database seeding scripts for development

#### Day 5: Database Security Hardening
- [ ] Create separate database roles: migration_owner, runtime_app, backup_operator
- [ ] Restrict runtime account privileges (no schema changes, no table creation)
- [ ] Implement connection pooling with SSL
- [ ] Configure encrypted backups

### Week 2: Authentication and Session Management

#### Day 1-2: User Registration and Password Handling
- [ ] Implement user registration endpoint with email validation
- [ ] Integrate Argon2id password hashing with proper salt generation
- [ ] Implement duplicate email rejection (case-insensitive, normalized)
- [ ] Add password strength validation
- [ ] Create transactional user creation (user + profile atomicity)

#### Day 3-4: OIDC/PKCE Authentication Flow
- [ ] Implement PKCE challenge generation endpoint
- [ ] Create token exchange endpoint with authorization code validation
- [ ] Generate short-lived access tokens (15 minutes)
- [ ] Implement refresh token rotation mechanism
- [ ] Add token revocation endpoint

#### Day 5: Session Management
- [ ] Implement device registration and session tracking
- [ ] Add session expiration (30-minute idle, 8-hour absolute)
- [ ] Create logout endpoint that invalidates server-side sessions
- [ ] Implement "get current user" endpoint with proper filtering

---

## Phase 2: Core API Development (Weeks 3-4)

### Week 3: Friendship and Authorization System

#### Day 1-2: Friendship Management
- [ ] Implement send friendship request endpoint
- [ ] Create accept/decline/revoke friendship endpoints
- [ ] Add block user functionality
- [ ] Implement friendship status checking
- [ ] Add rate limiting (10 requests per IP per 60 seconds)

#### Day 3-4: Directional Authorization Engine
- [ ] Build authorization middleware for location share validation
- [ ] Implement consent grant verification on every location read
- [ ] Create precision-based data filtering (exact vs approximate)
- [ ] Add expiry and revocation checks
- [ ] Implement IDOR prevention tests

#### Day 5: Profile Privacy Controls
- [ ] Create profile endpoint with viewer-based filtering
- [ ] Implement self/friend/non-friend response differentiation
- [ ] Add blocked user handling
- [ ] Ensure no location/activity leakage through profile

### Week 4: Location Sharing and Check-Ins

#### Day 1-2: Location Share Management
- [ ] Implement create location share endpoint
- [ ] Add precision selection (exact/approximate)
- [ ] Implement purpose and expiry fields
- [ ] Create revoke location share endpoint
- [ ] Add location share status endpoint

#### Day 3-4: Location Update Processing
- [ ] Create endpoint for submitting location updates
- [ ] Implement battery level and accuracy tracking
- [ ] Add timestamp validation and stale-state detection
- [ ] Build latest locations endpoint with authorization filtering
- [ ] Implement WebSocket gateway for real-time updates (optional for MVP)

#### Day 5: Check-In System
- [ ] Implement create check-in endpoint (manual/scheduled/trip)
- [ ] Add check-in status endpoint
- [ ] Create complete/cancel/acknowledge endpoints
- [ ] Build background job for reminder scheduling (Celery + Redis)
- [ ] Implement grace period logic

---

## Phase 3: Mobile Client Development (Weeks 5-6)

### Week 5: Flutter Foundation and Authentication

#### Day 1-2: Project Setup
- [ ] Initialize Flutter project with clean architecture
- [ ] Configure Android and iOS platform-specific settings
- [ ] Set up dependency injection (GetIt or Riverpod)
- [ ] Create network layer with Dio HTTP client
- [ ] Implement secure token storage (flutter_secure_storage)

#### Day 3-4: Authentication UI
- [ ] Build login screen with PKCE flow integration
- [ ] Create registration screen with form validation
- [ ] Implement token refresh logic
- [ ] Add session management (logout, device listing)
- [ ] Handle authentication errors gracefully

#### Day 5: Navigation and State Management
- [ ] Set up GoRouter or similar navigation solution
- [ ] Create authentication guard for protected routes
- [ ] Implement global state management for user context
- [ ] Build loading and error state handlers

### Week 6: Core Features Implementation

#### Day 1-2: Friend Management
- [ ] Build friend list screen
- [ ] Create send/accept/decline friendship request UI
- [ ] Implement friend search functionality
- [ ] Add block user confirmation dialogs
- [ ] Display friendship status indicators

#### Day 3-4: Location Sharing Interface
- [ ] Create location sharing initiation screen
- [ ] Build precision selector (exact/approximate)
- [ ] Implement expiry time picker
- [ ] Add active shares management screen
- [ ] Create revoke share confirmation flow

#### Day 5: Map Integration
- [ ] Integrate OpenStreetMap with flutter_map package
- [ ] Display authorized friends on map with appropriate markers
- [ ] Show accuracy circles and stale-state indicators
- [ ] Implement deep link generation for external navigation
- [ ] Add map attribution compliance

---

## Phase 4: Security Testing and Documentation (Weeks 7-8)

### Week 7: Security Control Implementation and Testing

#### Day 1-2: Input Validation and API Security
- [ ] Implement Pydantic validators for all endpoints
- [ ] Add coordinate validation (lat/lng bounds)
- [ ] Test SQL injection prevention (parameterized queries)
- [ ] Validate pagination parameters
- [ ] Reject unknown JSON properties
- [ ] Test malformed input handling

#### Day 3-4: CSRF and Session Security
- [ ] Implement CSRF protection for state-changing endpoints
- [ ] Test token replay after logout
- [ ] Verify session invalidation on password change
- [ ] Test concurrent session limits
- [ ] Validate secure cookie attributes (if web companion exists)

#### Day 5: Rate Limiting and Account Lockout
- [ ] Implement login attempt rate limiting
- [ ] Add account lockout after 5 failed attempts
- [ ] Configure 15-minute lockout duration
- [ ] Return HTTP 429 with Retry-After header
- [ ] Test lockout bypass attempts

### Week 8: Documentation and MVP Preparation

#### Day 1-2: Security Evidence Collection
- [ ] Document each security control with:
  - Attack/problem addressed
  - Implementation location
  - Test commands and output
  - Observed results
  - Remaining limitations
- [ ] Capture screenshots of test results
- [ ] Record commit hashes for implemented features
- [ ] Create test date logs

#### Day 3-4: Final Documentation Updates
- [ ] Update architecture diagrams with as-built information
- [ ] Document API endpoints with actual request/response examples
- [ ] Create deployment instructions
- [ ] Write testing instructions for mentor review
- [ ] Document known limitations and deferred features

#### Day 5: MVP Demonstration Preparation
- [ ] Prepare live demo script
- [ ] Create presentation slides covering:
  - Architecture overview
  - Main functionality demonstration
  - Security measures implemented
  - Testing evidence
  - Future development plan (October-December)
- [ ] Rehearse demonstration flow
- [ ] Prepare backup recordings in case of live demo issues

---

## Security Controls Implementation Matrix

| Control ID | Control Name | Implementation Week | Status | Evidence Required |
|------------|--------------|---------------------|--------|-------------------|
| SC-01 | Authentication | Week 2 | Pending | Login tests, token rotation logs |
| SC-02 | Directional Authorization | Week 3 | Pending | IDOR tests, consent verification logs |
| SC-03 | Private Profiles | Week 3 | Pending | Profile filtering tests |
| SC-04 | Input/API Security | Week 7 | Pending | Injection tests, validation error responses |
| SC-05 | Session Security | Week 2, 7 | Pending | Logout invalidation, CSRF tests |
| SC-06 | Data Protection | Week 1, 2 | Pending | Encryption config, backup tests |
| SC-07 | Stale-State Safety | Week 4 | Pending | Battery simulation, offline tests |
| SC-08 | Abuse Reporting | Deferred | Not in MVP | N/A |
| SC-09 | Incident Response | Week 8 | Pending | Tabletop exercise documentation |

---

## Required Deliverables by Task

### Task 3: System Design Completion
- [x] Architecture documented (already complete)
- [x] Technology stack selected (already complete)
- [ ] **Project structure created** (Week 1)
- [ ] **Database schema implemented** (Week 1)
- [ ] **API structure defined in code** (Week 2)

### Task 4: Security Prioritization
- [x] Security controls identified (already complete)
- [ ] **Security mechanisms implemented for MVP**:
  - [ ] Secure authentication (Week 2)
  - [ ] Password hashing (Week 2)
  - [ ] RBAC/Authorization (Week 3)
  - [ ] Input validation (Week 7)
  - [ ] Session management (Week 2)
  - [ ] Rate limiting (Week 3, 7)

### Task 5: MVP Functionality
- [ ] User registration and login (Week 2)
- [ ] Friendship management (Week 3)
- [ ] Location sharing (Week 4)
- [ ] Check-in system (Week 4)
- [ ] Map display (Week 6)
- [ ] Mobile app (Weeks 5-6)

### Task 6: Security Controls
- [ ] Secure authentication with PKCE (Week 2)
- [ ] Argon2id password hashing (Week 2)
- [ ] Server-side RBAC enforcement (Week 3)
- [ ] Input validation on all endpoints (Week 7)
- [ ] JWT/session handling with rotation (Week 2)
- [ ] API rate limiting (Week 3, 7)
- [ ] Account lockout (Week 7)

### Task 7: Security Testing
- [ ] Test authentication flows (Week 7)
- [ ] Test authorization boundaries (Week 7)
- [ ] Test input validation (Week 7)
- [ ] Test session security (Week 7)
- [ ] Document test results with evidence (Week 8)

### Task 8: Documentation
- [ ] Update architecture documentation with as-built details (Week 8)
- [ ] Document security implementations (Week 8)
- [ ] Create development plan for October-December (Week 8)
- [ ] Prepare MVP presentation (Week 8)

### Task 9: MVP Demonstration
- [ ] Working backend API (end of Week 4)
- [ ] Working mobile client (end of Week 6)
- [ ] Security controls demonstrated (Week 8)
- [ ] Live demo or recording (Week 8)

---

## Risk Mitigation

### Technical Risks
1. **Background location permissions on iOS**: Start with Android-first approach, document iOS limitations
2. **WebSocket complexity**: Defer realtime updates to post-MVP, use polling for MVP
3. **OAuth/OIDC complexity**: Use established libraries (Authlib), consider simplified token auth for MVP if blocked

### Timeline Risks
1. **Mobile development learning curve**: Allocate extra time in Week 5 for Flutter onboarding
2. **Security testing depth**: Focus on critical controls first (SC-01, SC-02, SC-05)
3. **Integration issues**: Build backend-first, mock API responses for mobile development

### Scope Risks
1. **Feature creep**: Strictly adhere to REQ-01 through REQ-09
2. **Deferred features temptation**: Document ideas but do not implement until MVP complete
3. **Perfectionism**: Prioritize working software over perfect code for internship timeline

---

## Success Criteria

The MVP will be considered complete when:

1. ✅ A user can register and log in securely
2. ✅ A user can send and manage friendship requests
3. ✅ A user can share location directionally with a friend
4. ✅ A friend can view shared location on a map
5. ✅ Location sharing respects precision, expiry, and revocation
6. ✅ Non-friends cannot access location data (IDOR prevention verified)
7. ✅ All endpoints validate input and return safe errors
8. ✅ Sessions expire and can be revoked
9. ✅ Documentation includes security evidence for each control
10. ✅ Mentor can reproduce tests from documentation

---

## Next Steps (Immediate)

1. **Create backend project structure** (today)
2. **Set up Docker development environment** (today)
3. **Implement database models and first migration** (this week)
4. **Begin authentication implementation** (next week)

**Remember:** Perfect documentation does not replace working software. Start coding now.
