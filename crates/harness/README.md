# NCA Harness Engineering

Ultimate harness engineering capabilities for self-healing, multi-agent development.

## Overview

The `nca-harness` crate provides:

- **Self-Healing Execution Loop**: Automatically detects compilation errors, analyzes them via LLM, generates patches, applies them, and retries until success
- **Multi-Agent Swarm**: Orchestrates specialized agents (Architect, Coder, Reviewer, Tester, Debugger) working in parallel
- **Sandboxed Execution**: Safe code execution with timeout and resource limits
- **Dynamic Tool Synthesis**: Agents can define new tools on the fly based on context needs
- **Context-Aware RAG**: Pulls relevant documentation/code snippets dynamically during repair

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Harness Engine                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Swarm     │  │  Sandbox    │  │   Synthesizer       │  │
│  │  (Agents)   │  │ (Execution) │  │ (Tools/Tests/Docs)  │  │
│  └─────────────┘  └─────────────┘  └─────────────────────┘  │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                   State Manager                       │   │
│  │  Tasks | Errors | Patches | History | Metrics         │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Components

### Engine (`engine.rs`)
Core self-healing loop that:
1. Executes tasks with the agent swarm
2. Compiles and tests code in sandbox
3. Analyzes errors automatically
4. Generates and applies fixes
5. Retries until success or max attempts

### Swarm (`swarm.rs`)
Multi-agent coordination with specialized roles:
- **Architect**: Plans system structure and breaks down tasks
- **Coder**: Implements clean, efficient code
- **Reviewer**: Reviews for quality, security, performance
- **Tester**: Writes comprehensive tests
- **Debugger**: Analyzes errors and provides fixes

### Sandbox (`sandbox.rs`)
Safe execution environment with:
- Configurable timeouts
- Memory limits (via rlimit on Unix)
- Compilation checking
- Test execution
- Command execution isolation

### Synthesizer (`synthesizer.rs`)
Dynamic capability expansion:
- Generate new tools from descriptions
- Auto-generate tests for code
- Create documentation
- Context-aware synthesis

### State (`state.rs`)
Global state management:
- Task tracking with status lifecycle
- Error collection and clearing
- Patch history
- Agent conversation history
- Thread-safe with DashMap and RwLock

### Tools (`tools.rs`)
Built-in and dynamic tools:
- File read/write
- Shell execution
- Directory listing
- Custom tool registration

## Usage

### Basic Example

```rust
use nca_harness::{HarnessEngine, AgentSwarm, Sandbox};

#[tokio::main]
async fn main() {
    let engine = HarnessEngine::new()
        .with_timeout(std::time::Duration::from_secs(600))
        .with_max_retries(10);
    
    let result = engine.execute_with_healing("Add async retry logic to the HTTP client").await;
    
    if result.success {
        println!("✅ Task completed in {} attempts", result.attempts);
    } else {
        println!("❌ Failed after {} attempts: {:?}", 
                 result.attempts, result.final_error);
    }
    
    println!("{}", engine.get_summary());
}
```

### Using Just Commands

```bash
# Run full self-healing harness
just harness

# Run multi-agent swarm demo
just swarm

# Auto-repair a specific file
just heal src/main.rs

# Run continuous improvement mode
just evolve

# Run harness tests
just test-harness
```

## Self-Healing Flow

```
1. Task Input
   ↓
2. Architect plans approach
   ↓
3. Coder implements
   ↓
4. Reviewer checks quality
   ↓
5. Tester writes tests
   ↓
6. Sandbox compiles ──fail──→ Debugger analyzes
   │                           ↓
   │                      Generate fix
   │                           ↓
   │                      Apply patch
   │                           ↓
   └────success───────────────┘
   ↓
7. Run tests ──fail──→ (loop back to 6)
   │
   success
   ↓
8. Complete ✅
```

## Configuration

Environment variables:
- `NCA_HARNESS_TIMEOUT`: Default timeout in seconds (default: 300)
- `NCA_HARNESS_MAX_RETRIES`: Maximum retry attempts (default: 5)
- `NCA_HARNESS_WORKDIR`: Working directory for sandbox

## Integration

Works seamlessly with other NCA components:
- **agent_os**: Agent definitions and orchestration
- **rig_core**: RAG pipeline for context retrieval
- **aider_rs**: Diff-based code editing
- **nca-common**: Shared configuration and types
- **nca-llm**: Multi-provider LLM support

## Testing

```bash
# Run all harness tests
cargo test -p nca-harness

# Run with output
cargo test -p nca-harness -- --nocapture

# Run specific test
cargo test -p nca-harness test_engine_creation
```

## Future Enhancements

- [ ] Distributed swarm across multiple machines
- [ ] GPU-accelerated code analysis
- [ ] Real-time collaboration between human and agents
- [ ] Automatic benchmark generation
- [ ] Security vulnerability detection and fixing
- [ ] Performance optimization suggestions
- [ ] Documentation generation and maintenance
- [ ] Dependency update automation

## License

MIT
