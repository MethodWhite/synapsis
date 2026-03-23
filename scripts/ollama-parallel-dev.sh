#!/bin/bash
# Ollama Parallel Development Orchestrator
# Uses local Ollama models for parallel task execution

set -e

echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Ollama Parallel Development Orchestrator                ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

# Available models
MODELS=(
    "huihui-qwen-9b:latest"    # Documentation, configs
    "deepseek-r1-i1:latest"    # Analysis, planning
    "deepseek-coder:6.7b"      # Code implementation
)

# Task assignment
assign_task() {
    local model=$1
    local task=$2
    local output=$3
    
    echo "[🤖 $model] Starting: $task"
    ollama run "$model" "$task" > "$output" 2>&1 &
    echo $!
}

echo "📋 Assigning parallel tasks to Ollama sub-agents..."
echo ""

# Parallel task execution
echo "[1/3] Documentation generation (huihui-qwen-9b)..."
assign_task "huihui-qwen-9b:latest" "Generate comprehensive README for MW-CLI auto-update system" "/tmp/mw-readme.out" &

echo "[2/3] Code analysis (deepseek-r1-i1)..."
assign_task "deepseek-r1-i1:latest" "Analyze updater.rs for potential improvements and edge cases" "/tmp/updater-analysis.out" &

echo "[3/3] Implementation review (deepseek-coder)..."
assign_task "deepseek-coder:6.7b" "Review install scripts for cross-platform compatibility issues" "/tmp/install-review.out" &

# Wait for all tasks
echo ""
echo "⏳ Waiting for sub-agents to complete..."
wait

echo ""
echo "✅ All sub-agents completed!"
echo ""
echo "Results:"
ls -lah /tmp/*.out
