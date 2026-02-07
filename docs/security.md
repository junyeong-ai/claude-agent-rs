# Security System

Comprehensive sandboxing with TOCTOU-safe operations and process isolation.

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Security Context                          │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │
│  │   SecureFs    │  │ BashAnalyzer  │  │ ResourceLimits│    │
│  │               │  │               │  │               │    │
│  │ - openat()    │  │ - AST parsing │  │ - setrlimit() │    │
│  │ - O_NOFOLLOW  │  │ - tree-sitter │  │ - CPU time    │    │
│  │ - Symlink     │  │ - Dangerous   │  │ - Memory      │    │
│  │   depth limit │  │   detection   │  │ - File count  │    │
│  └───────────────┘  └───────────────┘  └───────────────┘    │
│                                                              │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐    │
│  │ NetworkSandbox│  │ SecurityPolicy│  │  OS Sandbox   │    │
│  │               │  │               │  │               │    │
│  │ - Domain      │  │ - Permission  │  │ - Landlock    │    │
│  │   filtering   │  │   rules       │  │   (Linux)     │    │
│  │ - URL check   │  │ - Mode        │  │ - Seatbelt    │    │
│  └───────────────┘  └───────────────┘  │   (macOS)     │    │
│                                         └───────────────┘    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## OS-Level Sandbox

Process isolation using OS-specific sandboxing:

| Platform | Technology | Description |
|----------|------------|-------------|
| Linux | Landlock LSM | Kernel 5.13+, filesystem access control |
| macOS | Seatbelt | sandbox-exec with SBPL profiles |

For detailed configuration, see [Sandbox Guide](sandbox.md).

## TOCTOU-Safe File Operations

All file operations use `openat()` with `O_NOFOLLOW` to prevent time-of-check-to-time-of-use vulnerabilities.

### Problem: Traditional Approach

```
1. Check: path exists and is safe
2. [ATTACKER: replaces path with symlink]
3. Open: follows symlink to /etc/passwd
```

### Solution: openat() with O_NOFOLLOW

```rust
use claude_agent::security::SecureFs;

let fs = SecureFs::new(
    root_path,
    allowed_paths,
    denied_patterns,
    max_symlink_depth,  // Default: 10
)?;

// Safe file operations
let handle = fs.open(path)?;
let content = handle.read_to_string()?;
```

### How It Works

```
┌─────────────────────────────────────────┐
│            Path Resolution              │
├─────────────────────────────────────────┤
│                                         │
│  /project/src/main.rs                   │
│       │                                 │
│       ▼                                 │
│  open("/project", O_DIRECTORY)          │
│       │                                 │
│       ▼                                 │
│  openat(fd, "src", O_NOFOLLOW)          │
│       │                                 │
│       ├── Is symlink? → Check depth     │
│       │   └── Depth > 10? → Reject      │
│       │                                 │
│       ▼                                 │
│  openat(fd, "main.rs", O_NOFOLLOW)      │
│       │                                 │
│       ▼                                 │
│  Final file descriptor (atomic)         │
│                                         │
└─────────────────────────────────────────┘
```

## Bash Command Analysis

Commands are analyzed via AST (tree-sitter) before execution.

### Dangerous Pattern Detection

```rust
use claude_agent::security::bash::{BashAnalyzer, BashPolicy, SecurityConcern};

let policy = BashPolicy::default();
let analyzer = BashAnalyzer::new(policy);

let analysis = analyzer.analyze("rm -rf /");
// analysis.concerns contains SecurityConcern::DangerousCommand("rm root")

let analysis = analyzer.analyze("git status");
// analysis.concerns.is_empty() == true (safe command)
```

### Detected Patterns

| Category | Examples | Level |
|----------|----------|-------|
| Destructive | `rm -rf /`, `mkfs`, `dd if=/dev/zero` | Critical |
| Privilege | `sudo`, `su`, `chmod 777` | High |
| Network | `curl \| bash`, `wget -O- \| sh` | High |
| File system | `mv / /tmp`, `ln -sf` | Medium |
| Safe | `git status`, `cargo build`, `ls` | Safe |

