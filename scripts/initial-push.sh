#!/bin/bash

# EMR Platform - Initial GitHub Push Script
# This script helps set up Git and push the initial codebase to GitHub

set -e

echo "🚀 EMR Platform - Initial GitHub Setup"
echo "======================================="

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "api" ]; then
    echo "❌ Error: Please run this script from the EMR platform root directory"
    exit 1
fi

# Check if Git is initialized
if [ ! -d ".git" ]; then
    echo "📁 Initializing Git repository..."
    git init
    echo "✅ Git repository initialized"
else
    echo "✅ Git repository already exists"
fi

# Set up Git configuration (if not already set)
if ! git config user.name >/dev/null 2>&1; then
    echo "👤 Setting up Git user configuration..."
    read -p "Enter your name: " git_name
    read -p "Enter your email: " git_email
    git config user.name "$git_name"
    git config user.email "$git_email"
    echo "✅ Git user configuration set"
else
    echo "✅ Git user already configured as: $(git config user.name) <$(git config user.email)>"
fi

# Add remote origin if not already set
if ! git remote get-url origin >/dev/null 2>&1; then
    echo "🔗 Adding GitHub remote..."
    git remote add origin https://github.com/GabeGiancarlo/EMR.git
    echo "✅ GitHub remote added"
else
    current_remote=$(git remote get-url origin)
    if [ "$current_remote" != "https://github.com/GabeGiancarlo/EMR.git" ]; then
        echo "🔄 Updating GitHub remote..."
        git remote set-url origin https://github.com/GabeGiancarlo/EMR.git
        echo "✅ GitHub remote updated"
    else
        echo "✅ GitHub remote already configured correctly"
    fi
fi

# Create .env file from template if it doesn't exist
if [ ! -f ".env" ]; then
    echo "⚙️ Creating .env file from template..."
    cp infra/environment.template .env
    echo "✅ .env file created - please review and update as needed"
    echo "   📝 Edit .env with your local configuration before running the platform"
else
    echo "✅ .env file already exists"
fi

# Stage all files
echo "📝 Staging files for commit..."
git add .

# Check if there are any changes to commit
if git diff --staged --quiet; then
    echo "ℹ️  No changes to commit"
else
    # Create initial commit
    echo "💾 Creating initial commit..."
    git commit -m "feat: initial EMR platform implementation

🏥 Complete HIPAA-compliant EMR platform with:
- Rust backend with Actix-web API server
- FHIR R4 integration with Kodjin support
- Leptos SSR + WASM frontend
- Background job processing with Apalis
- Docker containerization and orchestration
- Comprehensive CI/CD with GitHub Actions
- Security scanning and compliance checks
- Development environment with devcontainers

✨ Features:
- Patient management with real-time search
- FHIR resource CRUD operations
- OAuth2/SMART-on-FHIR authentication
- Audit logging and compliance tracking
- Monitoring with Prometheus + Grafana
- TLS encryption and security headers

🛠️ Tech Stack:
- Backend: Rust 1.78, Actix-web, PostgreSQL 16, Redis, NATS
- Frontend: Leptos 0.8, WASM, Tailwind CSS
- Infrastructure: Docker, GitHub Actions, monitoring stack
- Security: TLS, JWT, audit logging, HIPAA compliance"

    echo "✅ Initial commit created"
fi

# Set up branch
echo "🌿 Setting up main branch..."
git branch -M main

# Push to GitHub
echo "🚀 Pushing to GitHub..."
if git push -u origin main; then
    echo "✅ Successfully pushed to GitHub!"
    echo ""
    echo "🎉 Your EMR Platform is now on GitHub!"
    echo "    Repository: https://github.com/GabeGiancarlo/EMR"
    echo ""
    echo "🔗 Next steps:"
    echo "   1. Review your repository at: https://github.com/GabeGiancarlo/EMR"
    echo "   2. Set up GitHub Actions secrets if needed"
    echo "   3. Configure branch protection rules"
    echo "   4. Review and update .env file for local development"
    echo "   5. Run 'just setup && just docker-up' to start the platform"
    echo ""
    echo "📚 Documentation:"
    echo "   - README: https://github.com/GabeGiancarlo/EMR/blob/main/README.md"
    echo "   - Issues: https://github.com/GabeGiancarlo/EMR/issues"
    echo "   - Actions: https://github.com/GabeGiancarlo/EMR/actions"
else
    echo "❌ Failed to push to GitHub"
    echo "   This might be due to authentication issues"
    echo "   Please check your GitHub credentials and try again"
    echo ""
    echo "🔧 Troubleshooting:"
    echo "   1. Make sure you're authenticated with GitHub"
    echo "   2. Try: gh auth login (if you have GitHub CLI)"
    echo "   3. Or set up SSH keys: https://docs.github.com/en/authentication/connecting-to-github-with-ssh"
    echo "   4. Then run: git push -u origin main"
    exit 1
fi

echo ""
echo "🏥 EMR Platform Setup Complete!"
echo "Happy coding! 🚀" 