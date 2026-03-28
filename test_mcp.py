#!/usr/bin/env python3
import subprocess
import json
import time
import sys
import os

def test_mcp_server():
    # Iniciar el servidor MCP en modo stdio
    proc = subprocess.Popen(
        ["./target/release/synapsis-mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    # Dar tiempo para que el servidor inicialice
    time.sleep(0.5)
    
    # Enviar solicitud de inicialización
    init_request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            },
            "capabilities": {}
        }
    }
    
    print("Enviando initialize...")
    proc.stdin.write(json.dumps(init_request) + "\n")
    proc.stdin.flush()
    
    # Leer respuesta
    line = proc.stdout.readline()
    if line:
        response = json.loads(line.strip())
        print(f"Respuesta recibida: {json.dumps(response, indent=2)}")
        if "result" in response:
            print("✓ Initialize exitoso")
        else:
            print("✗ Initialize falló")
            print(f"Error: {response.get('error')}")
    else:
        print("✗ No se recibió respuesta")
        # Leer stderr
        for err_line in proc.stderr:
            print(f"stderr: {err_line}", end="")
    
    # Enviar solicitud de shutdown (opcional)
    shutdown_request = {
        "jsonrpc": "2.0",
        "id": 2,
        "method": "shutdown",
        "params": None
    }
    print("Enviando shutdown...")
    proc.stdin.write(json.dumps(shutdown_request) + "\n")
    proc.stdin.flush()
    
    # Leer respuesta de shutdown
    line = proc.stdout.readline()
    if line:
        response = json.loads(line.strip())
        print(f"Respuesta shutdown: {json.dumps(response, indent=2)}")
    
    # Dar tiempo para que el servidor cierre
    time.sleep(0.2)
    
    # Terminar proceso
    proc.terminate()
    proc.wait(timeout=2)
    print("Prueba completada")

if __name__ == "__main__":
    test_mcp_server()