#!/bin/bash
# Synapsis Ollama Sub-Agents - Parallel Task Execution
# Usage: ./ollama-subagents.sh <task_type>

set -e

TASK_TYPE=${1:-"general"}

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Synapsis Ollama Sub-Agents - Parallel Execution         ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# Available models
MODELS=(
    "huihui-qwen-9b:latest"    # General tasks, documentation
    "deepseek-r1-i1:latest"    # Reasoning, analysis
    "deepseek-coder:6.7b"      # Code implementation
)

# Function to run task with specific model
run_subagent() {
    local model=$1
    local task=$2
    local output_file=$3
    
    echo "[Sub-Agent] Starting $model with task: $task"
    
    # Run in background for parallel execution
    ollama run "$model" "$task" > "$output_file" 2>&1 &
    echo $!
}

# Parallel task execution
case $TASK_TYPE in
    "documentation")
        echo "📝 Documentation Tasks (Parallel):"
        run_subagent "huihui-qwen-9b:latest" "Generate API documentation for Synapsis MCP tools" "/tmp/doc-agent-1.out"
        run_subagent "deepseek-coder:6.7b" "Generate code examples for Synapsis usage" "/tmp/doc-agent-2.out"
        wait
        echo "✅ Documentation tasks completed"
        ;;
    
    "security")
        echo "🔒 Security Tasks (Parallel):"
        run_subagent "deepseek-r1-i1:latest" "Analyze security vulnerabilities in Synapsis" "/tmp/sec-agent-1.out"
        run_subagent "huihui-qwen-9b:latest" "Generate security best practices document" "/tmp/sec-agent-2.out"
        wait
        echo "✅ Security tasks completed"
        ;;
    
    "code")
        echo "💻 Code Tasks (Parallel):"
        run_subagent "deepseek-coder:6.7b" "Implement missing Synapsis features" "/tmp/code-agent-1.out"
        run_subagent "deepseek-coder:1.3b" "Write unit tests for Synapsis modules" "/tmp/code-agent-2.out"
        wait
        echo "✅ Code tasks completed"
        ;;
    
    *)
        echo "🔄 General Tasks (Parallel):"
        for model in "${MODELS[@]}"; do
            run_subagent "$model" "Help with Synapsis development tasks" "/tmp/general-${model//[:.]/-}.out"
        done
        wait
        echo "✅ General tasks completed"
        ;;
esac

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  All Sub-Agents Completed                                ║"
echo "╚══════════════════════════════════════════════════════════╝"
