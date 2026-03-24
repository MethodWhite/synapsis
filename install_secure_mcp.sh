#!/bin/bash
# Installation script for Synapsis Secure MCP Server
# Usage: sudo ./install_secure_mcp.sh [--docker]

set -e

echo "🔧 Synapsis Secure MCP Installation"
echo "==================================="

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    echo "❌ This script must be run as root (use sudo)"
    exit 1
fi

# Parse arguments
DOCKER_MODE=false
if [[ "$1" == "--docker" ]]; then
    DOCKER_MODE=true
    echo "🐳 Docker mode enabled"
fi

# Configuration
INSTALL_DIR="/opt/synapsis"
BIN_DIR="$INSTALL_DIR/bin"
DATA_DIR="/var/lib/synapsis"
CONFIG_DIR="/etc/synapsis"
SERVICE_USER="synapsis"
SERVICE_GROUP="synapsis"
SERVICE_NAME="synapsis-mcp-secure"

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

# Step 1: Create user and group
echo ""
echo "👤 Creating user and group..."
if ! getent group $SERVICE_GROUP > /dev/null; then
    groupadd --system $SERVICE_GROUP
    print_success "Created group $SERVICE_GROUP"
else
    print_warning "Group $SERVICE_GROUP already exists"
fi

if ! id -u $SERVICE_USER > /dev/null 2>&1; then
    useradd --system --no-create-home --shell /bin/false \
        --gid $SERVICE_GROUP $SERVICE_USER
    print_success "Created user $SERVICE_USER"
else
    print_warning "User $SERVICE_USER already exists"
fi

# Step 2: Create directories
echo ""
echo "📁 Creating directories..."
for dir in "$INSTALL_DIR" "$BIN_DIR" "$DATA_DIR" "$CONFIG_DIR"; do
    if [[ ! -d "$dir" ]]; then
        mkdir -p "$dir"
        chown $SERVICE_USER:$SERVICE_GROUP "$dir"
        chmod 750 "$dir"
        print_success "Created $dir"
    else
        print_warning "$dir already exists"
        chown $SERVICE_USER:$SERVICE_GROUP "$dir" 2>/dev/null || true
    fi
done

# Step 3: Install binary
echo ""
echo "📦 Installing binary..."
if [[ "$DOCKER_MODE" == true ]]; then
    echo "   Skipping binary copy in Docker mode"
    echo "   Build with: docker build -t synapsis-mcp-secure ."
else
    # Check if binary exists in current directory
    if [[ -f "./target/release/synapsis-mcp" ]]; then
        cp "./target/release/synapsis-mcp" "$BIN_DIR/"
        chown $SERVICE_USER:$SERVICE_GROUP "$BIN_DIR/synapsis-mcp"
        chmod 750 "$BIN_DIR/synapsis-mcp"
        print_success "Copied binary to $BIN_DIR/synapsis-mcp"
    else
        print_warning "Binary not found at ./target/release/synapsis-mcp"
        echo "   Please build first: cargo build --release --bin synapsis-mcp"
        echo "   Or compile with: ./compile_secure_mcp.sh --release"
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
fi

# Step 4: Install systemd service
echo ""
echo "⚙️  Installing systemd service..."
if [[ -f "./systemd/synapsis-mcp-secure.service" ]]; then
    cp "./systemd/synapsis-mcp-secure.service" "/etc/systemd/system/"
    
    # Replace paths in service file
    sed -i "s|/opt/synapsis|$INSTALL_DIR|g" "/etc/systemd/system/synapsis-mcp-secure.service"
    sed -i "s|/var/lib/synapsis|$DATA_DIR|g" "/etc/systemd/system/synapsis-mcp-secure.service"
    
    systemctl daemon-reload
    print_success "Installed systemd service"
else
    print_warning "Service file not found at ./systemd/synapsis-mcp-secure.service"
fi

# Step 5: Create configuration
echo ""
echo "📝 Creating configuration..."
if [[ ! -f "$CONFIG_DIR/env" ]]; then
    cat > "$CONFIG_DIR/env" << EOF
# Synapsis Secure MCP Server Environment
# Generated on $(date)

# PQC Passphrase (change this in production!)
SYNAPSIS_PQC_PASSPHRASE="$(openssl rand -base64 32 2>/dev/null || echo "change-me-"$(date +%s))"

