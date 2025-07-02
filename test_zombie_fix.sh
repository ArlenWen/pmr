#!/bin/bash

# Test script to verify zombie process fix
echo "Testing zombie process fix..."

# Clean up any existing test processes
./target/debug/pmr clear --all 2>/dev/null || true

echo "1. Starting test processes..."
./target/debug/pmr start zombie-test1 sleep 3
./target/debug/pmr start zombie-test2 sleep 5
./target/debug/pmr start zombie-test3 sleep 2

echo "2. Checking initial process list..."
./target/debug/pmr list

echo "3. Waiting for processes to finish naturally..."
sleep 6

echo "4. Checking process list after natural termination..."
./target/debug/pmr list

echo "5. Checking for zombie processes in system..."
echo "Defunct processes before cleanup:"
ps aux | grep defunct | grep -v grep || echo "No defunct processes found"

echo "6. Cleaning up remaining processes..."
./target/debug/pmr clear --all

echo "7. Final check for zombie processes..."
echo "Defunct processes after cleanup:"
ps aux | grep defunct | grep -v grep || echo "No defunct processes found"

echo "Test completed!"
