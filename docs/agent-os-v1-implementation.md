# Agent OS Integration - V1 Implementation

## Overview
This document provides the detailed implementation plan for integrating OpenFang, rig-rlm, and Aider into NCA as a unified Agent Operating System.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     NCA CLI (TUI)                           │
├─────────────────────────────────────────────────────────────┤
│                   Agent Orchestrator                        │
│  (OpenFang-inspired multi-agent coordination in Rust)       │
├──────────────┬────────────────┬─────────────────────────────┤
│  Code Edit   │     RAG        │      Tool Execution         │
│  Engine      │   Pipeline     │         Layer               │
│  (V2: Rust)  │ (rig-rlm based)│  (Aider subprocess → Rust)  │
├──────────────┴────────────────┴─────────────────────────────┤
│                  Provider Abstraction Layer                 │
│           (23+ LLM providers with generic config)           │
└─────────────────────────────────────────────────────────────┘
```

## New Crates to Add

### 1. `nca-agent-os` Crate
**Purpose**: Multi-agent orchestration inspired by OpenFang's "Hands" concept

**Structure**:
```
crates/agent-os/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── agent.rs          # Agent definition and lifecycle
│   ├── orchestrator.rs   # Multi-agent coordination
│   ├── task.rs           # Task decomposition and assignment
│   ├── memory.rs         # Short-term and long-term memory
│   ├── tool.rs           # Tool registration and execution
│   └── events.rs         # Event-driven communication
```

**Key Features**:
- Agent lifecycle management (spawn, pause, resume, terminate)
- Task queue with priority scheduling
- Inter-agent messaging via channels
- Shared memory space for context sharing
- Event bus for decoupled communication

**OpenFang Inspiration**:
- "Hands" concept → Pre-configured agent profiles
- Autonomous operation → Scheduled tasks without prompts
- Guardrails → Approval gates for sensitive operations

### 2. `nca-rag` Crate
**Purpose**: Retrieval-Augmented Generation pipeline based on rig-rlm

**Structure**:
```
crates/rag/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── retriever.rs      # Document retrieval strategies
│   ├── indexer.rs        # Document indexing and embedding
│   ├── store.rs          # Vector store abstraction
│   ├── reranker.rs       # Context re-ranking
│   └── context.rs        # Context assembly for LLMs
```

**Key Features**:
- Multiple vector store backends (Qdrant, Pinecone, Chroma, in-memory)
- Hybrid search (semantic + keyword)
- Document chunking with overlap
- Embedding generation (multiple providers)
- Context window optimization

**rig-rlm Inspiration**:
- REPL-based execution boundary
- Recursive LLM calls (agents can spawn sub-agents)
- Sandbox isolation for tool execution

### 3. `nca-code-edit` Crate (V2)
**Purpose**: Native Rust code editing engine (Aider replacement)

**Structure**:
```
crates/code-edit/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── diff.rs           # Diff generation and application
│   ├── ast.rs            # AST parsing and manipulation
│   ├── edit.rs           # Atomic edit operations
│   ├── validator.rs      # Edit safety validation
│   ├── language.rs       # Language-specific handlers
│   └── git.rs            # Git-aware operations
```

**Dependencies**:
- `tree-sitter` + language grammars for AST parsing
- `git2` for native git operations
- `similar` for diff algorithms

**Key Features**:
- Multi-file atomic edits
- AST-aware transformations
- Syntax validation before application
- Rollback capability
- Git conflict detection

### 4. `nca-tools` Crate
**Purpose**: Tool registry and execution layer

**Structure**:
```
crates/tools/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── registry.rs       # Tool registration and discovery
│   ├── executor.rs       # Tool execution with sandboxing
│   ├── builtin.rs        # Built-in tools (file, shell, git, etc.)
│   ├── aider.rs          # Aider integration (V1)
│   └── skill.rs          # Skills.sh integration
```

**V1 Approach**:
- Aider as subprocess with structured output parsing
- Skills.sh integration via `.agents/skills/` directory
- Command execution with timeout and resource limits

**V2 Approach**:
- Native Rust implementations of all tools
- Direct AST manipulation instead of subprocess calls
- Parallel tool execution with dependency resolution

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)
**Goal**: Basic scaffolding and provider integration

1. **Create crate structure**
   ```bash
   mkdir -p crates/{agent-os,rag,tools}
   # Add Cargo.toml for each
   # Update workspace Cargo.toml
   ```

2. **Implement provider abstraction**
   - Extend existing `PartialProviderConfig` to support new crates
   - Add trait for LLM client that all providers implement
   - Create factory pattern for provider instantiation

3. **Basic agent skeleton**
   - Define `Agent` struct with lifecycle methods
   - Implement simple task queue
   - Add logging and tracing

### Phase 2: RAG Pipeline (Weeks 3-4)
**Goal**: Working retrieval system

1. **Vector store abstraction**
   - Define `VectorStore` trait
   - Implement in-memory store (for testing)
   - Add Qdrant client (most popular open-source)

2. **Document processing**
   - Text chunking with configurable size/overlap
   - Embedding generation via providers
   - Metadata extraction and filtering

3. **Retrieval strategies**
   - Semantic search (cosine similarity)
   - Keyword search (BM25)
   - Hybrid fusion (RRF - Reciprocal Rank Fusion)

4. **Integration with agents**
   - Agents can query RAG for context
   - Automatic context injection into prompts
   - Citation tracking

### Phase 3: Aider Integration (Weeks 5-6)
**Goal**: Working code editing via Aider subprocess

1. **Subprocess wrapper**
   - Spawn Aider with controlled arguments
   - Capture stdout/stderr with structured parsing
   - Handle timeouts and errors gracefully

2. **Git integration**
   - Detect git repos automatically
   - Create branches for agent work
   - Commit messages from agent reasoning

3. **Feedback loop**
   - Parse Aider's edit summaries
   - Validate changes with tests (if available)
   - Rollback on failure

### Phase 4: Agent Orchestration (Weeks 7-8)
**Goal**: Multi-agent collaboration

1. **Task decomposition**
   - Break complex tasks into subtasks
   - Assign to specialized agents
   - Track dependencies

2. **Inter-agent communication**
   - Message passing via channels
   - Shared blackboard for results
   - Conflict resolution

3. **Scheduling**
   - Priority queues
   - Resource allocation
   - Parallel execution where safe

### Phase 5: V2 Rust Rewrite (Months 3-6)
**Goal**: Native Rust code editing engine

1. **AST parsing infrastructure**
   - tree-sitter integration
   - Language detection
   - Syntax tree navigation

2. **Edit operations**
   - Find/replace with context
   - Insert at location (line, function, class)
   - Delete with scope awareness
   - Move/refactor operations

3. **Validation**
   - Syntax checking after edits
   - Type checking (where possible)
   - Test execution for regression detection

4. **Performance optimization**
   - Incremental parsing
   - Caching strategies
   - Parallel processing

## Skills Integration

The installed skills will guide development:

### Daily Workflow with Skills

**Morning Standup** (automated via skills):
```bash
# Using caveman skill for quick status
nca --skill caveman --prompt "What was accomplished yesterday?"