### Custom Policies

`BashPolicy` provides constructor methods and builder-style configuration:

```rust
// Default policy (blocks common network commands, allows substitutions)
let policy = BashPolicy::default();

// Strict policy (blocks all substitutions, remote exec, privilege escalation)
let policy = BashPolicy::strict();

// Permissive policy (allows everything)
let policy = BashPolicy::permissive();

// Customize blocked commands via builder method
let policy = BashPolicy::strict()
    .blocked_commands(["curl", "wget", "rm", "sudo"]);
```

## Environment Sanitization

Environment variables are filtered before command execution.

```rust
// Dangerous variables removed:
// - LD_PRELOAD
// - LD_LIBRARY_PATH
// - DYLD_* (macOS)
// - PATH manipulation

// Safe variables preserved:
// - HOME
// - USER
// - LANG
// - Project-specific vars
```

## Resource Limits

Process isolation via `setrlimit()`.

```rust
use claude_agent::security::ResourceLimits;

let limits = ResourceLimits::default()
    .cpu_time(60)                    // 60 seconds
    .virtual_memory(512 * 1024 * 1024)  // 512 MB
    .open_files(100)                 // 100 open files
    .processes(10);                  // 10 subprocesses
```

### Limit Types

| Limit | Method | Description | Default |
|-------|--------|-------------|---------|
| CPU time | `cpu_time()` | Maximum CPU seconds | 300s (5 min) |
| Virtual memory | `virtual_memory()` | Maximum virtual memory | 2GB |
| Open files | `open_files()` | Open file descriptors | 256 |
| Processes | `processes()` | Child processes | 32 |
| File size | `file_size()` | Maximum file size | 100MB |

## Network Sandbox

Domain filtering for web operations.

```rust
use claude_agent::security::sandbox::NetworkSandbox;

let sandbox = NetworkSandbox::new()
    .allowed_domains(vec!["github.com".into(), "*.githubusercontent.com".into()])
    .blocked_domains(vec!["*.internal".into()]);

// Or permissive (all domains allowed)
let sandbox = NetworkSandbox::permissive();
```

### Domain Checking

```rust
use claude_agent::security::sandbox::DomainCheck;

let check = sandbox.check("github.com");
// DomainCheck::Allowed

let check = sandbox.check("unknown.com");
// DomainCheck::Blocked (not in allowed list)
```

## SecurityContext Builder

```rust
use claude_agent::security::SecurityContext;

let ctx = SecurityContext::builder()
    .root("/project")
    .allowed_paths(vec!["/project/src".into()])
    .denied_patterns(vec!["*.env".into(), "*.key".into()])
    .max_symlink_depth(5)
    .limits(ResourceLimits::default())
    .bash_policy(BashPolicy::strict())
    .network(NetworkSandbox::new()
        .allowed_domains(vec!["github.com".into()])
        .blocked_domains(vec!["*.internal".into()]))
    .build()?;
```

## Security Guard

Validates tool inputs before execution.

```rust
use claude_agent::security::SecurityGuard;

// Automatic validation in ToolRegistry
if let Err(e) = SecurityGuard::validate(&security_ctx, "Bash", &input) {
    return ToolOutput::error(format!("Security: {}", e));
}
```

### Validation Checks

| Tool | Checks |
|------|--------|
| Read, Write, Edit | Path in allowed list, not in denied patterns |
| Bash | Command analysis, environment sanitization |
| WebFetch | URL domain allowed |
| Glob, Grep | Search path restrictions |

## Permissive Mode

For development/testing:

```rust
let ctx = SecurityContext::permissive();
// All operations allowed
// Root: /
// No limits
```

## Best Practices

1. **Always specify root**: Constrain file operations to project
2. **Use strict bash policies**: Allow only necessary commands
3. **Set resource limits**: Prevent runaway processes
4. **Filter network access**: Whitelist required domains
5. **Limit symlink depth**: Prevent traversal attacks
6. **Deny sensitive patterns**: `*.env`, `*.key`, `.git/config`
