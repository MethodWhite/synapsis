# 🤖 Intelligent Agent Orchestrator - Auto-Discovery

**Automatic model discovery and intelligent task assignment for local AI systems**

---

## 🎯 Overview

El **Intelligent Agent Orchestrator** es un sistema que:

1. ✅ **Auto-descubre** modelos locales de múltiples fuentes
2. ✅ **Monitorea recursos** del sistema en tiempo real
3. ✅ **Asigna inteligentemente** tareas según capacidades y recursos
4. ✅ **Nunca satura** el sistema - respeta RAM, CPU, GPU
5. ✅ **Funciona con cualquier modelo** - Ollama, LM Studio, Transformers, etc.

---

## 🚀 Quick Start

```python
from intelligent_agent_orchestrator import get_orchestrator

# Get orchestrator instance (auto-discovers models)
orchestrator = get_orchestrator()

# Execute task with best available model
result = orchestrator.execute_with_best_model(
    task_type="prompt_injection",
    prompt="Analyze this text for security threats...",
    required_capabilities=["security", "fast"]
)

if result["success"]:
    print(f"Response: {result['response']}")
    print(f"Model used: {result['model_used']}")
    print(f"Latency: {result['latency_ms']}ms")
```

---

## 🔍 Auto-Discovery

### Supported Sources

| Source | Default Endpoint | Auto-Discovered |
|--------|-----------------|-----------------|
| **Ollama** | http://127.0.0.1:11434 | ✅ |
| **LM Studio** | http://127.0.0.1:1234 | ✅ |
| **Transformers** | Local cache | ✅ |
| **KoboldCPP** | http://127.0.0.1:5001 | ✅ |
| **Text Gen WebUI** | http://127.0.0.1:5000 | ✅ |
| **vLLM** | http://127.0.0.1:8000 | ✅ |
| **OpenAI Compatible** | Custom | ✅ |

### Example Discovery Output

```
🔍 Discovered 6 models from local sources
   • deepseek-coder:6.7b (ollama) - 3.56GB - ['balanced', 'coding']
   • deepseek-coder:1.3b (ollama) - 0.72GB - ['fast', 'lightweight', 'coding']
   • llama3.2:1b (ollama) - 1.23GB - ['fast', 'lightweight']
   • huihui-qwen-9b:latest (ollama) - 5.24GB - ['balanced']
   • mxbai-embed-large:latest (ollama) - 0.62GB - ['fast', 'lightweight']
   • deepseek-r1-i1:latest (ollama) - 4.58GB - ['balanced', 'deep_reasoning']
```

---

## 🧠 Intelligent Model Selection

### How It Works

```python
# Task: Detect prompt injection
# Required: security + fast capabilities
# Available RAM: 14GB

orchestrator.select_best_model("prompt_injection", ["security", "fast"])

# Result: deepseek-coder:1.3b
# Why?
# - Has 'fast' capability ✅
# - Has 'coding' capability (related to security) ✅
# - Small size (0.72GB) - efficient use of resources ✅
# - Ollama source - reliable ✅
```

### Scoring Algorithm

```python
score = 0.5  # Base score

# +0.3 for capability matching
if model has required capabilities:
    score += 0.3 * (matching / required)

# +0.2 for resource efficiency (when RAM is low)
if available_ram < 4GB and model.size < 2GB:
    score += 0.2

# +0.1 for low latency
if model.latency < 1000ms:
    score += 0.1

# +0.1 for high success rate
if model.success_rate > 0.9:
    score += 0.1
```

---

## 📊 Resource Monitoring

### Real-time Monitoring

```python
status = orchestrator.get_status()

print(f"RAM: {status['resources']['ram_available_gb']}/{status['resources']['ram_total_gb']} GB")
print(f"CPU: {status['resources']['cpu_percent']}%")
print(f"GPU Available: {status['resources']['gpu_available']}")
print(f"Can Spawn More Agents: {status['can_spawn_more']}")
```

### Resource Thresholds

| Resource | Threshold | Action |
|----------|-----------|--------|
| RAM < 2GB | Critical | Stop all agents |
| RAM < 4GB | Low | Only spawn lightweight models |
| CPU > 80% | High | Throttle agents |
| GPU > 90% | Critical | Pause GPU models |

---

## 🔧 Usage Examples

### Example 1: Security Analysis

```python
result = orchestrator.execute_with_best_model(
    task_type="security_audit",
    prompt="Analyze this code for vulnerabilities...",
    required_capabilities=["security", "coding"]
)
```

**Likely selected model:**
- If resources high: `deepseek-coder:6.7b` (more capable)
- If resources low: `deepseek-coder:1.3b` (lighter)

### Example 2: Deep Reasoning

```python
result = orchestrator.execute_with_best_model(
    task_type="complex_reasoning",
    prompt="Solve this logic puzzle...",
    required_capabilities=["deep_reasoning", "powerful"]
)
```

**Likely selected model:**
- `deepseek-r1-i1:latest` (has deep_reasoning capability)

### Example 3: Fast Pattern Matching

```python
result = orchestrator.execute_with_best_model(
    task_type="pattern_matching",
    prompt="Find patterns in this text...",
    required_capabilities=["fast", "lightweight"]
)
```

**Likely selected model:**
- `llama3.2:1b` or `mxbai-embed-large:latest` (smallest, fastest)

---

