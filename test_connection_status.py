#!/usr/bin/env python3
"""
Test connection status tracking in Synapsis MCP server.
"""

import json
import subprocess
import time
import sys
import os

def test_connection_status():
    """Test that connection status tracking works with emoji indicators."""
    
    # Path to MCP server binary
    binary_path = "./target/debug/synapsis-mcp"
    if not os.path.exists(binary_path):
        print(f"Error: Binary not found at {binary_path}")
        print("Run: cargo build --bin synapsis-mcp")
        return False
    
    print("Starting MCP server test...")
    print(f"Binary: {binary_path}")
    
    # Start the server
    proc = subprocess.Popen(
        [binary_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    try:
        # Give server time to initialize
        time.sleep(0.5)
        
        # Read initial stderr output (debug messages)
        stderr_output = ""
        while True:
            line = proc.stderr.readline()
            if not line:
                break
            stderr_output += line
            if "[MCP] Rust Server Initialized" in line:
                break
        
        print(f"Server started. Initial output: {stderr_output[:100]}...")
        
        # Test 1: Initialize connection
        init_request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "clientInfo": {
                    "name": "test-client"
                }
            }
        }
        
        print(f"\nSending initialize request: {json.dumps(init_request)}")
        proc.stdin.write(json.dumps(init_request) + "\n")
        proc.stdin.flush()
        
        # Read response
        init_response = proc.stdout.readline()
        print(f"Initialize response: {init_response.strip()}")
        
        # Parse response to check if successful
        try:
            init_json = json.loads(init_response)
            if "result" in init_json:
                print("✅ Initialize successful")
            else:
                print("❌ Initialize failed")
                return False
        except json.JSONDecodeError:
            print(f"❌ Invalid JSON response: {init_response}")
            return False
        
        # Test 2: Call connection_status tool
        status_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "connection_status",
                "arguments": {}
            }
        }
        
        print(f"\nSending connection_status request: {json.dumps(status_request)}")
        proc.stdin.write(json.dumps(status_request) + "\n")
        proc.stdin.flush()
        
        # Read response
        status_response = proc.stdout.readline()
        print(f"Connection status response: {status_response.strip()}")
        
        # Parse and check for emoji indicators
        try:
            status_json = json.loads(status_response)
            if "result" in status_json:
                content = status_json["result"].get("content", [{}])[0].get("text", "")
                print(f"Status content: {content[:100]}...")
                
                # Check for emoji indicators
                if "🟢" in content or "🟡" in content or "🔴" in content:
                    print("✅ Connection status shows emoji indicators")
                    
                    # Check for our test client
                    if "test-client" in content:
                        print("✅ Our test client appears in connection list")
                    else:
                        print("⚠️  Test client not found in connection list")
                else:
                    print("❌ No emoji indicators found in connection status")
                    return False
            else:
                print("❌ Connection status request failed")
                return False
        except (json.JSONDecodeError, KeyError, IndexError) as e:
            print(f"❌ Error parsing status response: {e}")
            print(f"Raw response: {status_response}")
            return False
        
        # Test 3: Send agent_heartbeat to update activity
        heartbeat_request = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "agent_heartbeat",
                "arguments": {
                    "session_id": "test-session",
                    "status": "busy",
                    "task": "Testing connection status"
                }
            }
        }
        
        print(f"\nSending agent_heartbeat request: {json.dumps(heartbeat_request)}")
        proc.stdin.write(json.dumps(heartbeat_request) + "\n")
        proc.stdin.flush()
        
        # Read response
        heartbeat_response = proc.stdout.readline()
        print(f"Heartbeat response: {heartbeat_response.strip()}")
        
        try:
            heartbeat_json = json.loads(heartbeat_response)
            if "result" in heartbeat_json:
                print("✅ Heartbeat successful")
            else:
                print("⚠️  Heartbeat may have failed")
        except json.JSONDecodeError:
            print(f"⚠️  Could not parse heartbeat response: {heartbeat_response}")
        
        # Test 4: Check connection status again (should show updated activity)
        status_request2 = {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "connection_status",
                "arguments": {}
            }
        }
        
        print(f"\nSending second connection_status request")
        proc.stdin.write(json.dumps(status_request2) + "\n")
        proc.stdin.flush()
        
        status_response2 = proc.stdout.readline()
        print(f"Second status response: {status_response2[:100]}...")
        
        print("\n" + "="*60)
        print("✅ All tests completed successfully!")
        print("="*60)
        
        return True
        
    except Exception as e:
        print(f"❌ Test failed with exception: {e}")
        import traceback
        traceback.print_exc()
        return False
    
    finally:
        # Close stdin to signal EOF to server
        proc.stdin.close()
        
        # Wait for process to exit
        try:
            proc.wait(timeout=2)
            print(f"\nServer exited with code: {proc.returncode}")
        except subprocess.TimeoutExpired:
            print("\nServer did not exit, terminating...")
            proc.terminate()
            try:
                proc.wait(timeout=1)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()

if __name__ == "__main__":
    success = test_connection_status()
    sys.exit(0 if success else 1)