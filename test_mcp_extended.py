#!/usr/bin/env python3
import subprocess
import json
import time
import sys
import threading

def read_output(proc, output_lines):
    """Leer stdout en segundo plano"""
    for line in iter(proc.stdout.readline, ''):
        output_lines.append(line.strip())
        print(f"[SERVER] {line.strip()}")

def test_extended():
    print("=== Iniciando prueba extensa de servidor MCP ===")
    
    # Iniciar servidor
    proc = subprocess.Popen(
        ["./target/release/synapsis-mcp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    # Hilo para leer stderr
    stderr_lines = []
    def read_stderr():
        for line in iter(proc.stderr.readline, ''):
            stderr_lines.append(line.strip())
            print(f"[STDERR] {line.strip()}")
    
    stderr_thread = threading.Thread(target=read_stderr, daemon=True)
    stderr_thread.start()
    
    time.sleep(0.5)  # Esperar inicialización
    
    request_id = 1
    
    def send_request(method, params=None):
        nonlocal request_id
        req = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params or {}
        }
        proc.stdin.write(json.dumps(req) + "\n")
        proc.stdin.flush()
        request_id += 1
        return req["id"]
    
    def read_response(expected_id=None):
        # Leer línea de stdout
        line = proc.stdout.readline()
        if not line:
            return None
        try:
            resp = json.loads(line.strip())
            if expected_id and resp.get("id") != expected_id:
                print(f"⚠ ID mismatch: expected {expected_id}, got {resp.get('id')}")
            return resp
        except json.JSONDecodeError:
            print(f"⚠ Invalid JSON: {line}")
            return None
    
    # 1. Initialize
    print("\n1. Enviando initialize...")
    init_id = send_request("initialize", {
        "protocolVersion": "2024-11-05",
        "clientInfo": {
            "name": "test-client",
            "version": "1.0.0"
        },
        "capabilities": {}
    })
    
    init_resp = read_response(init_id)
    if init_resp and "result" in init_resp:
        print("✓ Initialize exitoso")
        print(f"   Server: {init_resp['result']['serverInfo']}")
    else:
        print("✗ Initialize falló")
        if init_resp and "error" in init_resp:
            print(f"   Error: {init_resp['error']}")
        return False
    
    # 2. Enviar notificación initialized (sin id)
    print("\n2. Enviando notificación initialized...")
    initialized_notification = {
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    }
    proc.stdin.write(json.dumps(initialized_notification) + "\n")
    proc.stdin.flush()
    print("✓ Notificación enviada")
    
    # 3. Listar herramientas
    print("\n3. Enviando tools/list...")
    tools_id = send_request("tools/list", {})
    tools_resp = read_response(tools_id)
    if tools_resp and "result" in tools_resp:
        tools = tools_resp["result"].get("tools", [])
        print(f"✓ Se encontraron {len(tools)} herramientas")
        for tool in tools[:5]:  # Mostrar primeras 5
            print(f"   - {tool.get('name')}: {tool.get('description')}")
        if len(tools) > 5:
            print(f"   ... y {len(tools) - 5} más")
    else:
        print("✗ tools/list falló")
        if tools_resp and "error" in tools_resp:
            print(f"   Error: {tools_resp['error']}")
    
    # 4. Listar recursos
    print("\n4. Enviando resources/list...")
    resources_id = send_request("resources/list", {})
    resources_resp = read_response(resources_id)
    if resources_resp and "result" in resources_resp:
        resources = resources_resp["result"].get("resources", [])
        print(f"✓ Se encontraron {len(resources)} recursos")
    else:
        print("✗ resources/list falló")
    
    # 5. Probar una herramienta específica (skill_list)
    print("\n5. Enviando tools/call para skill_list...")
    call_id = send_request("tools/call", {
        "name": "skill_list",
        "arguments": {}
    })
    call_resp = read_response(call_id)
    if call_resp and "result" in call_resp:
        print(f"✓ Tool call exitoso")
        content = call_resp["result"].get("content", [])
        if isinstance(content, list):
            print(f"   {len(content)} skills listados")
        else:
            print(f"   Resultado: {content}")
    else:
        print("✗ tools/call falló")
        if call_resp and "error" in call_resp:
            print(f"   Error: {call_resp['error']}")
    
    # 6. Esperar 2 segundos para verificar que la conexión sigue activa
    print("\n6. Esperando 2 segundos...")
    time.sleep(2)
    
    # 7. Enviar ping (no es parte de MCP, pero podemos enviar una solicitud trivial)
    print("\n7. Enviando tools/list nuevamente...")
    tools2_id = send_request("tools/list", {})
    tools2_resp = read_response(tools2_id)
    if tools2_resp and "result" in tools2_resp:
        print("✓ Conexión todavía activa")
    else:
        print("✗ Conexión perdida")
    
    # 8. Shutdown
    print("\n8. Enviando shutdown...")
    shutdown_id = send_request("shutdown", None)
    shutdown_resp = read_response(shutdown_id)
    if shutdown_resp and "result" in shutdown_resp:
        print("✓ Shutdown exitoso")
    else:
        print("✗ Shutdown falló")
    
    # Esperar un poco
    time.sleep(0.3)
    
    # Verificar si el proceso sigue vivo
    if proc.poll() is None:
        print("⚠ Proceso aún vivo, terminando...")
        proc.terminate()
        proc.wait(timeout=2)
    
    print("\n=== Prueba completada ===")
    return True

if __name__ == "__main__":
    success = test_extended()
    sys.exit(0 if success else 1)