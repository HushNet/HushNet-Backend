# 🐳 Docker Deployment Guide

Complete guide for deploying HushNet Backend with Docker.

---

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Docker Compose Setup](#docker-compose-setup)
- [Environment Variables](#environment-variables)
- [Building Images](#building-images)
- [Running Services](#running-services)
- [Database Management](#database-management)
- [Monitoring & Logs](#monitoring--logs)
- [Troubleshooting](#troubleshooting)
- [Production Deployment](#production-deployment)

---

## Overview

HushNet Backend can be deployed using Docker with two services:

1. **PostgreSQL Database** - Stores all application data
2. **HushNet Backend** - Rust API server

The setup uses Docker Compose for orchestration with health checks and automatic restarts.

---

## Prerequisites

- **Docker** 20.10+ ([Install](https://docs.docker.com/get-docker/))
- **Docker Compose** 2.0+ ([Install](https://docs.docker.com/compose/install/))
- 2GB RAM minimum
- 10GB disk space

### Check Installation

```bash
docker --version
docker compose version
```

---

## Quick Start

### 1. Clone Repository

```bash
git clone https://github.com/HushNet/HushNet-Backend.git
cd HushNet-Backend
```

### 2. Configure Environment

```bash
# Copy example environment file
cp .env.example .env

# Edit with your settings (optional)
nano .env
```

### 3. Start Services

```bash
# Build and start all services
docker compose up -d

# View logs
docker compose logs -f
```

### 4. Verify Deployment

```bash
# Check service health
docker compose ps

# Test API
curl http://localhost:8080/
```

**Expected response**:
```json
{
  "message": "Hello from HushNet Backend",
  "version": "0.1.0",
  "status": "healthy"
}
```

---

## Docker Compose Setup

### Architecture

```
┌─────────────────────────────────────────┐
│          Docker Network (hushnet)        │
│                                          │
│  ┌──────────────┐      ┌──────────────┐│
│  │  PostgreSQL  │◄─────┤   Backend    ││
│  │   :5432      │      │   :8080      ││
│  └──────┬───────┘      └──────┬───────┘│
│         │                     │         │
└─────────┼─────────────────────┼─────────┘
          │                     │
          │                     │
    ┌─────▼─────┐         ┌────▼─────┐
    │  Volume   │         │   Host   │
    │postgres_  │         │:8080     │
    │   data    │         │          │
    └───────────┘         └──────────┘
```

### Services Overview

| Service | Image | Port | Description |
|---------|-------|------|-------------|
| `postgres` | postgres:16-alpine | 5432 | PostgreSQL database |
| `backend` | Built from Dockerfile | 8080 | HushNet API server |

---

## Environment Variables

### `.env` File Structure

```bash
# Database Configuration
POSTGRES_USER=postgres           # PostgreSQL username
POSTGRES_PASSWORD=dev            # PostgreSQL password (CHANGE IN PRODUCTION!)
POSTGRES_DB=e2ee                # Database name
POSTGRES_PORT=5432              # Host port for PostgreSQL

# Backend Configuration
BACKEND_PORT=8080               # Host port for backend
SERVER_HOST=0.0.0.0            # Backend bind address
SERVER_PORT=8080               # Backend internal port

# Database URL (used by backend)
DATABASE_URL=postgresql://postgres:dev@postgres:5432/e2ee

# Logging Level
RUST_LOG=info                   # Options: error, warn, info, debug, trace
```

### Security Considerations

⚠️ **Production**: Change default passwords!

```bash
# Generate secure password
POSTGRES_PASSWORD=$(openssl rand -base64 32)
```

---

## Building Images

### Build Backend Image

```bash
# Build with default tag
docker compose build backend

# Build with custom tag
docker build -t hushnet-backend:latest .

# Build with specific target
docker build --target builder -t hushnet-builder .
```

### Multi-Stage Build Details

The Dockerfile uses a **multi-stage build**:

1. **Builder Stage** (`rust:1.75-slim`)
   - Compiles Rust code
   - Produces optimized binary
   - ~2GB image size

2. **Runtime Stage** (`debian:bookworm-slim`)
   - Only includes binary + runtime deps
   - ~150MB final image size
   - Runs as non-root user

### Build Arguments

```bash
# Build with custom Rust version
docker build --build-arg RUST_VERSION=1.76 .
```

---

## Running Services

### Start All Services

```bash
# Start in detached mode
docker compose up -d

# Start with build
docker compose up -d --build

# Start specific service
docker compose up -d postgres
```

### Stop Services

```bash
# Stop all services
docker compose stop

# Stop and remove containers
docker compose down

# Stop and remove volumes (⚠️ deletes data!)
docker compose down -v
```

### Restart Services

```bash
# Restart all
docker compose restart

# Restart specific service
docker compose restart backend
```

### Scale Services

```bash
# Run multiple backend instances (requires load balancer)
docker compose up -d --scale backend=3
```

---

## Database Management

### Access PostgreSQL

```bash
# Via docker exec
docker compose exec postgres psql -U postgres -d e2ee

# Via psql client (if installed)
psql -h localhost -U postgres -d e2ee
```

### Run SQL Scripts

```bash
# Execute SQL file
docker compose exec -T postgres psql -U postgres -d e2ee < script.sql

# Run inline SQL
docker compose exec postgres psql -U postgres -d e2ee -c "SELECT COUNT(*) FROM users;"
```

### Backup Database

```bash
# Create backup
docker compose exec postgres pg_dump -U postgres e2ee > backup_$(date +%Y%m%d).sql

# Restore backup
docker compose exec -T postgres psql -U postgres -d e2ee < backup.sql
```

### Database Migrations

```bash
# Apply seed.sql (runs automatically on first start)
docker compose exec postgres psql -U postgres -d e2ee -f /docker-entrypoint-initdb.d/seed.sql
```

---

## Monitoring & Logs

### View Logs

```bash
# All services
docker compose logs -f

# Specific service
docker compose logs -f backend

# Last 100 lines
docker compose logs --tail=100 backend

# Since specific time
docker compose logs --since 2024-01-01T10:00:00 backend
```

### Check Service Status

```bash
# List containers
docker compose ps

# Show resource usage
docker stats

# View service health
docker compose ps --format json | jq
```

### Health Checks

Both services have health checks:

```yaml
# PostgreSQL health check
healthcheck:
  test: ["CMD-SHELL", "pg_isready -U postgres"]
  interval: 10s
  timeout: 5s
  retries: 5

# Backend health check
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:8080/"]
  interval: 30s
  timeout: 3s
  retries: 3
```

Check health status:

```bash
docker inspect --format='{{.State.Health.Status}}' hushnet-backend
```

---

## Troubleshooting

### Common Issues

#### 1. Port Already in Use

**Error**: `Bind for 0.0.0.0:8080 failed: port is already allocated`

**Solution**:
```bash
# Change port in .env
BACKEND_PORT=8081

# Or stop conflicting service
lsof -ti:8080 | xargs kill -9  # macOS/Linux
netstat -ano | findstr :8080   # Windows
```

#### 2. Database Connection Failed

**Error**: `could not connect to server: Connection refused`

**Solution**:
```bash
# Check if PostgreSQL is healthy
docker compose ps postgres

# Wait for PostgreSQL to be ready
docker compose logs postgres

# Restart backend after PostgreSQL is ready
docker compose restart backend
```

#### 3. Build Failed

**Error**: `failed to solve: failed to compute cache key`

**Solution**:
```bash
# Clear Docker cache
docker compose build --no-cache

# Remove old images
docker system prune -a
```

#### 4. Permission Denied

**Error**: `permission denied while trying to connect to the Docker daemon socket`

**Solution**:
```bash
# Add user to docker group (Linux)
sudo usermod -aG docker $USER
newgrp docker

# Or run with sudo
sudo docker compose up -d
```

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug docker compose up

# Enable trace logging (very verbose)
RUST_LOG=trace docker compose up
```

### Inspect Container

```bash
# Open shell in backend container
docker compose exec backend bash

# Open shell in postgres container
docker compose exec postgres bash

# View environment variables
docker compose exec backend env
```

---

## Production Deployment

### Security Hardening

1. **Change Default Passwords**

```bash
# Generate secure credentials
POSTGRES_PASSWORD=$(openssl rand -base64 32)
```

2. **Use Docker Secrets** (Docker Swarm)

```yaml
secrets:
  postgres_password:
    external: true

services:
  postgres:
    secrets:
      - postgres_password
    environment:
      POSTGRES_PASSWORD_FILE: /run/secrets/postgres_password
```

3. **Limit Container Resources**

```yaml
services:
  backend:
    deploy:
      resources:
        limits:
          cpus: '1.0'
          memory: 512M
        reservations:
          cpus: '0.5'
          memory: 256M
```

### Reverse Proxy Setup

Use **Nginx** or **Traefik** for:
- HTTPS/TLS termination
- Load balancing
- Rate limiting

**Example with Nginx**:

```nginx
server {
    listen 443 ssl http2;
    server_name api.hushnet.com;

    ssl_certificate /etc/ssl/certs/fullchain.pem;
    ssl_certificate_key /etc/ssl/private/privkey.pem;

    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

### Monitoring

Add monitoring stack:

```yaml
services:
  prometheus:
    image: prom/prometheus
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9090:9090"

  grafana:
    image: grafana/grafana
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
```

### Automated Backups

```bash
#!/bin/bash
# backup.sh

DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="/backups"

# Backup database
docker compose exec -T postgres pg_dump -U postgres e2ee > "$BACKUP_DIR/db_$DATE.sql"

# Compress
gzip "$BACKUP_DIR/db_$DATE.sql"

# Delete backups older than 7 days
find "$BACKUP_DIR" -name "db_*.sql.gz" -mtime +7 -delete

echo "Backup completed: db_$DATE.sql.gz"
```

Add to crontab:
```bash
# Run daily at 2 AM
0 2 * * * /path/to/backup.sh
```

### High Availability

For production HA setup:

```yaml
services:
  backend:
    deploy:
      replicas: 3
      update_config:
        parallelism: 1
        delay: 10s
      restart_policy:
        condition: on-failure
        max_attempts: 3

  postgres:
    # Use managed PostgreSQL (AWS RDS, Google Cloud SQL)
    # Or setup PostgreSQL replication
```

---

## Docker Commands Cheat Sheet

```bash
# Build & Start
docker compose up -d --build        # Build and start services
docker compose up --force-recreate  # Recreate containers

# Stop & Remove
docker compose stop                 # Stop services
docker compose down                 # Stop and remove containers
docker compose down -v              # Stop and remove volumes

# Logs & Monitoring
docker compose logs -f              # Follow logs
docker compose logs --tail=100      # Last 100 lines
docker compose ps                   # List services
docker stats                        # Resource usage

# Execute Commands
docker compose exec backend bash    # Open shell
docker compose exec postgres psql   # Open PostgreSQL

# Maintenance
docker system prune -a             # Remove unused data
docker volume prune                # Remove unused volumes
docker compose build --no-cache    # Rebuild from scratch
```

---

## Resources

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose Documentation](https://docs.docker.com/compose/)
- [PostgreSQL Docker Image](https://hub.docker.com/_/postgres)
- [Best Practices for Writing Dockerfiles](https://docs.docker.com/develop/develop-images/dockerfile_best-practices/)

---

[← Back to Main Documentation](../README.md)
