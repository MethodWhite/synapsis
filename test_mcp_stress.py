#!/usr/bin/env python3
import subprocess
import json
import time
import sys
import threading

def read_stderr(proc):
    for line in iter(proc.stderr.readline, ''):
        sys.stderr.write(f"[SERVER] {line}")
        sys.stderr.flush()

def stress_test():
    print("=== Prueba de estrés MCP - 10 iteraciones ===")
    proc = subprocess.Popen(
        ["./target/release/synapsis-mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    # Leer stderr en hilo
    stderr_thread = threading.Thread(target=read_stderr, args=(proc,), daemon=True)
    stderr_thread.start()
    
    time.sleep(0.5)
    
    def send(req):
        proc.stdin.write(json.dumps(req) + "\n")
        proc.stdin.flush()
    
    def read():
        line = proc.stdout.readline()
        if not line:
            return None
        return json.loads(line.strip())
    
    # Initialize
    send({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "clientInfo": {"name": "stress-test"},
            "capabilities": {}
        }
    })
    resp = read()
    if not resp or "error" in resp:
        print("✗ Initialize falló")
        return False
    print("✓ Initialize OK")
    
    # Initialized notification
    send({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    })
    print("✓ Initialized enviado")
    
    # Loop de solicitudes
    success_count = 0
    for i in range(10):
        time.sleep(0.5)  # Esperar medio segundo entre solicitudes
        req_id = i + 2
        send({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/list",
            "params": {}
        })
        resp = read()
        if resp and "result" in resp:
            success_count += 1
            tools = len(resp["result"].get("tools", []))
            print(f"  Iteración {i+1}: OK ({tools} herramientas)")
        else:
            print(f"  Iteración {i+1}: FAIL")
            if resp and "error" in resp:
                print(f"    Error: {resp['error']}")
    
    print(f"\n✅ {success_count}/10 solicitudes exitosas")
    
    # Shutdown
    send({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "shutdown",
        "params": None
    })
    resp = read()
    if resp and "result" in resp:
        print("✓ Shutdown OK")
    else:
        print("✗ Shutdown falló")
    
    time.sleep(0.2)
    if proc.poll() is None:
        proc.terminate()
        proc.wait(timeout=2)
    
    return success_count == 10

if __name__ == "__main__":
    if stress_test():
        print("\n✅ Prueba de estrés PASADA")
        sys.exit(0)
    else:
        print("\n❌ Prueba de estrés FALLADA")
        sys.exit(1)