# Agent OS Integration Plan

## Overview
This document outlines the integration of OpenFang (Agent OS), rig-rlm, and Aider into the NCA project to create a comprehensive AI agent operating system with Rust-native tooling.

## Phase 1: Foundation (Current)
✅ **Skills Framework Installed**
- juliusbrussee/cavekit (5 skills): backprop, build, caveman, check, spec
- obra/superpowers (14 skills): brainstorming, dispatching-parallel-agents, executing-plans, etc.
- mattpocock/skills (14 skills): diagnose, tdd, triage, zoom-out, etc.

✅ **Multi-Provider Support**
- 23+ LLM providers configured (NVIDIA NIM, GLM, Kimi, Ollama Cloud, etc.)
- Generic provider config for any OpenAI-compatible API

## Phase 2: V1 Integration (Python-based)

### 2.1 OpenFang Agent OS Integration
**Source**: https://github.com/RightNow-AI/openfang

**Key Components to Integrate**:
- Agent orchestration framework
- Multi-agent collaboration patterns
- Tool execution engine
- Memory and context management
- Event-driven architecture

**Integration Steps**:
1. Analyze OpenFang's agent architecture (`/tmp/openfang/agents/`)
2. Extract core orchestration patterns
3. Create Rust bindings or FFI layer for Python components
4. Integrate with NCA's existing runtime

### 2.2 rig-rlm Integration
**Source**: https://github.com/joshua-mo-143/rig-rlm

**Key Components**:
- Retrieval-Augmented Language Model framework
- Vector store integration
- Context retrieval mechanisms

**Integration Steps**:
1. Review rig-rlm architecture (`/tmp/rig-rlm/src/`)
2. Implement RAG pipeline in NCA runtime
3. Add vector store support (Qdrant, Pinecone, etc.)
4. Create retrieval tools for agents

### 2.3 Aider Integration
**Source**: https://github.com/Aider-AI/aider

**Key Components**:
- Code editing and refactoring
- Git-aware code modifications
- Test-driven development workflows

**Integration Approach**:
1. Use Aider as a subprocess tool initially
2. Create wrapper commands for NCA to invoke Aider
3. Parse Aider output for agent feedback loops
4. Gradually migrate critical features to Rust

## Phase 3: V2 Rust Rewrite (Claude Code Killer)

### 3.1 Core Architecture
**Goals**:
- Native Rust implementation of Aider's core functionality
- High-performance code editing engine
- Integrated AST parsing and manipulation
- Real-time git operations
- Multi-file atomic edits

### 3.2 Key Modules to Build

#### `nca-code-edit` Crate
- Diff generation and application
- AST-aware code transformations
- Language-specific parsers (tree-sitter integration)
- Safe edit validation

#### `nca-git` Crate
- Native git2.rs bindings
- Branch management
- Commit history analysis
- Conflict resolution

#### `nca-agent-os` Crate
- Agent orchestration (inspired by OpenFang)
- Task decomposition
- Parallel agent execution
- Result aggregation

#### `nca-rag` Crate
- Embedding generation
- Vector search
- Context ranking
- Memory management

### 3.3 Performance Targets
- Edit application: <100ms for typical files
- Git operations: <50ms for status/diff
- Agent spawning: <200ms
- Context retrieval: <500ms for 10K documents

## Implementation Roadmap

### Q1 2026: V1 Complete
- [ ] OpenFang orchestration integrated
- [ ] rig-rlm RAG pipeline working
- [ ] Aider subprocess integration functional
- [ ] All 23+ providers tested with full workflow

### Q2 2026: V2 Foundation
- [ ] `nca-code-edit` crate MVP
- [ ] Basic AST parsing for Rust/Python/TS
- [ ] Git operations native in Rust
- [ ] Performance benchmarks established

### Q3 2026: V2 Agent OS
- [ ] Multi-agent orchestration in Rust
- [ ] Parallel execution engine
- [ ] Advanced RAG with hybrid search
- [ ] Memory persistence layer

### Q4 2026: Claude Code Killer
- [ ] Feature parity with Aider
- [ ] 10x performance improvement
- [ ] Superior DX with TUI
- [ ] Enterprise-ready security model

## Skills Utilization

The installed skills will drive development:

**From cavekit**:
- `spec`: Define precise specifications for each module
- `build`: Automated build and CI/CD pipelines
- `check`: Quality gates and testing
- `backprop`: Bug-to-spec feedback loops

**From superpowers**:
- `dispatching-parallel-agents`: Multi-agent development
- `subagent-driven-development`: Decompose complex tasks
- `test-driven-development`: TDD for all Rust crates
- `systematic-debugging`: Performance optimization

**From mattpocock**:
- `diagnose`: Architecture review and bottleneck identification
- `tdd`: Type-driven development in Rust
- `zoom-out`: Strategic planning and prioritization
- `triage`: Issue management and prioritization

## Next Steps

1. **Immediate**: Review OpenFang agent architectures
2. **Week 1**: Design RAG integration with rig-rlm patterns
3. **Week 2**: Create Aider wrapper tool for NCA
4. **Week 3-4**: Begin Rust rewrite planning for code-edit crate
5. **Month 2**: First V2 alpha with native code editing

## Success Metrics

- **V1**: Full workflow with 3rd party tools
- **V2 Alpha**: 50% of Aider features in Rust
- **V2 Beta**: Performance targets met
- **V2 GA**: Production-ready Claude Code alternative
