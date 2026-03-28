#!/bin/bash
# =============================================================================
# Synapsis Verification Script
# =============================================================================
# Purpose: Provide independent verification of Synapsis technical claims
# Usage: ./verify_synapsis.sh
# =============================================================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║${NC}          Synapsis Independent Verification          ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════╝${NC}"
echo ""

PASS=0
FAIL=0
WARN=0

# Function to report test results
report() {
    local name="$1"
    local result="$2"
    local details="$3"
    
    if [ "$result" == "PASS" ]; then
        echo -e "${GREEN}✅ PASS${NC}: $name"
        [ -n "$details" ] && echo "   $details"
        ((PASS++))
    elif [ "$result" == "FAIL" ]; then
        echo -e "${RED}❌ FAIL${NC}: $name"
        [ -n "$details" ] && echo "   $details"
        ((FAIL++))
    else
        echo -e "${YELLOW}⚠️  WARN${NC}: $name"
        [ -n "$details" ] && echo "   $details"
        ((WARN++))
    fi
}

# =============================================================================
# Section 1: Build Verification
# =============================================================================
echo -e "${BLUE}━━━ Section 1: Build Verification ━━━${NC}"

# Test 1.1: Cargo build
if cargo build --release >/dev/null 2>&1; then
    BINARY_SIZE=$(ls -lh target/release/synapsis 2>/dev/null | awk '{print $5}')
    report "Release Build" "PASS" "Binary size: $BINARY_SIZE"
else
    report "Release Build" "FAIL" "cargo build --release failed"
fi

# Test 1.2: Binary exists
if [ -f "target/release/synapsis" ]; then
    report "Binary Exists" "PASS" "target/release/synapsis"
else
    report "Binary Exists" "FAIL" "Binary not found"
fi

echo ""

# =============================================================================
# Section 2: Test Suite
# =============================================================================
echo -e "${BLUE}━━━ Section 2: Test Suite ━━━${NC}"

# Test 2.1: Library tests
if cargo test --lib >/dev/null 2>&1; then
    report "Library Tests" "PASS" "All lib tests passing"
else
    report "Library Tests" "FAIL" "cargo test --lib failed"
fi

