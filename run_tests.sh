#!/bin/bash

# PMR Project Test Runner
# This script runs all tests for the PMR project

set -e  # Exit on any error

echo "🚀 PMR Project Test Suite"
echo "========================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to run a test suite
run_test_suite() {
    local test_name="$1"
    local test_command="$2"
    
    print_status "Running $test_name..."
    
    if eval "$test_command"; then
        print_success "$test_name passed!"
        return 0
    else
        print_error "$test_name failed!"
        return 1
    fi
}

# Initialize counters
total_suites=0
passed_suites=0
failed_suites=0

echo "📋 Test Plan:"
echo "1. Unit Tests (Library)"
echo "2. CLI Tests"
echo "3. Database Tests"
echo "4. Integration Tests"
echo "5. End-to-End Tests"
echo "6. Performance Tests"
echo "7. Large Scale Tests (1000+ processes)"
echo "8. HTTP API Tests (optional)"
echo ""

# 1. Unit Tests
print_status "Starting test execution..."
echo ""

total_suites=$((total_suites + 1))
if run_test_suite "Unit Tests" "cargo test --lib"; then
    passed_suites=$((passed_suites + 1))
else
    failed_suites=$((failed_suites + 1))
fi
echo ""

# 2. CLI Tests
total_suites=$((total_suites + 1))
if run_test_suite "CLI Tests" "cargo test --test cli_tests"; then
    passed_suites=$((passed_suites + 1))
else
    failed_suites=$((failed_suites + 1))
fi
echo ""

# 3. Database Tests
total_suites=$((total_suites + 1))
if run_test_suite "Database Tests" "cargo test --test database_tests"; then
    passed_suites=$((passed_suites + 1))
else
    failed_suites=$((failed_suites + 1))
fi
echo ""

# 4. Integration Tests
total_suites=$((total_suites + 1))
if run_test_suite "Integration Tests" "cargo test --test integration_tests"; then
    passed_suites=$((passed_suites + 1))
else
    failed_suites=$((failed_suites + 1))
fi
echo ""

# 5. End-to-End Tests
total_suites=$((total_suites + 1))
if run_test_suite "End-to-End Tests" "cargo test --test end_to_end_tests"; then
    passed_suites=$((passed_suites + 1))
else
    failed_suites=$((failed_suites + 1))
fi
echo ""

# 6. Performance Tests (with timeout)
total_suites=$((total_suites + 1))
print_status "Running Performance Tests (may take longer)..."
if timeout 300 cargo test --test performance_tests --release; then
    print_success "Performance Tests passed!"
    passed_suites=$((passed_suites + 1))
else
    exit_code=$?
    if [ $exit_code -eq 124 ]; then
        print_warning "Performance Tests timed out (5 minutes)"
    else
        print_error "Performance Tests failed!"
    fi
    failed_suites=$((failed_suites + 1))
fi
echo ""

# 7. Large Scale Tests (with extended timeout)
total_suites=$((total_suites + 1))
print_status "Running Large Scale Tests (testing 1000+ processes, may take much longer)..."
print_warning "This test may consume significant system resources and take 10+ minutes"
if timeout 900 cargo test --test large_scale_tests --release; then
    print_success "Large Scale Tests passed!"
    passed_suites=$((passed_suites + 1))
else
    exit_code=$?
    if [ $exit_code -eq 124 ]; then
        print_warning "Large Scale Tests timed out (15 minutes)"
    else
        print_error "Large Scale Tests failed!"
    fi
    failed_suites=$((failed_suites + 1))
fi
echo ""

# 8. HTTP API Tests (optional)
if [ "$1" = "--with-api" ] || [ "$1" = "-a" ]; then
    total_suites=$((total_suites + 1))
    if run_test_suite "HTTP API Tests" "cargo test --test api_tests --features http-api"; then
        passed_suites=$((passed_suites + 1))
    else
        failed_suites=$((failed_suites + 1))
    fi
    echo ""
fi

# Build Tests
print_status "Testing build configurations..."
echo ""

# Test default build
if cargo build --release > /dev/null 2>&1; then
    print_success "Default build successful"
else
    print_error "Default build failed"
fi

# Test build with HTTP API feature
if cargo build --release --features http-api > /dev/null 2>&1; then
    print_success "HTTP API build successful"
else
    print_error "HTTP API build failed"
fi

echo ""
echo "📊 Test Summary"
echo "==============="
echo "Total test suites: $total_suites"
echo "Passed: $passed_suites"
echo "Failed: $failed_suites"

if [ $failed_suites -eq 0 ]; then
    print_success "All test suites passed! 🎉"
    echo ""
    echo "✅ PMR is ready for production use!"
    exit 0
else
    print_error "$failed_suites test suite(s) failed"
    echo ""
    echo "❌ Please fix failing tests before deployment"
    exit 1
fi
