#!/bin/bash

# EMR Platform - Devcontainer Post-Create Script
# This script sets up the development environment after container creation

set -e

echo "🚀 Setting up EMR Platform development environment..."

# Update package lists
echo "📦 Updating package lists..."
sudo apt-get update -qq

# Install additional development tools
echo "🔧 Installing development tools..."
sudo apt-get install -y -qq \
    curl \
    wget \
    jq \
    tree \
    htop \
    vim \
    nano \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    libpq-dev \
    postgresql-client \
    redis-tools \
    netcat \
    telnet \
    make \
    cmake \
    llvm \
    clang \
    mkcert \
    ca-certificates \
    gnupg \
    lsb-release

# Install Rust components
echo "🦀 Setting up Rust environment..."
rustup update stable
rustup default stable
rustup target add wasm32-unknown-unknown
rustup component add rustfmt clippy llvm-tools-preview

# Install additional Rust tools
echo "🔨 Installing Rust development tools..."
cargo install --locked \
    cargo-watch \
    cargo-edit \
    cargo-audit \
    cargo-deny \
    cargo-license \
    cargo-nextest \
    cargo-llvm-cov \
    cargo-expand \
    cargo-tree \
    cargo-outdated \
    cargo-geiger \
    cargo-machete \
    cargo-semver-checks \
    wasm-pack \
    just \
    sqlx-cli

# Install Node.js tools for web development
echo "📱 Setting up Node.js tools..."
npm install -g \
    typescript \
    ts-node \
    @types/node \
    eslint \
    prettier \
    vite \
    rollup \
    webpack \
    postcss \
    tailwindcss \
    @tailwindcss/typography \
    @tailwindcss/forms \
    @tailwindcss/aspect-ratio

# Set up Git configuration
echo "🔧 Configuring Git..."
git config --global init.defaultBranch main
git config --global pull.rebase false
git config --global core.autocrlf false
git config --global core.eol lf
git config --global core.editor "code --wait"
git config --global merge.tool vscode
git config --global mergetool.vscode.cmd "code --wait $MERGED"
git config --global diff.tool vscode
git config --global difftool.vscode.cmd "code --wait --diff $LOCAL $REMOTE"

# Create necessary directories
echo "📁 Creating project directories..."
mkdir -p ~/.cargo/bin
mkdir -p ~/.config
mkdir -p ~/bin
mkdir -p /tmp/target
mkdir -p /workspace/logs
mkdir -p /workspace/tmp
mkdir -p /workspace/scripts

# Set up environment variables
echo "🌍 Setting up environment variables..."
cat >> ~/.bashrc << 'EOF'

# EMR Platform Development Environment
export RUST_LOG=debug
export RUST_BACKTRACE=1
export DATABASE_URL=postgresql://emr_user:emr_secure_password_change_in_prod@postgres:5432/emr_platform
export REDIS_URL=redis://redis:6379
export NATS_URL=nats://nats:4222
export CARGO_TARGET_DIR=/tmp/target
export PATH=$PATH:~/.cargo/bin:~/bin

# Aliases for development
alias ll='ls -alF'
alias la='ls -A'
alias l='ls -CF'
alias ..='cd ..'
alias ...='cd ../..'
alias grep='grep --color=auto'
alias fgrep='fgrep --color=auto'
alias egrep='egrep --color=auto'
alias tree='tree -I target'
alias cr='cargo run'
alias cb='cargo build'
alias ct='cargo test'
alias cc='cargo check'
alias cf='cargo fmt'
alias cl='cargo clippy'
alias cw='cargo watch'
alias jd='just dev'
alias jt='just test'
alias jb='just build'
alias jl='just lint'
alias psql-emr='psql $DATABASE_URL'
alias redis-emr='redis-cli -u $REDIS_URL'
alias docker-logs='docker compose logs -f'
alias docker-ps='docker compose ps'
alias docker-down='docker compose down'
alias docker-up='docker compose up -d'

# Functions
function cargo-clean-all() {
    echo "Cleaning all cargo caches..."
    cargo clean
    rm -rf ~/.cargo/registry/index
    rm -rf ~/.cargo/registry/cache
    rm -rf ~/.cargo/git
}

function emr-status() {
    echo "EMR Platform Status:"
    echo "==================="
    curl -s http://localhost:8080/healthz | jq '.' || echo "API not responding"
    echo ""
    curl -s http://localhost:3000/api/health | jq '.' || echo "Web not responding"
    echo ""
    docker compose ps
}

function emr-logs() {
    if [ "$1" ]; then
        docker compose logs -f "$1"
    else
        docker compose logs -f
    fi
}

