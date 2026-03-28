#!/usr/bin/env python3
import subprocess
import json
import time
import sys

def test():
    proc = subprocess.Popen(
        ["./target/release/synapsis-mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    time.sleep(0.5)
    
    # Initialize
    init = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "clientInfo": {"name": "test"},
            "capabilities": {}
        }
    }
    proc.stdin.write(json.dumps(init) + "\n")
    proc.stdin.flush()
    
    # Read response
    line = proc.stdout.readline()
    print(f"Init response: {line}")
    
    # Send initialized notification
    initialized = {
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    }
    proc.stdin.write(json.dumps(initialized) + "\n")
    proc.stdin.flush()
    print("Sent initialized")
    
    # Wait a bit
    time.sleep(0.1)
    
    # Send tools/list
    tools = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }
    proc.stdin.write(json.dumps(tools) + "\n")
    proc.stdin.flush()
    
    # Read response
    line = proc.stdout.readline()
    print(f"Tools response: {line}")
    
    # Shutdown
    shutdown = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "shutdown",
        "params": None
    }
    proc.stdin.write(json.dumps(shutdown) + "\n")
    proc.stdin.flush()
    
    line = proc.stdout.readline()
    print(f"Shutdown response: {line}")
    
    proc.terminate()
    proc.wait()

if __name__ == "__main__":
    test()