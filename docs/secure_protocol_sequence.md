# Secure MCP Protocol Sequence

## Overview

The secure MCP protocol uses **Kyber512 key exchange** followed by **AES-256-GCM encryption** for all messages. This provides post-quantum security with forward secrecy.

## Handshake Sequence

### Step-by-Step Protocol

```
Client                                 Server
  |                                       |
  |--- 1. Generate client keypair ------->|
  |    (pk_c, sk_c)                       |
  |                                       |
  |--- 2. Send pk_c (base64) ------------>|
  |                                       |
  |                                       |--- 3. Generate server keypair (optional)
  |                                       |    (pk_s, sk_s) - not used for KEM
  |                                       |
  |                                       |--- 4. Encapsulate shared secret
  |                                       |    (ciphertext, shared_secret) = encapsulate(pk_c)
  |                                       |
  |<-- 5. Send pk_s + ciphertext ---------|
  |    (base64, base64)                   |
  |                                       |
  |--- 6. Decapsulate shared secret ----->|
  |    shared_secret = decapsulate(ciphertext, sk_c)
  |                                       |
  |--- 7. Derive AES key ---------------->|
  |    aes_key = shared_secret[0:32]      |
  |                                       |
  |                                       |--- 8. Derive AES key
  |                                       |    aes_key = shared_secret[0:32]
  |
  |=== SECURE CHANNEL ESTABLISHED ========|
  |
  |--- 9. Encrypt message --------------->|
  |    ciphertext = AES256GCM(aes_key, plaintext)
  |    send(base64(ciphertext))
  |                                       |
  |                                       |--- 10. Decrypt message
  |                                       |    plaintext = AES256GCM(aes_key, ciphertext)
  |                                       |
  |                                       |--- 11. Process MCP request
  |                                       |
  |<-- 12. Encrypt response --------------|
  |    ciphertext = AES256GCM(aes_key, response)
  |    send(base64(ciphertext))
  |
  |--- 13. Decrypt response ------------->|
```

## Message Formats

### Handshake Phase

**Client → Server (line 1):**
```
<base64-encoded client public key (512 bytes)>
```

**Server → Client (line 2):**
```
<base64-encoded server public key (512 bytes)> <base64-encoded ciphertext (768 bytes)>
```

### Secure Communication Phase

**All subsequent messages:**
```
<base64-encoded AES-256-GCM ciphertext>
```

The AES-256-GCM ciphertext format:
```
[12-byte nonce][ciphertext][16-byte authentication tag]
```

## Key Derivation

```
shared_secret = Kyber512 key exchange (32 bytes)
aes_key = shared_secret[0:32]  # Use first 32 bytes directly
```

## Security Properties

### 1. Forward Secrecy
- Ephemeral Kyber512 keypairs for each session
- No long-term key material used for encryption

### 2. Post-Quantum Security
- Kyber512 (ML-KEM-512) - NIST PQC standard
- AES-256-GCM as symmetric primitive

### 3. Authenticated Encryption
- AES-256-GCM provides confidentiality and integrity
- Random 12-byte nonce for each message

### 4. MITM Protection
- Server authentication planned via Dilithium signatures
- Current: No server authentication (trust-on-first-use)

## Error Handling

### Handshake Failures
1. **Invalid public key format**: Close connection
2. **Decapsulation failure**: Close connection  
3. **Timeout (30s)**: Close connection

### Secure Channel Failures
1. **Decryption failure**: Close connection
2. **Authentication tag mismatch**: Close connection
3. **Malformed base64**: Close connection

## Implementation Details

### File: `src/presentation/mcp/secure_tcp.rs`

**Key Functions:**
- `perform_kyber_handshake()`: Server-side handshake
- `perform_client_handshake()`: Client-side handshake
- `encrypt_message()`: AES-256-GCM encryption wrapper
- `decrypt_message()`: AES-256-GCM decryption wrapper

**Structs:**
- `SecureTcpServer`: Multi-client secure server
- `SecureTcpClient`: Secure client connection

### Integration Points

**MCP Server Integration:**
```rust
// Start secure server
secure_tcp::start_secure_tcp_server(mcp_server, "127.0.0.1:7439");

// Connect secure client
let client = SecureTcpClient::connect("127.0.0.1:7439");
```

**Command Line:**
```bash
# Start secure server
synapsis-mcp --tcp --secure --tcp-addr 127.0.0.1:7439

# Connect secure client  
synapsis-mcp --bridge --url 127.0.0.1:7439 --secure
```

## Performance Characteristics

### Handshake Time
- Kyber512 keypair generation: ~2ms
- Kyber512 encapsulation: ~1ms  
- Kyber512 decapsulation: ~1ms
- **Total handshake**: ~5ms

### Encryption Overhead
- AES-256-GCM encryption: <0.1ms per KB
- Base64 encoding/decoding: ~0.5ms per KB
- **Total overhead**: <1ms per typical message

### Memory Usage
- Per connection: ~5KB (keys + buffers)
- Server memory: ~1MB per 100 concurrent connections

## Compatibility Notes

### Algorithm Support
- **Required**: Kyber512, AES-256-GCM
- **Optional**: Dilithium5 (for server authentication)
- **Fallback**: None - secure mode is all-or-nothing

### Protocol Versioning
- Current: v1 (Kyber512 + AES-256-GCM)
- Future: v2 (add Dilithium5 signatures)
- Backward compatibility: Not maintained (breaking changes allowed)

## Testing Protocol

### Manual Test Commands
```bash
# Terminal 1: Start server
./target/release/synapsis-mcp --tcp --secure --tcp-addr 127.0.0.1:7440

# Terminal 2: Test handshake (will fail but shows connection)
echo "dGVzdA==" | nc localhost 7440

# Terminal 3: Full test with Python script
python3 tests/secure_handshake_test.py
```

### Automated Tests
```bash
# Run PQC integration tests
cargo test --test synapsis_pqc_integration

# Run secure TCP tests (to be implemented)
cargo test --test secure_tcp_integration
```

## Future Enhancements

### Planned Improvements
1. **Server Authentication**: Add Dilithium5 signatures
2. **Certificate Pinning**: Store server public key fingerprint
3. **Session Resumption**: Cache handshake results
4. **Protocol Negotiation**: Support multiple PQC algorithms
5. **Quantum-Safe Signatures**: Replace Dilithium with newer algorithms

### Security Hardening
1. **Timing Attack Protection**: Constant-time operations
2. **Side-Channel Resistance**: Hardware acceleration
3. **Key Rotation**: Periodic re-keying for long sessions
4. **Entropy Verification**: Validate random number generation