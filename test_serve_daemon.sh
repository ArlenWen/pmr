#!/bin/bash

# Test script for pmr serve daemon functionality
set -e

PMR="./target/release/pmr"

echo "=== Testing PMR Serve Daemon Functionality ==="

# Clean up any existing server
echo "1. Cleaning up any existing HTTP server..."
$PMR serve-stop 2>/dev/null || true
$PMR delete __pmr_http_server__ 2>/dev/null || true

# Test serve-status when no server is running
echo "2. Testing serve-status when no server is running..."
$PMR serve-status

# Start server in daemon mode
echo "3. Starting HTTP server in daemon mode..."
$PMR serve --daemon --port 8080

# Check status
echo "4. Checking server status..."
$PMR serve-status

# Test JSON format
echo "5. Testing JSON format status..."
$PMR --format json serve-status

# Try to start another server (should fail)
echo "6. Testing duplicate server start (should fail)..."
$PMR serve --daemon --port 8080 || echo "Expected failure - server already running"

# Test API connectivity
echo "7. Testing API connectivity..."
# Generate token first
TOKEN_OUTPUT=$($PMR auth generate test-token)
TOKEN=$(echo "$TOKEN_OUTPUT" | grep "Token:" | cut -d' ' -f2)
echo "Generated token: $TOKEN"

# Test API call
curl -s -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/processes | jq .

# Stop server
echo "8. Stopping HTTP server..."
$PMR serve-stop

# Verify server is stopped
echo "9. Verifying server is stopped..."
$PMR serve-status

# Test restart functionality
echo "10. Testing restart functionality..."
$PMR serve-restart --port 8080

# Check status after restart
echo "11. Checking status after restart..."
$PMR serve-status

# Final cleanup
echo "12. Final cleanup..."
$PMR serve-stop

echo "=== All tests completed successfully! ==="
