# Test Script for QNet Linter

# This script demonstrates how to test the Go Spec Linter
# Run this after installing Go 1.21+

echo "Installing Go dependencies..."
go mod download

echo "Building linter..."
go build -o qnet-lint ./cmd/qnet-lint

echo "Testing validation on QNet project..."
./qnet-lint validate ../

echo "Testing SBOM generation..."
./qnet-lint sbom ../

echo "Test complete!"
