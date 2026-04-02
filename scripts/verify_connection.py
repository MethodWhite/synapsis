#!/usr/bin/env python3
import socket
import json
import sys

def verify_connection():
    host = '127.0.0.1'
    port = 7439
    
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.settimeout(2)
        s.connect((host, port))
        f = s.makefile('rw', buffering=1)
        
        def call_tool(name, args, req_id):
            req = {
                "jsonrpc": "2.0",
                "id": req_id,
                "method": "tools/call",
                "params": {"name": name, "arguments": args}
            }
            f.write(json.dumps(req) + "\n")
            # Clear potential stale data
            while True:
                line = f.readline()
                if not line: break
                resp = json.loads(line)
                if resp.get("id") == req_id:
                    return resp
            return None

        # 1. Initialize
        f.write(json.dumps({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"verifer"}}}) + "\n")
        f.readline()
        f.write(json.dumps({"jsonrpc":"2.0","method":"initialized","params":{}}) + "\n")

        # 2. Check active agents
        print("Checking active agents...")
        resp = call_tool("agents_active", {}, 100)
        if resp and "result" in resp:
            print(f"Active Agents: {resp['result']['content'][0]['text']}")
        else:
            print(f"Error checking agents: {resp}")

        # 3. Final connection status
        print("Checking connection status...")
        resp = call_tool("connection_status", {}, 101)
        if resp and "result" in resp:
            print(f"Status:\n{resp['result']['content'][0]['text']}")

        s.close()
    except Exception as e:
        print(f"❌ Error: {e}")

if __name__ == "__main__":
    verify_connection()
