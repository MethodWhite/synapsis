#!/usr/bin/env python3
"""
Synapsis Session Share - Cross-platform session sharing CLI.

Connects to Synapsis MCP server via HTTP/JSON-RPC to share observations
between sessions across different AI tools (opencode, cursor, etc.).

Usage:
  ./session-share.sh list                          List all active sessions
  ./session-share.sh by-project <project>          List sessions in project
  ./session-share.sh broadcast <msg>               Broadcast to all sessions in same project
  ./session-share.sh status                        Check if bridge is active
  ./session-share.sh start <project> [directory]   Start a new session
  ./session-share.sh end <session_id> [summary]    End a session
  ./session-share.sh summary <session_id>          Get session summary
"""

import json
import sys
import subprocess
import os
import time

SYNAPSIS_URL = os.environ.get("SYNAPSIS_URL", "http://127.0.0.1:7438")
MESSAGE_URL = f"{SYNAPSIS_URL}/message"

def mcp_request(method_name, arguments=None):
    """Send a tools/call JSON-RPC request to the Synapsis MCP server."""
    if arguments is None:
        arguments = {}
    payload = {
        "jsonrpc": "2.0",
        "method": "tools/call",
        "params": {
            "name": method_name,
            "arguments": arguments,
        },
        "id": int(time.time() * 1000) % 1000000,
    }
    try:
        result = subprocess.run(
            ["curl", "-s", "-X", "POST", MESSAGE_URL,
             "-H", "Content-Type: application/json",
             "-d", json.dumps(payload)],
            capture_output=True, text=True, timeout=10,
        )
        if result.returncode != 0:
            return {"error": f"curl failed: {result.stderr.strip()}"}
        resp = json.loads(result.stdout)
        if "error" in resp:
            return {"error": resp["error"].get("message", str(resp["error"]))}
        content = resp.get("result", {}).get("content", [])
        texts = [c.get("text", "") for c in content if c.get("type") == "text"]
        return {"ok": True, "text": "\n".join(texts), "raw": resp}
    except json.JSONDecodeError as e:
        return {"error": f"Invalid JSON response: {e}"}
    except subprocess.TimeoutExpired:
        return {"error": "Request timed out"}
    except Exception as e:
        return {"error": str(e)}


def raw_request(method, params=None):
    """Send a raw JSON-RPC request (non-tools/call)."""
    if params is None:
        params = {}
    payload = {
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": int(time.time() * 1000) % 1000000,
    }
    try:
        result = subprocess.run(
            ["curl", "-s", "-X", "POST", MESSAGE_URL,
             "-H", "Content-Type: application/json",
             "-d", json.dumps(payload)],
            capture_output=True, text=True, timeout=10,
        )
        if result.returncode != 0:
            return {"error": f"curl failed: {result.stderr.strip()}"}
        resp = json.loads(result.stdout)
        return resp
    except Exception as e:
        return {"error": str(e)}


def cmd_status():
    """Check if the Synapsis MCP bridge is active."""
    resp = raw_request("initialize")
    if "error" in resp:
        print(f"BRIDGE: DOWN ({resp['error'].get('message', resp['error'])})")
        return False
    info = resp.get("result", {}).get("serverInfo", {})
    print(f"BRIDGE: ACTIVE")
    print(f"Server: {info.get('name', 'synapsis')} v{info.get('version', '?')}")
    print(f"Protocol: {resp.get('result', {}).get('protocolVersion', '?')}")
    print(f"Endpoint: {MESSAGE_URL}")
    return True


def cmd_list():
    """List active agents/sessions using agent_list tool."""
    resp = mcp_request("agent_list")
    if "error" in resp:
        print(f"Error: {resp['error']}")
        return

    text = resp.get("text", "No agents found.")
    print("Active Sessions\n================")
    print(text)

    resp2 = mcp_request("mem_stats")
    if "ok" in resp2:
        print(f"\nMemory Stats:\n{resp2['text']}")


def cmd_by_project(project):
    """List sessions in a specific project."""
    resp = mcp_request("agent_list_by_project", {"project": project})
    if "error" in resp:
        print(f"Error: {resp['error']}")
        return
    print(f"Sessions in project '{project}'\n{'=' * 40}")
    print(resp.get("text", "No sessions found."))

    resp2 = mcp_request("mem_current_project", {"project": project})
    if "ok" in resp2:
        print(f"\n{resp2['text']}")


def cmd_broadcast(msg):
    """Broadcast a message to all sessions by saving as shared observation."""
    title = f"broadcast:{time.strftime('%Y-%m-%d %H:%M:%S')}"
    resp = mcp_request("mem_save", {
        "title": title,
        "content": msg,
        "type": "discovery",
        "project": "shared",
        "scope": "project",
        "session_id": "session-share",
    })
    if "error" in resp:
        print(f"Broadcast failed: {resp['error']}")
        return
    print(f"Broadcast sent: '{title}'")
    print(resp.get("text", ""))


def cmd_start(project, directory="."):
    """Start a new session."""
    resp = mcp_request("mem_session_start", {
        "project": project,
        "directory": directory,
    })
    if "error" in resp:
        print(f"Error: {resp['error']}")
        return
    print(resp.get("text", f"Session started in project '{project}'"))


def cmd_end(session_id, summary=None):
    """End a session."""
    args = {"session_id": session_id}
    if summary:
        args["summary"] = summary
    resp = mcp_request("mem_session_end", args)
    if "error" in resp:
        print(f"Error: {resp['error']}")
        return
    print(resp.get("text", f"Session {session_id} ended."))


def cmd_summary(session_id):
    """Get session summary."""
    resp = mcp_request("mem_session_summary", {"session_id": session_id})
    if "error" in resp:
        print(f"Error: {resp['error']}")
        return
    print(resp.get("text", f"No summary for session {session_id}."))


def usage():
    print(__doc__)
    sys.exit(1)


if __name__ == "__main__":
    if len(sys.argv) < 2:
        usage()

    command = sys.argv[1]

    if command == "status":
        cmd_status()
    elif command == "list":
        cmd_list()
    elif command == "by-project" and len(sys.argv) >= 3:
        cmd_by_project(sys.argv[2])
    elif command == "broadcast" and len(sys.argv) >= 3:
        cmd_broadcast(sys.argv[2])
    elif command == "start" and len(sys.argv) >= 3:
        cmd_start(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else ".")
    elif command == "end" and len(sys.argv) >= 3:
        cmd_end(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
    elif command == "summary" and len(sys.argv) >= 3:
        cmd_summary(sys.argv[2])
    else:
        usage()
