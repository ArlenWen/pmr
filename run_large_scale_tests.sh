#!/bin/bash

# Large Scale Test Runner for PMR
# This script runs comprehensive tests for managing thousands of processes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Function to print colored status messages
print_status() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_header() {
    echo -e "${PURPLE}$1${NC}"
}

# Function to check system resources
check_system_resources() {
    print_header "🔍 System Resource Check"
    
    # Check available memory
    if command -v free >/dev/null 2>&1; then
        echo "Memory status:"
        free -h
        
        # Get available memory in MB
        available_mem=$(free -m | awk 'NR==2{printf "%.0f", $7}')
        if [ "$available_mem" -lt 1000 ]; then
            print_warning "Low available memory: ${available_mem}MB. Large scale tests may fail."
            print_warning "Consider closing other applications or increasing system memory."
        else
            print_success "Available memory: ${available_mem}MB - sufficient for large scale tests"
        fi
    fi
    
    # Check ulimit for processes
    echo ""
    echo "Process limits:"
    echo "Max processes: $(ulimit -u)"
    echo "Max open files: $(ulimit -n)"
    
    max_processes=$(ulimit -u)
    if [ "$max_processes" -lt 2000 ]; then
        print_warning "Process limit is low: $max_processes"
        print_warning "You may want to increase it with: ulimit -u 4096"
    else
        print_success "Process limit: $max_processes - sufficient for large scale tests"
    fi
    
    # Check disk space
    echo ""
    echo "Disk space in current directory:"
    df -h .
    
    echo ""
}

# Function to run a specific large scale test
run_large_scale_test() {
    local test_name="$1"
    local timeout_seconds="$2"
    
    print_status "Running: $test_name"
    print_status "Timeout: ${timeout_seconds}s ($(($timeout_seconds / 60)) minutes)"
    
    local start_time=$(date +%s)
    
    if timeout "$timeout_seconds" cargo test --test large_scale_tests --release "$test_name" -- --nocapture; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        print_success "$test_name completed in ${duration}s"
        return 0
    else
        local exit_code=$?
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        
        if [ $exit_code -eq 124 ]; then
            print_warning "$test_name timed out after ${timeout_seconds}s"
        else
            print_error "$test_name failed after ${duration}s"
        fi
        return 1
    fi
}

# Main execution
print_header "🚀 PMR Large Scale Test Suite"
print_header "Testing process management with 1000+ processes"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || ! grep -q "name = \"pmr\"" Cargo.toml; then
    print_error "This script must be run from the PMR project root directory"
    exit 1
fi

# System resource check
check_system_resources

# Ask for confirmation
echo ""
print_warning "Large scale tests will:"
print_warning "- Create and manage thousands of processes"
print_warning "- Generate substantial log files"
print_warning "- Use significant system resources"
print_warning "- Take 15-30 minutes to complete"
echo ""

if [ "$1" != "--yes" ] && [ "$1" != "-y" ]; then
    read -p "Do you want to continue? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_status "Test cancelled by user"
        exit 0
    fi
fi

echo ""
print_header "🧪 Starting Large Scale Tests"
echo ""

# Build in release mode first
print_status "Building PMR in release mode..."
if cargo build --release; then
    print_success "Build completed"
else
    print_error "Build failed"
    exit 1
fi

echo ""

# Initialize counters
total_tests=0
passed_tests=0
failed_tests=0

# Test 1: Thousand Process Creation
total_tests=$((total_tests + 1))
print_header "Test 1/6: Thousand Process Creation"
if run_large_scale_test "test_thousand_process_creation" 600; then
    passed_tests=$((passed_tests + 1))
else
    failed_tests=$((failed_tests + 1))
fi
echo ""

# Test 2: Concurrent Thousand Process Management
total_tests=$((total_tests + 1))
print_header "Test 2/6: Concurrent Process Management"
if run_large_scale_test "test_concurrent_thousand_process_management" 900; then
    passed_tests=$((passed_tests + 1))
else
    failed_tests=$((failed_tests + 1))
fi
echo ""

# Test 3: Database Performance at Scale
total_tests=$((total_tests + 1))
print_header "Test 3/6: Database Performance at Scale"
if run_large_scale_test "test_database_performance_at_scale" 600; then
    passed_tests=$((passed_tests + 1))
else
    failed_tests=$((failed_tests + 1))
fi
echo ""

# Test 4: Memory Stability at Scale
total_tests=$((total_tests + 1))
print_header "Test 4/6: Memory Stability at Scale"
if run_large_scale_test "test_memory_stability_at_scale" 900; then
    passed_tests=$((passed_tests + 1))
else
    failed_tests=$((failed_tests + 1))
fi
echo ""

# Test 5: Log Handling at Scale
total_tests=$((total_tests + 1))
print_header "Test 5/6: Log Handling at Scale"
if run_large_scale_test "test_log_handling_at_scale" 600; then
    passed_tests=$((passed_tests + 1))
else
    failed_tests=$((failed_tests + 1))
fi
echo ""

# Test 6: System Resource Limits
total_tests=$((total_tests + 1))
print_header "Test 6/6: System Resource Limits"
if run_large_scale_test "test_system_resource_limits" 900; then
    passed_tests=$((passed_tests + 1))
else
    failed_tests=$((failed_tests + 1))
fi
echo ""

# Optional: Mixed Workload Test
if [ "$1" = "--full" ] || [ "$1" = "-f" ]; then
    total_tests=$((total_tests + 1))
    print_header "Bonus Test: Mixed Workload at Scale"
    if run_large_scale_test "test_mixed_workload_at_scale" 300; then
        passed_tests=$((passed_tests + 1))
    else
        failed_tests=$((failed_tests + 1))
    fi
    echo ""
fi

# Final results
print_header "📊 Large Scale Test Results"
echo ""
echo "Total tests: $total_tests"
echo "Passed: $passed_tests"
echo "Failed: $failed_tests"

if [ $failed_tests -eq 0 ]; then
    print_success "All large scale tests passed! 🎉"
    print_success "PMR successfully handles thousands of processes"
else
    print_warning "$failed_tests out of $total_tests tests failed"
    if [ $passed_tests -gt 0 ]; then
        print_status "However, $passed_tests tests passed, indicating partial large scale capability"
    fi
fi

echo ""
print_header "💡 Test Summary"
echo "These tests verified PMR's ability to:"
echo "- Create and manage 1000+ processes simultaneously"
echo "- Handle concurrent operations on large process sets"
echo "- Maintain database performance with large datasets"
echo "- Preserve memory stability under heavy load"
echo "- Process logs from hundreds of processes"
echo "- Gracefully handle system resource limits"

if [ "$1" = "--full" ] || [ "$1" = "-f" ]; then
    echo "- Handle mixed production-like workloads at scale"
fi

echo ""
print_status "Large scale testing completed"

exit $failed_tests
