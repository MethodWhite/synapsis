# TCP Usage Recommendations

## Summary
Based on performance analysis and user requirements, the following recommendations are implemented:

## 1. Primary Interface: MCP Server (stdio)
- **Recommended for**: Single-user local development
- **Command**: `synapsis-mcp` (no arguments)
- **Advantages**: Lowest latency, no TCP overhead, simple configuration
- **Status**: ✅ **Default and recommended**

## 2. TCP Server (Multi-client/Remote)
- **Recommended for**: Multiple concurrent clients, remote connections
- **Command**: `synapsis-mcp --tcp [--secure]`
- **Port**: Default 7439 (secure: 7439, insecure: 7438)
- **Advantages**: Supports multiple simultaneous connections
- **Disadvantages**: Adds TCP overhead, requires port management

## 3. Secure TCP (PQC Encrypted)
- **Recommended for**: Network connections requiring encryption
- **Command**: `synapsis-mcp --tcp --secure`
- **Protocol**: Kyber512 key exchange + AES-256-GCM encryption
- **Advantages**: Post-quantum secure, authenticated encryption
- **Disadvantages**: Handshake overhead, slightly higher latency

## Deprecated Components

### ❌ Bridge Mode (`--bridge`)
- **Status**: Deprecated
- **Reason**: Adds unnecessary TCP overhead for local use
- **Migration**: Use local mode (`synapsis-mcp`) or TCP server mode (`--tcp`)

### ❌ Standalone TCP Server (`synapsis` binary)
- **Status**: Deprecated (phasing out)
- **Reason**: Redundant with MCP TCP server
- **Migration**: Use `synapsis-mcp --tcp`

### ❌ HTTP Server (`synapsis-http`)
- **Status**: Deprecated
- **Reason**: Not core to MCP functionality
- **Migration**: Use MCP protocol directly

## Configuration Updates

### MCP Client Configuration (e.g., Cursor, VS Code)
```yaml
servers:
  - name: synapsis
    command: /path/to/synapsis-mcp
    args: []  # Empty for local mode (recommended)
```

For multi-client scenarios:
```yaml
args: ["--tcp", "--secure"]  # Secure TCP server mode
```

## Performance Considerations

| Mode | Latency | Use Case |
|------|---------|----------|
| Local stdio | ⭐⭐⭐⭐⭐ Lowest | Single client, local development |
| TCP (loopback) | ⭐⭐⭐ Medium | Multiple local clients |
| Secure TCP | ⭐⭐ Higher | Network/encrypted connections |

## Implementation Status

- [x] Updated MCP binary with deprecation warnings
- [x] Modified default configuration to use local mode
- [x] Added connection status tool with green/red indicators
- [x] Commented out HTTP server binary
- [x] Added deprecation warning to standalone TCP server

## Next Steps

1. Monitor usage and gather feedback
2. Gradually migrate existing TCP server clients to MCP
3. Consider removing deprecated binaries in future release
4. Optimize secure TCP handshake for lower latency