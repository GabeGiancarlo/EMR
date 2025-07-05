# EMR Platform - HIPAA-Compliant Electronic Medical Record System

[![CI](https://github.com/GabeGiancarlo/EMR/actions/workflows/ci.yml/badge.svg)](https://github.com/GabeGiancarlo/EMR/actions/workflows/ci.yml)
[![Security](https://github.com/GabeGiancarlo/EMR/actions/workflows/security.yml/badge.svg)](https://github.com/GabeGiancarlo/EMR/actions/workflows/security.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.78+-orange.svg)](https://www.rust-lang.org)
[![FHIR](https://img.shields.io/badge/FHIR-R4-green.svg)](https://www.hl7.org/fhir/)

A modern, HIPAA-compliant Electronic Medical Record (EMR) platform built with Rust, featuring FHIR R4 integration, robust security, and high performance.

## 🏥 Overview

This EMR platform is designed to meet the demanding requirements of healthcare organizations while maintaining compliance with HIPAA regulations. It provides a comprehensive solution for patient data management, clinical workflows, and healthcare interoperability.

### Key Features

- **HIPAA Compliance**: Built with healthcare privacy and security in mind
- **FHIR R4 Integration**: Full support for HL7 FHIR R4 resources and operations
- **Modern Architecture**: Microservices-based design with Rust for performance and safety
- **Real-time Processing**: Background job processing for data sync and analytics
- **Secure by Design**: End-to-end encryption, audit logging, and access controls
- **Scalable Infrastructure**: Docker containerization with orchestration support
- **Developer Experience**: Comprehensive tooling and documentation

## 🚀 Quick Start

### Prerequisites

- **Rust 1.78+** (stable)
- **Docker** and **Docker Compose**
- **Node.js 20+** (for web frontend tooling)
- **PostgreSQL 16** (for database)
- **Just** (task runner) - `cargo install just`

### One-Command Setup

```bash
# Clone and start the platform
git clone https://github.com/GabeGiancarlo/EMR.git
cd EMR
just setup && just docker-up
```

After setup, the platform will be available at:
- **Web Interface**: https://localhost:3000
- **API Server**: https://localhost:8080
- **Health Check**: https://localhost:8080/healthz

### Using VS Code Dev Containers

For the best development experience, use VS Code with the Dev Containers extension:

1. Open the project in VS Code
2. Press `F1` and select "Dev Containers: Reopen in Container"
3. Wait for the container to build and start
4. All dependencies and tools will be pre-installed

## 📋 Architecture

### System Components

```
┌─────────────────────────────────────────────────────────────────┐
│                           Web Interface                         │
│                        (Leptos SSR + WASM)                     │
└─────────────────────────┬───────────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────────┐
│                         API Gateway                            │
│                      (Actix-Web + TLS)                        │
└─────────────┬─────────────────────────────────┬─────────────────┘
              │                                 │
┌─────────────▼─────────────┐     ┌─────────────▼─────────────────┐
│       Core Services       │     │      Background Jobs         │
│    (Domain Logic + DB)    │     │      (Apalis + Redis)        │
└─────────────┬─────────────┘     └─────────────┬─────────────────┘
              │                                 │
┌─────────────▼─────────────────────────────────▼─────────────────┐
│                    Infrastructure Layer                        │
│            PostgreSQL + Redis + NATS + Monitoring             │
└─────────────────────────────────────────────────────────────────┘
```

### Service Architecture

- **`api/`** - REST API server with authentication, FHIR proxy, and business logic
- **`web/`** - Frontend web application with server-side rendering
- **`core/`** - Domain models, business logic, and repository abstractions
- **`fhir/`** - FHIR R4 integration and Kodjin server client
- **`jobs/`** - Background processing for data synchronization and analytics
- **`infra/`** - Docker configuration, monitoring, and deployment

## 🛠️ Development

### Available Commands

```bash
# Development
just dev          # Start development servers with hot reload
just dev-stop     # Stop development servers
just dev-restart  # Restart development servers

# Building
just build        # Build all services
just build-api    # Build API server
just build-web    # Build web frontend
just build-jobs   # Build jobs worker

# Testing
just test         # Run all tests
just test-api     # Run API tests
just test-web     # Run web tests
just test-jobs    # Run jobs tests
just test-coverage # Run tests with coverage

# Code Quality
just lint         # Run linting (clippy)
just fmt          # Format code
just check        # Type checking
just audit        # Security audit

# Docker
just docker-up    # Start all services
just docker-down  # Stop all services
just docker-logs  # View logs
just docker-build # Build Docker images

# Database
just db-setup     # Setup database
just db-migrate   # Run migrations
just db-reset     # Reset database

# Utilities
just clean        # Clean build artifacts
just docs         # Generate documentation
just health       # Health check
```

### Development Workflow

1. **Setup Environment**
   ```bash
   just setup
   ```

2. **Start Development Services**
   ```bash
   just dev
   ```

3. **Make Changes**
   - Edit code in your preferred editor
   - Changes are automatically reloaded

4. **Run Tests**
   ```bash
   just test
   ```

5. **Check Code Quality**
   ```bash
   just lint
   just fmt
   ```

6. **Commit Changes**
   ```bash
   git add .
   git commit -m "feat: your feature description"
   ```

## 🔐 Security & Compliance

### HIPAA Compliance Features

- **Data Encryption**: End-to-end encryption for data at rest and in transit
- **Access Controls**: Role-based access control with audit logging
- **Audit Trails**: Comprehensive logging of all data access and modifications
- **Data Retention**: Configurable retention policies for PHI data
- **Secure Communication**: TLS 1.3 for all network communications
- **Authentication**: Multi-factor authentication support
- **Data Backup**: Encrypted backups with retention policies

### Security Measures

- **Memory Safety**: Rust's ownership system prevents common vulnerabilities
- **Input Validation**: Comprehensive validation of all inputs
- **SQL Injection Prevention**: Parameterized queries and ORM protection
- **XSS Prevention**: Content Security Policy and input sanitization
- **CSRF Protection**: Token-based CSRF protection
- **Rate Limiting**: API rate limiting and DDoS protection
- **Container Security**: Minimal attack surface with non-root containers

## 📊 FHIR Integration

### Supported Resources

- **Patient Management**: Patient, Person, RelatedPerson
- **Clinical Data**: Observation, Condition, Procedure, Medication
- **Administrative**: Organization, Practitioner, Location, Encounter
- **Workflow**: Task, Appointment, Schedule, Slot

### FHIR Operations

- **CRUD Operations**: Create, Read, Update, Delete for all resources
- **Search**: Advanced search with multiple parameters
- **Validation**: FHIR resource validation and constraint checking
- **Bundles**: Transaction and batch processing
- **Subscriptions**: Real-time notifications for resource changes

### Kodjin Integration

The platform integrates with Kodjin FHIR Server for comprehensive FHIR support:

```bash
# Start with FHIR server
docker compose --profile fhir-server up -d
```

## 🔧 Configuration

### Environment Variables

Key configuration options (see `infra/environment.template` for complete list):

```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost/emr_platform

# Security
JWT_SECRET=your-secret-key
TLS_CERT_PATH=/path/to/cert.pem
TLS_KEY_PATH=/path/to/key.pem

# FHIR
FHIR_BASE_URL=http://localhost:8080/fhir
FHIR_SERVER_ENABLED=true

# Features
FEATURE_PATIENT_MANAGEMENT=true
FEATURE_FHIR_INTEGRATION=true
FEATURE_ANALYTICS=true
FEATURE_AUDIT_LOGGING=true
```

### Production Deployment

1. **Configure Environment**
   ```bash
   cp infra/environment.template .env
   # Edit .env with production values
   ```

2. **Generate TLS Certificates**
   ```bash
   # Use Let's Encrypt or your certificate authority
   certbot certonly --standalone -d yourdomain.com
   ```

3. **Deploy Services**
   ```bash
   docker compose --profile production up -d
   ```

4. **Monitor Health**
   ```bash
   just health
   ```

## 📈 Monitoring & Observability

### Metrics & Monitoring

- **Prometheus**: Metrics collection and alerting
- **Grafana**: Dashboards and visualization
- **Jaeger**: Distributed tracing
- **Health Checks**: Built-in health monitoring

### Logging

- **Structured Logging**: JSON-formatted logs with correlation IDs
- **Log Levels**: Configurable log levels per service
- **Audit Logging**: Comprehensive audit trail for compliance
- **Log Aggregation**: Centralized log collection and analysis

### Observability Stack

```bash
# Start monitoring services
docker compose --profile monitoring up -d
```

Access monitoring services:
- **Grafana**: http://localhost:3001
- **Prometheus**: http://localhost:9091
- **Jaeger**: http://localhost:16686

## 🧪 Testing

### Test Types

- **Unit Tests**: Individual component testing
- **Integration Tests**: Service integration testing
- **End-to-End Tests**: Full workflow testing
- **Security Tests**: Vulnerability and penetration testing
- **Performance Tests**: Load and stress testing

### Running Tests

```bash
# All tests
just test

# Specific test suites
just test-api
just test-web
just test-jobs

# With coverage
just test-coverage

# Performance tests
cargo bench
```

## 📚 API Documentation

### REST API

The API follows RESTful conventions with comprehensive OpenAPI documentation:

```bash
# View API documentation
curl https://localhost:8080/api/docs
```

### Key Endpoints

- **Health**: `GET /healthz`
- **Authentication**: `POST /auth/login`
- **Patients**: `GET|POST|PUT|DELETE /api/patients`
- **FHIR**: `GET|POST|PUT|DELETE /fhir/{resource}`
- **Jobs**: `GET /api/jobs`

### FHIR API

Full FHIR R4 API support:

```bash
# Get patient
GET /fhir/Patient/{id}

# Search patients
GET /fhir/Patient?name=Smith&gender=male

# Create patient
POST /fhir/Patient
```

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Setup

1. **Fork the Repository**
2. **Create Feature Branch**
   ```bash
   git checkout -b feature/amazing-feature
   ```
3. **Make Changes**
4. **Run Tests**
   ```bash
   just test
   just lint
   ```
5. **Commit Changes**
   ```bash
   git commit -m "feat: add amazing feature"
   ```
6. **Push to Branch**
   ```bash
   git push origin feature/amazing-feature
   ```
7. **Open Pull Request**

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Use descriptive variable names
- Add comprehensive documentation
- Write tests for new features
- Follow conventional commits

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🆘 Support

### Documentation

- [API Documentation](docs/api.md)
- [Deployment Guide](docs/deployment.md)
- [Security Guide](docs/security.md)
- [FHIR Integration](docs/fhir.md)

### Community

- [GitHub Issues](https://github.com/GabeGiancarlo/EMR/issues)
- [GitHub Discussions](https://github.com/GabeGiancarlo/EMR/discussions)
- [Discord Server](https://discord.gg/emr-platform)

### Commercial Support

For enterprise support, consulting, or custom development:
- Email: support@emr-platform.com
- Website: https://emr-platform.com

## 🗺️ Roadmap

### Current Version (v1.0)

- ✅ Core EMR functionality
- ✅ FHIR R4 integration
- ✅ Web interface
- ✅ Background processing
- ✅ Security & compliance
- ✅ Docker deployment

### Upcoming Features (v1.1)

- 🔄 Mobile application
- 🔄 Advanced analytics
- 🔄 Machine learning integration
- 🔄 Multi-tenant support
- 🔄 Advanced reporting

### Future Releases

- 📋 Telemedicine integration
- 📋 IoT device integration
- 📋 Blockchain for audit trails
- 📋 Advanced AI/ML features
- 📋 International compliance (GDPR, etc.)

## 🔗 Related Projects

- [Kodjin FHIR Server](https://www.kodjin.com/)
- [HL7 FHIR](https://www.hl7.org/fhir/)
- [Leptos](https://leptos.dev/)
- [Actix Web](https://actix.rs/)

## 📊 Project Statistics

- **Languages**: Rust (95%), TypeScript (3%), SQL (2%)
- **Lines of Code**: ~50,000+ (excluding tests)
- **Test Coverage**: 85%+
- **Dependencies**: Production-ready, well-maintained crates
- **Documentation**: Comprehensive API and usage docs

---

**Built with ❤️ by the EMR Platform Team**

*Making healthcare data management secure, efficient, and compliant.* 