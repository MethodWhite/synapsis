#!/bin/bash
# Pre-flight check for Synapsis Secure MCP compilation
# Checks dependencies, code structure, and potential issues

set -e

echo "🔍 Synapsis Secure MCP Pre-flight Check"
echo "========================================"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

check_file_exists() {
    if [[ -f "$1" ]]; then
        print_success "Found $1"
        return 0
    else
        print_error "Missing $1"
        return 1
    fi
}

check_file_contains() {
    if grep -q "$2" "$1" 2>/dev/null; then
        print_success "$1 contains '$2'"
        return 0
    else
        print_warning "$1 does not contain '$2'"
        return 1
    fi
}

# Check 1: Essential source files
echo ""
echo "📁 Checking source files..."
ESSENTIAL_FILES=(
    "src/presentation/mcp/secure_tcp.rs"
    "src/bin/mcp.rs"
    "src/core/crypto_provider.rs"
    "src/core/pqc.rs"
    "src/domain/crypto.rs"
    "Cargo.toml"
)

missing_files=0
for file in "${ESSENTIAL_FILES[@]}"; do
    check_file_exists "$file" || ((missing_files++))
done

if [[ $missing_files -gt 0 ]]; then
    print_error "Missing $missing_files essential files"
else
    print_success "All essential files present"
fi

# Check 2: Cargo.toml dependencies
echo ""
echo "📦 Checking Cargo.toml dependencies..."
REQUIRED_CRATES=(
    "pqcrypto-kyber"
    "pqcrypto-dilithium"
    "aes-gcm"
    "base64"
    "serde"
    "serde_json"
    "tokio"
)

for crate in "${REQUIRED_CRATES[@]}"; do
    if grep -q "$crate" Cargo.toml; then
        print_success "Crate $crate in Cargo.toml"
    else
        print_warning "Crate $crate not found in Cargo.toml"
    fi
done

# Check 3: Module exports
echo ""
echo "🔗 Checking module exports..."
check_file_contains "src/presentation/mcp/mod.rs" "pub mod secure_tcp"
check_file_contains "src/presentation/mcp/mod.rs" "pub use server::McpServer"

# Check 4: Secure TCP implementation
echo ""
echo "🔐 Checking secure TCP implementation..."
SECURE_TCP_CHECKS=(
    "struct SecureTcpServer"
    "struct SecureTcpClient"
    "fn perform_kyber_handshake"
    "fn encrypt_message"
    "fn decrypt_message"
    "PqcAlgorithm::Kyber512"
    "PqcAlgorithm::Aes256Gcm"
)

for check in "${SECURE_TCP_CHECKS[@]}"; do
    if grep -q "$check" src/presentation/mcp/secure_tcp.rs; then
        print_success "secure_tcp.rs contains $check"
    else
        print_warning "secure_tcp.rs missing $check"
    fi
done

# Check 5: MCP binary integration
echo ""
echo "🔄 Checking MCP binary integration..."
check_file_contains "src/bin/mcp.rs" "secure_tcp"
check_file_contains "src/bin/mcp.rs" "--secure"
check_file_contains "src/bin/mcp.rs" "--insecure"
check_file_contains "src/bin/mcp.rs" "run_tcp_server"

# Check 6: Crypto provider implementation
echo ""
echo "🔑 Checking crypto provider..."
check_file_contains "src/core/crypto_provider.rs" "impl CryptoProvider for SynapsisPqcProvider"
check_file_contains "src/core/crypto_provider.rs" "generate_keypair"
check_file_contains "src/core/crypto_provider.rs" "encapsulate"
check_file_contains "src/core/crypto_provider.rs" "decapsulate"

# Check 7: PQC algorithm support
echo ""
echo "⚡ Checking PQC algorithm support..."
check_file_contains "src/domain/crypto.rs" "enum PqcAlgorithm"
check_file_contains "src/domain/crypto.rs" "Kyber512"
check_file_contains "src/domain/crypto.rs" "Aes256Gcm"

# Check 8: Error handling
echo ""
echo "🚨 Checking error handling..."
check_file_contains "src/domain/errors.rs" "crypto_pqc_not_supported"
check_file_contains "src/domain/errors.rs" "crypto_cipher"

# Check 9: Documentation
echo ""
echo "📚 Checking documentation..."
check_file_exists "SECURE_MCP_SETUP.md"
check_file_exists "docs/secure_protocol_sequence.md"
check_file_exists "compile_secure_mcp.sh"

# Check 10: Test files
echo ""
echo "🧪 Checking test files..."
check_file_exists "tests/secure_handshake_test.py"
check_file_exists "tests/synapsis_pqc_integration.rs"

# Summary
echo ""
echo "📊 Pre-flight Check Summary"
echo "==========================="

if [[ $missing_files -gt 0 ]]; then
    print_error "CRITICAL: Missing essential files"
    echo "   Please ensure all source files are present"
    exit 1
fi

echo ""
echo "🚀 Ready for compilation!"
echo ""
echo "Next steps:"
echo "  1. Ensure Rust 1.75+ is installed"
echo "  2. Run: ./compile_secure_mcp.sh --release"
echo "  3. Test: ./target/release/synapsis-mcp --help"
echo "  4. Start server: ./target/release/synapsis-mcp --tcp --secure"
echo "  5. Test client: ./target/release/synapsis-mcp --bridge --secure"
echo ""
echo "For Docker build:"
echo "  docker build -t synapsis-mcp-secure ."
echo ""
echo "For system installation:"
echo "  sudo ./install_secure_mcp.sh"
echo ""
echo "Documentation:"
echo "  • SECURE_MCP_SETUP.md - Complete setup guide"
echo "  • docs/secure_protocol_sequence.md - Protocol details"
echo "  • NEXT_STEPS_SECURE_MCP.md - Implementation roadmap"

exit 0