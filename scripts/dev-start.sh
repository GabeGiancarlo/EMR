#!/bin/bash

# EMR Platform - Development Startup Script
# This script starts the development environment

set -e

echo "🏥 EMR Platform - Starting Development Environment"
echo "=================================================="

# Check if API server is already running
if lsof -Pi :8080 -sTCP:LISTEN -t >/dev/null ; then
    echo "✅ API Server already running on port 8080"
else
    echo "🚀 Starting API Server on port 8080..."
    cd api
    cargo run &
    API_PID=$!
    cd ..
    
    # Wait a moment for the server to start
    sleep 3
    
    # Check if server started successfully
    if curl -f http://localhost:8080/healthz >/dev/null 2>&1; then
        echo "✅ API Server started successfully"
    else
        echo "❌ Failed to start API Server"
        exit 1
    fi
fi

# Open the demo in the browser
echo "🌐 Opening EMR Platform Demo..."
open demo/index.html

echo ""
echo "🎉 Development Environment Ready!"
echo ""
echo "📍 API Server: http://localhost:8080"
echo "📍 Health Check: http://localhost:8080/healthz"
echo "📍 Patients API: http://localhost:8080/api/patients"
echo "📍 Demo Interface: demo/index.html (opened in browser)"
echo ""
echo "🔧 To stop the API server:"
if [ ! -z "$API_PID" ]; then
    echo "   kill $API_PID"
    echo $API_PID > .api_pid
fi
echo "   or use: just dev-stop"
echo ""
echo "🔍 API Endpoints:"
echo "   GET /healthz - System health check"
echo "   GET /api/patients - List all patients"
echo "   GET /api/patients/{id} - Get specific patient"
echo ""
echo "✨ Ready for real-time development!" 