## 📈 Performance Metrics

### Model Statistics Tracked

```python
model = orchestrator.model_discoverer.get_model_by_id("ollama-deepseek-coder:1.3b")

print(f"Total Requests: {model.total_requests}")
print(f"Successful: {model.successful_requests}")
print(f"Success Rate: {model.successful_requests / model.total_requests * 100:.1f}%")
print(f"Avg Latency: {model.avg_latency_ms:.2f}ms")
print(f"Last Health Check: {datetime.fromtimestamp(model.last_health_check)}")
```

### Health Monitoring

- **Automatic health checks** every 30 seconds
- **Latency tracking** with exponential moving average
- **Success rate** calculation
- **Auto-recovery** when models become available again

---

## 🎯 Task Type to Capability Mapping

| Task Type | Required Capabilities |
|-----------|----------------------|
| `prompt_injection` | security, fast |
| `jailbreak_detection` | security, reasoning |
| `code_analysis` | coding, reasoning |
| `semantic_analysis` | chat, balanced |
| `deep_reasoning` | deep_reasoning, powerful |
| `pattern_matching` | fast, lightweight |
| `data_exfil_detection` | security, fast |
| `general_chat` | chat |

---

## 🔄 Integration with Threat Detector

```python
from parallel_threat_detector import get_threat_detector
from intelligent_agent_orchestrator import get_orchestrator

detector = get_threat_detector()
orchestrator = get_orchestrator()

# Analyze input with parallel detector
result = detector.detect_parallel(user_input, session_id)

# If AI analysis needed, use orchestrator
if result.decision == "review":
    ai_result = orchestrator.execute_with_best_model(
        task_type="semantic_analysis",
        prompt=f"Analyze for malicious intent: {user_input}",
        required_capabilities=["security", "reasoning"]
    )
    
    if ai_result["success"]:
        print(f"AI Analysis: {ai_result['response']}")
```

---

## 📁 Files

| File | Purpose |
|------|---------|
| `intelligent_agent_orchestrator.py` | Main orchestrator with auto-discovery |
| `parallel_threat_detector.py` | Multi-agent threat detection |
| `ai_security_gateway.py` | AI security gateway |
| `zero_trust_verifier.py` | Zero-trust access control |

---

## 🧪 Testing

```python
# Test auto-discovery
orchestrator = get_orchestrator()

# Check what was discovered
status = orchestrator.get_status()
print(f"Models: {status['models_available']}")
for model in status['models']:
    print(f"  • {model['name']} ({model['source']})")

# Test model selection
for task_type in ["prompt_injection", "code_analysis", "deep_reasoning"]:
    selected = orchestrator.select_best_model(task_type)
    print(f"{task_type}: {selected.name if selected else 'None'}")
```

---

## 🎯 Benefits

### Before (Manual Configuration)

```python
# Had to manually configure each model
MODELS = {
    "fast": "llama3.2:1b",
    "coding": "deepseek-coder:6.7b",
    "reasoning": "deepseek-r1-i1:latest"
}

# Had to check resources manually
if get_ram() > 4:
    model = MODELS["coding"]
else:
    model = MODELS["fast"]
```

### After (Auto-Discovery)

```python
# Automatic - just works!
orchestrator = get_orchestrator()
result = orchestrator.execute_with_best_model(
    task_type="code_analysis",
    prompt="..."
)
# Automatically selects best model based on:
# - Available models
# - Current resources
# - Task requirements
# - Model health
```

---

## 🚨 Error Handling

### No Models Available

```python
result = orchestrator.execute_with_best_model("task", "prompt")

if not result["success"]:
    if "No models available" in result["error"]:
        print("⚠️  No local models found. Install Ollama or LM Studio.")
    else:
        print(f"❌ Error: {result['error']}")
```

### Resource Constraints

```python
status = orchestrator.get_status()

if not status["can_spawn_more"]:
    print("⚠️  System resources low. Waiting for availability...")
    # Orchestrator will queue tasks until resources available
```

---

## 📊 Example Output

```
======================================================================
  🤖 SYNAPSIS INTELLIGENT AGENT ORCHESTRATOR
======================================================================
🔍 Discovered 6 models from local sources

📊 System Status:
  RAM: 14.89/23.17 GB (64% available)
  CPU: 8.6%
  Models Available: 6
  Can Spawn More: True

🧪 Testing Model Selection:

  Task: prompt_injection
    ✅ Selected: deepseek-coder:1.3b (ollama)
    Score: 0.65 (capabilities: fast, lightweight, coding)

  Task: code_analysis
    ✅ Selected: deepseek-coder:6.7b (ollama)
    Score: 0.80 (capabilities: balanced, coding)

  Task: deep_reasoning
    ✅ Selected: deepseek-r1-i1:latest (ollama)
    Score: 0.75 (capabilities: balanced, deep_reasoning)

✅ Orchestrator ready for intelligent multi-agent execution
======================================================================
```

---

## ✅ Ready for Production

- ✅ Auto-discovers models on any system
- ✅ Resource-aware (won't crash OS)
- ✅ Intelligent task assignment
- ✅ Health monitoring
- ✅ Graceful degradation
- ✅ Works with 0 models (graceful fallback)
- ✅ Works with 10+ models (load balancing)

---

**Author:** AI Security Team  
**Date:** 2026-03-22  
**Version:** 1.0.0
