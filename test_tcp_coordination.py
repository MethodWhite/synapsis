#!/usr/bin/env python3
"""
Test coordination with agents via TCP MCP server.
"""

import json
import socket
import time
import sys

def send_tcp_request(request_json, host='127.0.0.1', port=7438):
    """Send JSON-RPC request via TCP and get response."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5.0)
    try:
        sock.connect((host, port))
        # Send request with newline delimiter
        request_str = json.dumps(request_json) + '\n'
        sock.sendall(request_str.encode('utf-8'))
        # Receive response (assuming it ends with newline)
        response_bytes = b''
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            response_bytes += chunk
            if b'\n' in chunk:
                break
        response_str = response_bytes.decode('utf-8').strip()
        # Might be multiple lines; take the last complete JSON
        lines = response_str.split('\n')
        for line in reversed(lines):
            if line.strip():
                return json.loads(line.strip())
        return json.loads(lines[-1].strip())
    except Exception as e:
        print(f"TCP error: {e}")
        return None
    finally:
        sock.close()

def test_coordination():
    print("Testing coordination with agents via TCP MCP server...")
    
    # Step 1: Initialize connection
    init_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "clientInfo": {
                "name": "coordinator-agent",
                "version": "1.0"
            }
        }
    }
    
    print("Sending initialize...")
    init_response = send_tcp_request(init_request)
    if not init_response or 'result' not in init_response:
        print(f"Initialize failed: {init_response}")
        return False
    print("✅ Initialize successful")
    
    # Step 2: Check connection status
    status_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "connection_status",
            "arguments": {}
        }
    }
    
    print("\nChecking connection status...")
    status_response = send_tcp_request(status_request)
    if status_response and 'result' in status_response:
        content = status_response['result'].get('content', [{}])[0].get('text', '')
        print(f"Connection status:\n{content}")
        # Look for PQC agents in the list
        if 'pqc' in content.lower():
            print("✅ PQC agents detected in connections")
        else:
            print("⚠️ No PQC agents in connection list (they may not have reconnected yet)")
    else:
        print(f"Connection status failed: {status_response}")
    
    # Step 3: Send heartbeat to register our session
    heartbeat_request = {
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "agent_heartbeat",
            "arguments": {
                "session_id": "coordinator-session",
                "status": "busy",
                "task": "Coordinating with all agents"
            }
        }
    }
    
    print("\nSending agent heartbeat...")
    heartbeat_response = send_tcp_request(heartbeat_request)
    if heartbeat_response and 'result' in heartbeat_response:
        print("✅ Heartbeat registered")
    else:
        print(f"Heartbeat response: {heartbeat_response}")
    
    # Step 4: Send message to PQC worker agent
    # Get PQC worker session ID from database or connection status
    # For now, use the session ID we saw earlier
    pqc_worker_session = "pqc-worker-7d91b1e2023a4abf-bd0aea69e4-fe8fa2-1774333994"
    
    message_request = {
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "send_message",
            "arguments": {
                "session_id": "coordinator-session",
                "to": pqc_worker_session,
                "content": "Hola equipo! 👋 ¿Cómo les va con las tareas PQC? Soy el coordinador usando el nuevo MCP server. ¿Todo bien?"
            }
        }
    }
    
    print(f"\nSending message to PQC worker {pqc_worker_session}...")
    message_response = send_tcp_request(message_request)
    if message_response and 'result' in message_response:
        print("✅ Message sent successfully")
    else:
        print(f"Message response: {message_response}")
    
    # Step 5: Check agent details for PQC worker
    details_request = {
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "agent_details",
            "arguments": {
                "session_id": pqc_worker_session
            }
        }
    }
    
    print(f"\nGetting agent details for {pqc_worker_session}...")
    details_response = send_tcp_request(details_request)
    if details_response and 'result' in details_response:
        content = details_response['result'].get('content', [{}])[0].get('text', '')
        print(f"Agent details: {content}")
    else:
        print(f"Details response: {details_response}")
    
    # Step 6: Check task queue status
    task_request = {
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "task_queue_status",
            "arguments": {}
        }
    }
    
    print("\nChecking task queue status...")
    task_response = send_tcp_request(task_request)
    if task_response and 'result' in task_response:
        content = task_response['result'].get('content', [{}])[0].get('text', '')
        print(f"Task queue: {content}")
    else:
        print(f"Task queue response: {task_response}")
    
    print("\n" + "="*60)
    print("Coordination test completed!")
    print("="*60)
    return True

if __name__ == "__main__":
    success = test_coordination()
    sys.exit(0 if success else 1)