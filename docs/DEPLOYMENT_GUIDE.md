# 🚀 Synapsis Deployment Guide

**Purpose:** Deploy Synapsis to production environments

**Last Updated:** 2026-03-27

---

## 📋 Table of Contents

1. [Prerequisites](#prerequisites)
2. [Docker Deployment](#docker-deployment)
3. [Kubernetes Deployment](#kubernetes-deployment)
4. [Bare Metal Deployment](#bare-metal-deployment)
5. [Configuration](#configuration)
6. [Monitoring](#monitoring)
7. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### System Requirements

- **CPU:** 2+ cores (4+ recommended)
- **RAM:** 4GB minimum (8GB recommended)
- **Storage:** 10GB free space
- **OS:** Linux (Ubuntu 20.04+, Debian 11+, RHEL 8+)

### Software Requirements

- **Rust:** 1.88+
- **Docker:** 20.10+ (for containerized deployment)
- **Kubernetes:** 1.25+ (for K8s deployment)
- **PostgreSQL:** 15+ (optional, for external database)

---

## Docker Deployment

### Quick Start

```bash
# Clone repository
git clone https://github.com/methodwhite/synapsis.git
cd synapsis

# Start all services (dev mode)
docker-compose up -d

# Check status
docker-compose ps

# View logs
docker-compose logs -f synapsis-dev
```

### Production Deployment

```bash
# Build production image
docker build -f Dockerfile.prod -t synapsis:latest .

# Run production container
docker run -d \
  --name synapsis \
  -p 7438:7438 \
  -v synapsis-data:/root/.local/share/synapsis \
  -e RUST_LOG=info \
  -e SYNAPSIS_SECURE_MODE=true \
  synapsis:latest
```

### Docker Compose Production

Create `docker-compose.prod.yml`:

```yaml
version: '3.8'

services:
  synapsis:
    image: synapsis:latest
    container_name: synapsis
    restart: unless-stopped
    ports:
      - "7438:7438"
    volumes:
      - synapsis-data:/root/.local/share/synapsis
      - ./config:/app/config
    environment:
      - RUST_LOG=warn
      - SYNAPSIS_SECURE_MODE=true
      - SYNAPSIS_DB_KEY=${SYNAPSIS_DB_KEY}
    networks:
      - synapsis-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:7438/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

networks:
  synapsis-network:
    driver: bridge

volumes:
  synapsis-data:
```

Deploy:

```bash
docker-compose -f docker-compose.prod.yml up -d
```

---

## Kubernetes Deployment

### Namespace and ConfigMap

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: synapsis
---
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: synapsis-config
  namespace: synapsis
data:
  RUST_LOG: "info"
  SYNAPSIS_SECURE_MODE: "true"
  SYNAPSIS_DATA_DIR: "/data/synapsis"
```

### Deployment

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: synapsis
  namespace: synapsis
spec:
  replicas: 3
  selector:
    matchLabels:
      app: synapsis
  template:
    metadata:
      labels:
        app: synapsis
    spec:
      containers:
      - name: synapsis
        image: synapsis:latest
        ports:
        - containerPort: 7438
          name: mcp
        volumeMounts:
        - name: data
          mountPath: /data/synapsis
        envFrom:
        - configMapRef:
            name: synapsis-config
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 7438
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 7438
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: synapsis-data
```

### Service

```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: synapsis
  namespace: synapsis
spec:
  selector:
    app: synapsis
  ports:
  - port: 7438
    targetPort: 7438
    name: mcp
  type: ClusterIP
```

### Deploy

```bash
# Apply manifests
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml

# Check status
kubectl get pods -n synapsis
kubectl get svc -n synapsis

# View logs
kubectl logs -f deployment/synapsis -n synapsis
```

---

## Bare Metal Deployment

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup install 1.88
rustup default 1.88
```

### Build from Source

```bash
# Clone repository
git clone https://github.com/methodwhite/synapsis.git
cd synapsis

# Build release
cargo build --release

# Install binary
sudo cp target/release/synapsis /usr/local/bin/
sudo cp target/release/synapsis-mcp /usr/local/bin/
```

### Systemd Service

Create `/etc/systemd/system/synapsis.service`:

```ini
[Unit]
Description=Synapsis MCP Server
After=network.target

[Service]
Type=simple
User=synapsis
Group=synapsis
ExecStart=/usr/local/bin/synapsis --tcp 7438
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info
Environment=SYNAPSIS_DATA_DIR=/var/lib/synapsis

[Install]
WantedBy=multi-user.target
```

Deploy:

```bash
# Create user
sudo useradd -r -s /bin/false synapsis

# Create data directory
sudo mkdir -p /var/lib/synapsis
sudo chown synapsis:synapsis /var/lib/synapsis

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable synapsis
sudo systemctl start synapsis

# Check status
sudo systemctl status synapsis
```

---

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Log level | `info` |
| `SYNAPSIS_DATA_DIR` | Data directory | `~/.local/share/synapsis` |
| `SYNAPSIS_SECURE_MODE` | Enable PQC security | `false` |
| `SYNAPSIS_DB_KEY` | SQLCipher encryption key | (required for encryption) |
| `SYNAPSIS_TCP_PORT` | TCP server port | `7438` |

### Configuration Files

#### `/etc/synapsis/config.toml`

```toml
[server]
tcp_port = 7438
bind_address = "0.0.0.0"
secure_mode = true

[security]
pqc_algorithm = "Kyber512"
challenge_response = true

[database]
path = "/var/lib/synapsis/synapsis.db"
encryption = true

[logging]
level = "info"
format = "json"
```

---

## Monitoring

### Prometheus Metrics

Synapsis exposes metrics at `/metrics`:

```bash
# Scrape metrics
curl http://localhost:7438/metrics

# Example metrics
synapsis_agents_active
synapsis_sessions_total
synapsis_tasks_pending
synapsis_requests_total
synapsis_request_duration_seconds
```

### Grafana Dashboard

Import dashboard from `docker/grafana/dashboards/synapsis.json`:

```bash
# Import via API
curl -X POST \
  -H "Content-Type: application/json" \
  -d @dashboard.json \
  http://admin:synapsis_admin@localhost:3000/api/dashboards/db
```

### Alerts

Configure alerts in Prometheus:

```yaml
groups:
  - name: synapsis
    rules:
      - alert: SynapsisDown
        expr: up{job="synapsis"} == 0
        for: 5m
        annotations:
          summary: "Synapsis instance is down"
          
      - alert: HighErrorRate
        expr: rate(synapsis_errors_total[5m]) > 0.1
        for: 5m
        annotations:
          summary: "High error rate detected"
```

---

## Troubleshooting

### Common Issues

#### Issue: Container won't start

```bash
# Check logs
docker-compose logs synapsis

# Check permissions
docker-compose exec synapsis ls -la /root/.local/share/synapsis
```

#### Issue: Database locked

```bash
# Stop service
docker-compose down

# Remove WAL files
rm /path/to/synapsis.db-*

# Restart
docker-compose up -d
```

#### Issue: High memory usage

```bash
# Check memory
docker stats synapsis

# Reduce concurrency
export SYNAPSIS_MAX_CONCURRENT=5
```

#### Issue: PQC handshake fails

```bash
# Verify secure mode
docker-compose exec synapsis env | grep SECURE

# Check Kyber implementation
./verify_kyber_real.sh
```

### Logs

```bash
# Docker logs
docker-compose logs -f synapsis

# Systemd logs
journalctl -u synapsis -f

# Application logs
tail -f /var/lib/synapsis/logs/synapsis.log
```

### Health Checks

```bash
# HTTP health check
curl http://localhost:7438/health

# TCP connection check
nc -zv localhost 7438

# Full verification
./verify_synapsis.sh
```

---

## Security Considerations

### Production Checklist

- [ ] Enable secure mode (`SYNAPSIS_SECURE_MODE=true`)
- [ ] Set strong database encryption key
- [ ] Configure firewall rules
- [ ] Enable TLS for external connections
- [ ] Rotate credentials regularly
- [ ] Monitor for suspicious activity
- [ ] Regular security audits
- [ ] Keep dependencies updated

### Firewall Rules

```bash
# Allow MCP port
sudo ufw allow 7438/tcp

# Allow Prometheus scraping (internal only)
sudo ufw allow from 10.0.0.0/8 to any port 9090

# Enable firewall
sudo ufw enable
```

---

## Performance Tuning

### Recommended Settings

```bash
# Increase file descriptors
ulimit -n 65536

# Set TCP parameters
sudo sysctl -w net.core.somaxconn=65535
sudo sysctl -w net.ipv4.tcp_max_syn_backlog=65535

# Increase memory limit
export RUST_MIN_STACK=8388608  # 8MB
```

### Resource Limits (Docker)

```yaml
services:
  synapsis:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 4G
        reservations:
          cpus: '1'
          memory: 2G
```

---

## Backup and Recovery

### Backup

```bash
# Stop service
docker-compose down

# Backup data
tar -czf synapsis-backup-$(date +%Y%m%d).tar.gz \
  /var/lib/synapsis

# Restart
docker-compose up -d
```

### Recovery

```bash
# Stop service
docker-compose down

# Restore data
tar -xzf synapsis-backup-YYYYMMDD.tar.gz -C /

# Restart
docker-compose up -d
```

---

## Support

- **Documentation:** https://github.com/methodwhite/synapsis/tree/main/docs
- **Issues:** https://github.com/methodwhite/synapsis/issues
- **Email:** methodwhite@proton.me

---

**Deployment Date:** 2026-03-27  
**Version:** 0.1.0  
**Status:** ✅ Production-Ready
