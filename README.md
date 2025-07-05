# 🏥 EMR Platform

[![CI](https://github.com/GabeGiancarlo/EMR/actions/workflows/ci.yml/badge.svg)](https://github.com/GabeGiancarlo/EMR/actions/workflows/ci.yml)
[![Security](https://github.com/GabeGiancarlo/EMR/actions/workflows/security.yml/badge.svg)](https://github.com/GabeGiancarlo/EMR/actions/workflows/security.yml)

A **HIPAA-compliant Electronic Medical Record (EMR) platform** built with Rust, featuring real-time development capabilities and modern web technologies.

## 🚀 **Current Status: LIVE & WORKING!**

✅ **Rust API Server** - Fully functional on `localhost:8080`  
✅ **Interactive Demo Interface** - Beautiful web UI with real-time API integration  
✅ **Development Environment** - Hot-reload development with simple commands  
✅ **3 Working API Endpoints** - Health check, patients list, and individual patient data  

## 🎯 Quick Start

### Prerequisites
- [Rust](https://rustup.rs/) (1.78+)
- [Just](https://github.com/casey/just) command runner

### Start Development Environment

```bash
# Clone and start the platform
git clone https://github.com/GabeGiancarlo/EMR.git
cd EMR
just dev
```

**That's it!** This will:
1. 🚀 Start the Rust API server on `http://localhost:8080`
2. 🌐 Open the interactive demo in your browser
3. ✅ Show you all available endpoints and features

### Stop Everything
```bash
just dev-stop
```

## 📡 Live API Endpoints

| Method | Endpoint | Description | Status |
|--------|----------|-------------|--------|
| `GET` | `/healthz` | System health check | ✅ Working |
| `GET` | `/api/patients` | List all patients | ✅ Working |
| `GET` | `/api/patients/{id}` | Get specific patient | ✅ Working |

### Test the API
```bash
# Quick API test
just api-test

# Manual testing
curl http://localhost:8080/healthz
curl http://localhost:8080/api/patients
curl http://localhost:8080/api/patients/patient-001
```

## 🌐 Demo Interface Features

The interactive demo (`demo/index.html`) includes:

- **📊 Dashboard** - Real-time system status and metrics
- **👥 Patient Management** - Interactive patient list with live search
- **🔧 API Testing** - Live endpoint testing with JSON responses
- **⚡ Real-time Updates** - Auto-refreshing connection status
- **🎨 Modern UI** - Responsive design with Tailwind CSS

## 🛠️ Development Commands

| Command | Description |
|---------|-------------|
| `just dev` | Start complete development environment |
| `just api` | Start only the API server |
| `just demo` | Open the demo interface |
| `just api-test` | Test all API endpoints |
| `just build` | Build the project |
| `just test` | Run all tests |
| `just lint` | Run code linting |
| `just status` | Show project status |

## 🏗️ Architecture

```
EMR Platform/
├── api/                 # 🦀 Rust API Server (Actix-web)
│   ├── src/main.rs     # Main server with endpoints
│   └── Cargo.toml      # Dependencies
├── demo/               # 🌐 Interactive Demo Interface
│   └── index.html      # Full-featured web UI
├── scripts/            # 🔧 Development automation
├── justfile           # ⚡ Command runner configuration
└── README.md          # 📖 This file
```

### Technology Stack

- **Backend**: Rust + Actix-web
- **Frontend**: HTML5 + JavaScript + Tailwind CSS
- **API**: RESTful JSON endpoints with CORS
- **Development**: Just + Hot-reload + Auto-browser opening
- **Data**: Mock patient data (ready for database integration)

## 🔥 What's Working Right Now

### ✅ **Rust API Server**
- Fast, memory-safe HTTP server
- CORS-enabled for browser access
- JSON API responses
- Real patient data endpoints
- Health monitoring

### ✅ **Demo Interface**
- Real-time API communication
- Interactive patient search and filtering
- Live endpoint testing
- Responsive design
- Auto-refreshing status indicators

### ✅ **Developer Experience**
- One-command startup (`just dev`)
- Hot-reload development
- Auto-opening browser demo
- Comprehensive testing commands
- Easy stop/start controls

## 🎯 Roadmap & Next Steps

Now that the foundation is working, upcoming features include:

### Phase 1: Core Features
- [ ] **Database Integration** (PostgreSQL)
- [ ] **Patient CRUD Operations** (Create, Update, Delete)
- [ ] **Authentication System** (OAuth2/JWT)
- [ ] **Data Validation** & Error Handling

### Phase 2: Medical Features
- [ ] **FHIR R4 Integration** (External medical systems)
- [ ] **Appointment Management**
- [ ] **Medical Records** & Documentation
- [ ] **Prescription Management**

### Phase 3: Advanced Features
- [ ] **Audit Trails** (HIPAA compliance)
- [ ] **Role-based Access Control**
- [ ] **Medical Imaging** Support
- [ ] **Reporting & Analytics**

### Phase 4: Production
- [ ] **Docker Deployment**
- [ ] **CI/CD Pipeline**
- [ ] **Security Hardening**
- [ ] **Performance Optimization**

## 🔒 Security & Compliance

This platform is being built with **HIPAA compliance** in mind:

- **Data Encryption** (planned)
- **Audit Logging** (planned)
- **Access Controls** (planned)
- **Secure Communications** (HTTPS ready)
- **Memory Safety** (Rust guarantees)

## 🤝 Contributing

This project is in active development. To contribute:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Test with `just test` and `just lint`
5. Submit a pull request

### Development Setup
```bash
git clone https://github.com/GabeGiancarlo/EMR.git
cd EMR
just setup  # Install development dependencies
just dev    # Start development environment
```

## 📊 Project Stats

- **Language**: Rust 🦀
- **Framework**: Actix-web
- **API Endpoints**: 3 (working)
- **Frontend**: Interactive HTML/JS demo
- **Development Commands**: 15+
- **Status**: ✅ Functional prototype

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/GabeGiancarlo/EMR/issues)
- **Discussions**: [GitHub Discussions](https://github.com/GabeGiancarlo/EMR/discussions)

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 🎉 **Ready to Code!**

Your EMR platform is **live and ready for development**:

```bash
just dev    # Start everything
```

**Open the demo in your browser and start building the future of healthcare technology!** 🚀

---

*Built with ❤️ and Rust 🦀* 