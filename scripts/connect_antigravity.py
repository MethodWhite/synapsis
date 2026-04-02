#!/usr/bin/env python3
import socket
import json
import sys

def connect_to_synapsis():
    host = '127.0.0.1'
    port = 7439
    
    print(f"Connecting to Synapsis TCP at {host}:{port}...")
    
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.connect((host, port))
        print("✅ Socket connected")
        
        f = s.makefile('rw')
        
        def send_request(method, params, req_id=None):
            req = {
                "jsonrpc": "2.0",
                "method": method,
                "params": params
            }
            if req_id is not None:
                req["id"] = req_id
            
            f.write(json.dumps(req) + "\n")
            f.flush()
            
            if req_id is not None:
                line = f.readline()
                return json.loads(line)
            return None

        # 1. Initialize
        print("Sending initialize...")
        resp = send_request("initialize", {
            "protocolVersion": "2024-11-05",
            "clientInfo": {"name": "Antigravity-Agent", "version": "1.0.0"}
        }, req_id=1)
        print(f"Init Response: {json.dumps(resp)}")
        
        # 2. Initialized notification
        print("Sending initialized notification...")
        send_request("initialized", {})
        
        # 3. Agent Heartbeat
        print("Sending agent_heartbeat...")
        resp = send_request("tools/call", {
            "name": "agent_heartbeat",
            "arguments": {
                "session_id": "Antigravity-Session",
                "status": "idle",
                "task": "Active and connected"
            }
        }, req_id=2)
        print(f"Heartbeat Response: {json.dumps(resp)}")
        
        # 4. Connection Status
        print("Checking connection_status...")
        resp = send_request("tools/call", {
            "name": "connection_status",
            "arguments": {}
        }, req_id=3)
        print(f"Connection Status:\n{resp['result']['content'][0]['text']}")
        
        s.close()
        return True

    except Exception as e:
        print(f"❌ Error: {e}")
        return False

if __name__ == "__main__":
    success = connect_to_synapsis()
    sys.exit(0 if success else 1)