# Using triage skill for prioritization
nca --skill triage --prompt "Prioritize today's tasks based on agent-os-integration.md"
```

**Development Sessions**:
```bash
# Using spec skill before implementing features
nca --skill spec --prompt "Define the VectorStore trait for nca-rag crate"

# Using tdd skill for implementation
nca --skill tdd --prompt "Implement in-memory vector store with tests"

# Using check skill before commits
nca --skill check --prompt "Review changes in crates/rag/ for quality"
```

**Code Review**:
```bash
# Using request-code-review skill
nca --skill request-code-review --prompt "Review nca-agent-os orchestrator implementation"

# Using systematic-debugging skill for issues
nca --skill systematic-debugging --prompt "Debug race condition in agent message passing"
```

**End of Day**:
```bash
# Using finishing-a-development-branch skill
nca --skill finishing-a-development-branch --prompt "Complete the RAG integration branch"

# Using handoff skill for async collaboration
nca --skill handoff --prompt "Document progress for next developer"
```

## Configuration

Add to `~/.nca/config.toml`:

```toml
[agent-os]
enabled = true
max_concurrent_agents = 5
task_timeout_seconds = 300
approval_required_for = ["file_write", "shell_command", "git_push"]

[rag]
enabled = true
default_store = "qdrant"
chunk_size = 512
chunk_overlap = 50
top_k_results = 5
hybrid_search = true

[aider]
enabled = true
binary_path = "aider"  # Or full path
auto_git_branch = true
test_before_commit = true

[providers]
default = "ollamacloud"
fallback = ["openai", "anthropic"]

[[agents]]
name = "researcher"
provider = "perplexity"
tools = ["web_search", "rag_query", "file_read"]
schedule = "0 6 * * *"  # Daily at 6 AM

[[agents]]
name = "coder"
provider = "claude"
tools = ["code_edit", "git", "test_runner"]
guardrails = ["require_tests", "no_prod_writes"]
```

## Testing Strategy

### Unit Tests
- Each crate has comprehensive unit tests
- Mock providers for deterministic testing
- Property-based testing for edit operations

### Integration Tests
- End-to-end agent workflows
- Multi-agent collaboration scenarios
- RAG pipeline with real documents

### Performance Tests
- Benchmark edit application speed
- Measure RAG retrieval latency
- Stress test concurrent agents

## Migration Path

### From V1 to V2

1. **Feature Flag Approach**
   ```rust
   #[cfg(feature = "v2-code-edit")]
   use nca_code_edit::CodeEditor;
   
   #[cfg(not(feature = "v2-code-edit"))]
   use nca_tools::aider::AiderWrapper as CodeEditor;
   ```

2. **Gradual Rollout**
   - Start with non-critical operations
   - Compare results between V1 and V2
   - Enable V2 for beta users first

3. **Rollback Plan**
   - Keep Aider binary as fallback
   - Feature flag to disable V2 instantly
   - Automated health checks

## Success Criteria

### V1 Complete When:
- [ ] All 23+ providers work with agent orchestration
- [ ] RAG pipeline retrieves relevant context
- [ ] Aider integration edits code successfully
- [ ] Multi-agent tasks complete end-to-end
- [ ] Skills framework integrated and documented

### V2 Complete When:
- [ ] Native code editing matches Aider capabilities
- [ ] 10x performance improvement on benchmarks
- [ ] Zero external Python dependencies for core features
- [ ] Production-ready with enterprise security
- [ ] Comprehensive documentation and examples

## Next Immediate Actions

1. **Today**: 
   - [ ] Create crate directories and Cargo.toml files
   - [ ] Set up basic module structure
   - [ ] Add workspace dependencies

2. **This Week**:
   - [ ] Implement `Agent` struct and lifecycle
   - [ ] Create `VectorStore` trait
   - [ ] Build Aider subprocess wrapper

3. **Next Week**:
   - [ ] Integrate with existing NCA CLI
   - [ ] Add configuration parsing
   - [ ] Write initial tests

---

*This document is a living specification. Update as implementation progresses.*