function emr-shell() {
    if [ "$1" ]; then
        docker compose exec "$1" bash
    else
        echo "Usage: emr-shell <service>"
        echo "Available services:"
        docker compose ps --services
    fi
}

function emr-reset() {
    echo "Resetting EMR Platform..."
    docker compose down -v
    docker compose up -d
    echo "Reset complete!"
}

EOF

# Set up shell prompt
echo "🎨 Setting up shell prompt..."
cat >> ~/.bashrc << 'EOF'

# Custom prompt for EMR development
export PS1='\[\033[01;32m\]\u@emr-dev\[\033[00m\]:\[\033[01;34m\]\w\[\033[00m\]\$ '

EOF

# Create development scripts
echo "📝 Creating development scripts..."

# Health check script
cat > ~/bin/health-check.sh << 'EOF'
#!/bin/bash
echo "🏥 EMR Platform Health Check"
echo "==========================="

# Check API
echo -n "API Server: "
if curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/healthz | grep -q "200"; then
    echo "✅ Healthy"
else
    echo "❌ Not responding"
fi

# Check Web
echo -n "Web Server: "
if curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/api/health | grep -q "200"; then
    echo "✅ Healthy"
else
    echo "❌ Not responding"
fi

# Check Database
echo -n "PostgreSQL: "
if pg_isready -h postgres -p 5432 -U emr_user > /dev/null 2>&1; then
    echo "✅ Connected"
else
    echo "❌ Cannot connect"
fi

# Check Redis
echo -n "Redis: "
if redis-cli -h redis -p 6379 ping > /dev/null 2>&1; then
    echo "✅ Connected"
else
    echo "❌ Cannot connect"
fi

# Check NATS
echo -n "NATS: "
if nc -z nats 4222 > /dev/null 2>&1; then
    echo "✅ Connected"
else
    echo "❌ Cannot connect"
fi

echo ""
echo "🐳 Docker Status:"
docker compose ps
EOF

# Make scripts executable
chmod +x ~/bin/health-check.sh

# Set up mkcert for local TLS development
echo "🔐 Setting up local TLS certificates..."
mkcert -install > /dev/null 2>&1 || echo "mkcert not available, skipping TLS setup"

# Create test certificates directory
mkdir -p /workspace/infra/certs
cd /workspace/infra/certs

# Generate self-signed certificates for development
openssl req -x509 -newkey rsa:4096 -keyout server.key -out server.crt -days 365 -nodes \
    -subj "/C=US/ST=Development/L=Local/O=EMR Platform/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,DNS:emr-api,DNS:emr-web,IP:127.0.0.1" \
    > /dev/null 2>&1

# Create initial .env file from template
echo "⚙️ Creating initial configuration..."
cd /workspace
if [ ! -f .env ]; then
    cp infra/environment.template .env
    echo "Created .env file from template"
fi

# Set proper permissions
echo "🔧 Setting up permissions..."
sudo chown -R vscode:vscode /workspace
sudo chown -R vscode:vscode ~/.cargo
sudo chown -R vscode:vscode ~/.config
sudo chown -R vscode:vscode ~/bin
chmod 600 /workspace/infra/certs/server.key
chmod 644 /workspace/infra/certs/server.crt

# Build project dependencies
echo "🏗️ Building project dependencies..."
cd /workspace
cargo fetch --locked
cargo build --workspace --all-features

# Generate initial database migration (if not exists)
echo "🗄️ Setting up database..."
if [ ! -d "migrations" ]; then
    sqlx migrate add initial_schema
    echo "Created initial migration"
fi

# Set up git hooks
echo "🪝 Setting up git hooks..."
mkdir -p .git/hooks
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
echo "Running pre-commit checks..."
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
EOF
chmod +x .git/hooks/pre-commit

# Create welcome message
echo "🎉 Development environment setup complete!"
echo ""
echo "🏥 EMR Platform Development Environment"
echo "======================================"
echo ""
echo "Available commands:"
echo "  just dev      - Start development servers"
echo "  just test     - Run tests"
echo "  just lint     - Run linting"
echo "  just build    - Build all services"
echo "  health-check.sh - Check service health"
echo "  emr-status    - Show platform status"
echo "  emr-logs      - Show service logs"
echo "  emr-shell     - Access service shell"
echo "  emr-reset     - Reset entire platform"
echo ""
echo "Services will be available at:"
echo "  API:    https://localhost:8080"
echo "  Web:    https://localhost:3000"
echo "  DB:     postgresql://emr_user:***@localhost:5432/emr_platform"
echo "  Redis:  redis://localhost:6379"
echo "  NATS:   nats://localhost:4222"
echo ""
echo "Happy coding! 🚀"

# Source the new bashrc
source ~/.bashrc 