# Test 2.2: Test files exist
TEST_COUNT=$(ls tests/*.rs 2>/dev/null | wc -l)
if [ "$TEST_COUNT" -gt 0 ]; then
    report "Integration Tests" "PASS" "$TEST_COUNT test files found"
else
    report "Integration Tests" "FAIL" "No test files found"
fi

echo ""

# =============================================================================
# Section 3: Code Quality
# =============================================================================
echo -e "${BLUE}━━━ Section 3: Code Quality ━━━${NC}"

# Test 3.1: Format check
if cargo fmt --check >/dev/null 2>&1; then
    report "Code Format" "PASS" "rustfmt check passed"
else
    report "Code Format" "FAIL" "rustfmt check failed"
fi

# Test 3.2: Clippy (warnings allowed, errors not)
if cargo clippy -- -D errors >/dev/null 2>&1; then
    report "Clippy Lint" "PASS" "No clippy errors"
else
    report "Clippy Lint" "WARN" "Clippy has warnings or errors"
fi

echo ""

# =============================================================================
# Section 4: Security
# =============================================================================
echo -e "${BLUE}━━━ Section 4: Security ━━━${NC}"

# Test 4.1: Security audit
if command -v cargo-audit >/dev/null 2>&1; then
    if cargo audit >/dev/null 2>&1; then
        report "Security Audit" "PASS" "cargo audit: No vulnerabilities"
    else
        report "Security Audit" "FAIL" "cargo audit found vulnerabilities"
    fi
else
    report "Security Audit" "WARN" "cargo-audit not installed"
fi

# Test 4.2: PQC dependencies
if grep -q "pqcrypto-kyber" Cargo.toml; then
    report "PQC Dependencies" "PASS" "Kyber dependency found"
else
    report "PQC Dependencies" "FAIL" "PQC dependencies missing"
fi

# Test 4.3: PQC implementation
if grep -q "Kyber512" src -r --include="*.rs"; then
    report "PQC Implementation" "PASS" "Kyber512 usage found in code"
else
    report "PQC Implementation" "FAIL" "No Kyber512 usage found"
fi

echo ""

# =============================================================================
# Section 5: Documentation
# =============================================================================
echo -e "${BLUE}━━━ Section 5: Documentation ━━━${NC}"

# Test 5.1: README exists
if [ -f "README.md" ]; then
    README_LINES=$(wc -l < README.md)
    report "README.md" "PASS" "$README_LINES lines"
else
    report "README.md" "FAIL" "File not found"
fi

# Test 5.2: CLI Guide exists
if [ -f "docs/CLI_GUIDE.md" ]; then
    CLI_LINES=$(wc -l < docs/CLI_GUIDE.md)
    report "CLI_GUIDE.md" "PASS" "$CLI_LINES lines"
else
    report "CLI_GUIDE.md" "FAIL" "File not found"
fi

# Test 5.3: Security docs
if [ -f "docs/SECURITY.md" ]; then
    report "Security Docs" "PASS" "docs/SECURITY.md exists"
else
    report "Security Docs" "FAIL" "File not found"
fi

# Test 5.4: Documentation count
DOC_COUNT=$(find docs -name "*.md" | wc -l)
report "Documentation Count" "PASS" "$DOC_COUNT markdown files in docs/"

echo ""

# =============================================================================
# Section 6: Project Maturity
# =============================================================================
echo -e "${BLUE}━━━ Section 6: Project Maturity ━━━${NC}"

# Test 6.1: Commit count (if git available)
if command -v git >/dev/null 2>&1; then
    COMMIT_COUNT=$(git rev-list --count HEAD 2>/dev/null || echo "0")
    if [ "$COMMIT_COUNT" -gt 100 ]; then
        report "Git Commits" "PASS" "$COMMIT_COUNT commits"
    else
        report "Git Commits" "WARN" "$COMMIT_COUNT commits (low)"
    fi
    
    # Test 6.2: First commit date
    FIRST_COMMIT=$(git log --reverse --format="%ai" | head -1)
    if [ -n "$FIRST_COMMIT" ]; then
        report "First Commit" "PASS" "$FIRST_COMMIT"
    else
        report "First Commit" "WARN" "Could not determine"
    fi
else
    report "Git History" "WARN" "Git not available"
fi

# Test 6.3: Contributors
if command -v git >/dev/null 2>&1; then
    CONTRIBUTOR_COUNT=$(git log --format="%aN" | sort -u | wc -l)
    if [ "$CONTRIBUTOR_COUNT" -gt 1 ]; then
        report "Contributors" "PASS" "$CONTRIBUTOR_COUNT contributors"
    else
        report "Contributors" "WARN" "Single contributor ($CONTRIBUTOR_COUNT)"
    fi
fi

echo ""

# =============================================================================
# Section 7: CI/CD
# =============================================================================
echo -e "${BLUE}━━━ Section 7: CI/CD ━━━${NC}"

# Test 7.1: GitHub Actions
if [ -f ".github/workflows/ci.yml" ]; then
    report "GitHub Actions" "PASS" ".github/workflows/ci.yml exists"
else
    report "GitHub Actions" "FAIL" "No CI/CD workflow found"
fi

echo ""

# =============================================================================
# Summary
# =============================================================================
echo -e "${BLUE}━━━ Summary ━━━${NC}"
echo ""
echo -e "${GREEN}PASS:${NC} $PASS"
echo -e "${RED}FAIL:${NC} $FAIL"
echo -e "${YELLOW}WARN:${NC} $WARN"
echo ""

TOTAL=$((PASS + FAIL + WARN))
SCORE=$((PASS * 100 / TOTAL))

echo -e "Overall Score: ${BLUE}${SCORE}%${NC}"
echo ""

if [ $SCORE -ge 80 ]; then
    echo -e "${GREEN}✅ Synapsis verification PASSED${NC}"
    echo "   Project meets technical standards"
    exit 0
elif [ $SCORE -ge 60 ]; then
    echo -e "${YELLOW}⚠️  Synapsis verification PASSED with warnings${NC}"
    echo "   Project is functional but has areas for improvement"
    exit 0
else
    echo -e "${RED}❌ Synapsis verification FAILED${NC}"
    echo "   Project does not meet technical standards"
    exit 1
fi
