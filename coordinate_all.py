#!/usr/bin/env python3
"""
Coordinate with all active agents via Synapsis MCP TCP server.
"""
import json
import socket
import sys

def send_tcp(request):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.settimeout(5)
    try:
        sock.connect(('127.0.0.1', 7438))
        sock.sendall((json.dumps(request) + '\n').encode())
        response = b''
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            response += chunk
            if b'\n' in chunk:
                break
        return json.loads(response.decode().strip())
    except Exception as e:
        print(f"Error: {e}")
        return None
    finally:
        sock.close()

def main():
    print("🔄 Coordinating with all agents...")
    
    # 1. Get active agents
    agents_req = {
        "jsonrpc": "2.0",
        "method": "agents_active",
        "params": {},
        "id": 1
    }
    resp = send_tcp(agents_req)
    if not resp or 'result' not in resp:
        print("❌ Failed to get agents")
        return
    
    agents = resp['result'].get('agents', [])
    print(f"📊 Found {len(agents)} active agents")
    
    # 2. Register our own coordinator session
    register_req = {
        "jsonrpc": "2.0",
        "method": "session_register",
        "params": {
            "arguments": {
                "agent_type": "coordinator",
                "project": "default"
            }
        },
        "id": 2
    }
    resp = send_tcp(register_req)
    if resp and 'result' in resp:
        my_session = resp['result']['session_id']
        print(f"✅ Coordinator session: {my_session}")
    else:
        print("⚠️ Could not register coordinator session, using placeholder")
        my_session = "coordinator-" + str(hash('temp'))
    
    # 3. Send heartbeat for coordinator
    heartbeat_req = {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "agent_heartbeat",
            "arguments": {
                "session_id": my_session,
                "status": "busy",
                "task": "Coordinating with all agents"
            }
        },
        "id": 3
    }
    send_tcp(heartbeat_req)
    
    # 4. Send broadcast message to each agent
    messages_sent = 0
    for agent in agents[:10]:  # limit to first 10
        session_id = agent['session_id']
        agent_type = agent['agent_type']
        instance = agent['instance']
        
        message_req = {
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "send_message",
                "arguments": {
                    "session_id": my_session,
                    "to": session_id,
                    "content": f"👋 Hola {agent_type} ({instance})! Soy el coordinador del nuevo servidor MCP. ¿Todo bien por ahí? 🚀"
                }
            },
            "id": 1000 + messages_sent
        }
        resp = send_tcp(message_req)
        if resp and 'result' in resp:
            print(f"   📨 Message sent to {agent_type}-{instance}")
            messages_sent += 1
        else:
            print(f"   ⚠️ Failed to send to {agent_type}-{instance}")
    
    # 5. Create a coordination task
    task_req = {
        "jsonrpc": "2.0",
        "method": "task_create",
        "params": {
            "arguments": {
                "project": "default",
                "task_type": "coordination",
                "payload": "{\"message\": \"Team sync: please update your status and report any issues.\", \"priority\": 1}",
                "priority": 1
            }
        },
        "id": 4
    }
    resp = send_tcp(task_req)
    if resp and 'result' in resp:
        task_id = resp['result']['task_id']
        print(f"✅ Created coordination task: {task_id}")
    else:
        print("⚠️ Could not create task")
    
    # 6. Check connection status
    status_req = {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": "connection_status",
            "arguments": {}
        },
        "id": 5
    }
    resp = send_tcp(status_req)
    if resp and 'result' in resp:
        content = resp['result']['content'][0]['text']
        print(f"📡 Connection status:\n{content}")
    
    print(f"\n🎉 Coordination complete! Sent {messages_sent} messages to agents.")
    print("   Agents should receive messages via event bus.")
    print("   Check agent heartbeats for updates.")

if __name__ == "__main__":
    main()