# Server settings
SYNAPSIS_TCP_ADDR="0.0.0.0:7439"
SYNAPSIS_LOG_LEVEL="info"
SYNAPSIS_DATA_DIR="$DATA_DIR"

# Security settings
SYNAPSIS_SECURE_MODE="true"
SYNAPSIS_MAX_CONNECTIONS="100"
EOF
    chown $SERVICE_USER:$SERVICE_GROUP "$CONFIG_DIR/env"
    chmod 640 "$CONFIG_DIR/env"
    print_success "Created environment file: $CONFIG_DIR/env"
    print_warning "⚠️  Please edit $CONFIG_DIR/env to set a secure PQC passphrase!"
else
    print_warning "Environment file already exists: $CONFIG_DIR/env"
fi

# Step 6: Create data directory structure
echo ""
echo "🗂️  Setting up data directory..."
mkdir -p "$DATA_DIR/logs" "$DATA_DIR/vault" "$DATA_DIR/sessions"
chown -R $SERVICE_USER:$SERVICE_GROUP "$DATA_DIR"
chmod -R 750 "$DATA_DIR"
print_success "Created data directory structure"

# Step 7: Enable and start service
echo ""
echo "🚀 Starting service..."
if [[ -f "/etc/systemd/system/synapsis-mcp-secure.service" ]]; then
    systemctl enable "$SERVICE_NAME" 2>/dev/null || true
    
    echo "   To start service manually:"
    echo "   sudo systemctl start $SERVICE_NAME"
    echo ""
    echo "   To check status:"
    echo "   sudo systemctl status $SERVICE_NAME"
    echo ""
    echo "   To view logs:"
    echo "   sudo journalctl -u $SERVICE_NAME -f"
else
    print_warning "Service not installed, skipping enable"
fi

# Step 8: Firewall configuration (optional)
echo ""
echo "🔥 Configuring firewall (optional)..."
if command -v ufw > /dev/null && ufw status | grep -q "Status: active"; then
    ufw allow 7439/tcp comment "Synapsis Secure MCP"
    print_success "Added firewall rule for port 7439/tcp"
elif command -v firewall-cmd > /dev/null; then
    if systemctl is-active --quiet firewalld; then
        firewall-cmd --permanent --add-port=7439/tcp
        firewall-cmd --reload
        print_success "Added firewall rule for port 7439/tcp"
    fi
else
    print_warning "Firewall not detected or not active"
fi

# Step 9: Create client configuration example
echo ""
echo "💻 Client configuration example:"
cat << EOF

For MCP clients (Cursor, VS Code, Claude Desktop), add to your config:

=== ~/.config/aichat/mcp.yaml ===
servers:
  - name: synapsis
    command: $BIN_DIR/synapsis-mcp
    args: ["--bridge", "--url", "localhost:7439", "--secure"]

=== Cursor specific ===
Add to Cursor MCP config:
- name: synapsis
  command: $BIN_DIR/synapsis-mcp
  args: ["--bridge", "--url", "localhost:7439", "--secure"]

EOF

# Step 10: Summary
echo ""
echo "🎉 Installation Complete!"
echo "========================"
echo ""
echo "Summary:"
echo "  • User/Group: $SERVICE_USER:$SERVICE_GROUP"
echo "  • Install dir: $INSTALL_DIR"
echo "  • Binary dir: $BIN_DIR"
echo "  • Data dir: $DATA_DIR"
echo "  • Config dir: $CONFIG_DIR"
echo "  • Service: $SERVICE_NAME"
echo "  • Port: 7439/tcp (secure PQC)"
echo ""
echo "Next steps:"
echo "  1. Edit $CONFIG_DIR/env to set secure PQC passphrase"
echo "  2. Start service: sudo systemctl start $SERVICE_NAME"
echo "  3. Test connection: $BIN_DIR/synapsis-mcp --bridge --secure"
echo "  4. Configure your MCP client"
echo ""
echo "For troubleshooting, see:"
echo "  • Logs: sudo journalctl -u $SERVICE_NAME -f"
echo "  • Documentation: SECURE_MCP_SETUP.md"
echo "  • Protocol details: docs/secure_protocol_sequence.md"
echo ""
echo "Security reminder:"
echo "  • Keep $CONFIG_DIR/env secure (chmod 640)"
echo "  • Rotate PQC passphrase periodically"
echo "  • Monitor logs for unauthorized access attempts"
echo "  • Use firewall to restrict access to trusted IPs"

exit 0