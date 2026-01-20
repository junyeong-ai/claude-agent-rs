# Sandbox System

OS-level process isolation for secure command execution.

> For broader security features (TOCTOU-safe file operations, bash analysis, resource limits), see [Security Guide](security.md).

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Sandbox Architecture                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              SandboxRuntime (trait)                  │    │
│  │                                                      │    │
│  │  is_available() → bool                               │    │
│  │  apply() → Result<()>                                │    │
│  │  wrap_command(cmd) → Result<String>                  │    │
│  │  environment_vars() → HashMap                        │    │
│  └────────────────────┬─────────────────────────────────┘    │
│                       │                                      │
│          ┌────────────┴────────────┐                        │
│          ▼                         ▼                        │
│  ┌───────────────┐        ┌───────────────┐                 │
│  │ LandlockSandbox│        │SeatbeltSandbox│                 │
│  │   (Linux)     │        │   (macOS)     │                 │
│  │               │        │               │                 │
│  │ Kernel 5.13+  │        │ sandbox-exec  │                 │
│  │ LSM-based     │        │ Profile-based │                 │
│  └───────────────┘        └───────────────┘                 │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

## Platform Support

| Platform | Technology | Kernel/OS Version |
|----------|------------|-------------------|
| Linux | Landlock LSM | Kernel 5.13+ |
| macOS | Seatbelt (sandbox-exec) | All versions |
| Windows | Not supported | - |

## Quick Start

```rust
use claude_agent::security::sandbox::{Sandbox, SandboxConfig, create_sandbox};

// Simple creation
let sandbox = create_sandbox(&working_dir, true);

// With configuration
let config = SandboxConfig::new(working_dir.to_path_buf())
    .with_allowed_paths(vec!["/usr/local/bin".into()])
    .with_excluded_commands(vec!["docker".into()]);

let sandbox = Sandbox::new(config);

// Check availability
if sandbox.is_available() {
    let wrapped = sandbox.wrap_command("cargo build")?;
    // Execute wrapped command
}
```

## SandboxConfig

```rust
pub struct SandboxConfig {
    pub enabled: bool,                      // Enable sandbox
    pub auto_allow_bash_if_sandboxed: bool, // Auto-approve bash in sandbox
    pub excluded_commands: HashSet<String>, // Commands to bypass
    pub allow_unsandboxed_commands: bool,   // Allow bypass for excluded
    pub enable_weaker_nested_sandbox: bool, // Nested container support
    pub working_dir: PathBuf,               // Full access directory
    pub allowed_paths: Vec<PathBuf>,        // Read-only allowed paths
    pub denied_paths: Vec<String>,          // Blocked path patterns
    pub network: NetworkConfig,             // Network restrictions
}
```

### Configuration Options

| Option | Default | Description |
|--------|---------|-------------|
| `enabled` | `false` | Enable sandboxing |
| `auto_allow_bash_if_sandboxed` | `true` | Skip bash permission prompts when sandboxed |
| `excluded_commands` | `[]` | Commands that bypass sandbox |
| `allow_unsandboxed_commands` | `true` | Allow excluded commands to run unsandboxed |
| `enable_weaker_nested_sandbox` | `false` | Use weaker sandbox for containers |

### Builder Pattern

```rust
let config = SandboxConfig::new(PathBuf::from("/project"))
    .with_auto_allow_bash(true)
    .with_allowed_paths(vec![
        PathBuf::from("/usr/local"),
        PathBuf::from("/opt/tools"),
    ])
    .with_denied_paths(vec![
        "*.env".into(),
        "secrets/*".into(),
    ])
    .with_excluded_commands(vec![
        "docker".into(),
        "podman".into(),
    ])
    .with_network(NetworkConfig::with_proxy(Some(8080), None));
```

## NetworkConfig

```rust
pub struct NetworkConfig {
    pub allow_unix_sockets: Vec<String>,  // Allowed Unix sockets
    pub allow_local_binding: bool,        // Allow binding to localhost
    pub http_proxy_port: Option<u16>,     // HTTP proxy port
    pub socks_proxy_port: Option<u16>,    // SOCKS proxy port
}
```

### Proxy Configuration

```rust
// Force network through proxy
let network = NetworkConfig::with_proxy(Some(8080), Some(1080));

// Environment variables set automatically:
// HTTP_PROXY=http://127.0.0.1:8080
// HTTPS_PROXY=http://127.0.0.1:8080
// ALL_PROXY=socks5://127.0.0.1:1080
// NO_PROXY=localhost,127.0.0.1,::1

let config = SandboxConfig::new(working_dir)
    .with_network(network);
```

## Linux Landlock

Landlock is a Linux Security Module (LSM) available since kernel 5.13.

### How It Works

```
┌─────────────────────────────────────────┐
│          Landlock Sandbox               │
├─────────────────────────────────────────┤
│                                         │
│  1. Create ruleset                      │
│     Ruleset::default()                  │
│          │                              │
│          ▼                              │
│  2. Add filesystem rules                │
│     - Working dir: full access          │
│     - Allowed paths: read-only          │
│     - System paths: read-only           │
│          │                              │
│          ▼                              │
│  3. restrict_self()                     │
│     Process permanently sandboxed       │
│                                         │
└─────────────────────────────────────────┘
```

### Landlock ABI Versions

| ABI | Kernel | Features |
|-----|--------|----------|
| V1 | 5.13+ | Basic filesystem access |
| V2 | 5.19+ | File truncation |
| V3 | 6.2+ | File ioctl |
| V4 | 6.4+ | Network rules |

Best available ABI is auto-detected.

### Default Allowed Paths

```rust
// Read-only system paths
"/usr", "/lib", "/lib64", "/lib32",
"/bin", "/sbin", "/etc",
"/proc", "/sys", "/dev"

// Read-write temp paths
"/tmp", "/var/tmp"
```

## macOS Seatbelt

Seatbelt uses Apple's sandbox-exec with SBPL profiles.

### How It Works

```
┌─────────────────────────────────────────┐
│         Seatbelt Sandbox                │
├─────────────────────────────────────────┤
│                                         │
│  1. Generate SBPL profile               │
│     (version 1)                         │
│     (deny default)                      │
│     (allow file-read* ...)              │
│          │                              │
│          ▼                              │
│  2. Write profile to temp file          │
│     /tmp/claude-sandbox-{pid}.sb        │
│          │                              │
│          ▼                              │
│  3. Wrap command                        │
│     sandbox-exec -f profile.sb          │
│       bash -c 'original command'        │
│                                         │
└─────────────────────────────────────────┘
```

### Default Profile Rules

```lisp
;; System paths (read-only)
(allow file-read* (subpath "/usr"))
(allow file-read* (subpath "/bin"))
(allow file-read* (subpath "/Library"))
(allow file-read* (subpath "/System"))

;; Home directory tools (read-only)
(allow file-read* (subpath "~/.cargo"))
(allow file-read* (subpath "~/.rustup"))
(allow file-read* (subpath "~/.npm"))

;; Working directory (full access)
(allow file-read* file-write* file-ioctl (subpath "{working_dir}"))

;; Process execution
(allow process-exec (subpath "/bin"))
(allow process-exec (subpath "/usr/bin"))
(allow process-fork)

;; Network (configurable)
(allow network-outbound)
(allow network-outbound (remote udp "*:53"))  ;; DNS
```

## Command Wrapping

### Excluded Commands

Some commands need to bypass the sandbox:

```rust
let config = SandboxConfig::new(working_dir)
    .with_excluded_commands(vec![
        "docker".into(),    // Needs its own sandboxing
        "podman".into(),
        "kubectl".into(),
    ]);

// If command is excluded and allow_unsandboxed_commands is true:
sandbox.wrap_command("docker run nginx");
// Returns: "docker run nginx" (unwrapped)

// If allow_unsandboxed_commands is false:
// Returns: Error
```

### Auto-Allow Bash

When sandboxing is enabled, bash commands can be auto-approved:

```rust
let config = SandboxConfig::new(working_dir)
    .with_auto_allow_bash(true);

let sandbox = Sandbox::new(config);

if sandbox.should_auto_allow_bash() {
    // Skip permission prompt for bash commands
}
```

## Integration with Agent

```rust
use claude_agent::{Agent, security::sandbox::SandboxConfig};

let sandbox_config = SandboxConfig::new(project_dir.to_path_buf())
    .with_auto_allow_bash(true)
    .with_allowed_paths(vec![deps_dir]);

let agent = Agent::builder()
    .from_claude_code(".").await?
    .sandbox(sandbox_config)
    .build()
    .await?;
```

## Checking Sandbox Support

```rust
use claude_agent::security::sandbox::is_sandbox_supported;

if is_sandbox_supported() {
    println!("Sandbox available on this platform");
} else {
    println!("Sandbox not available");
}
```

## Bypass Options

```rust
// Explicit bypass (dangerouslyDisableSandbox in Bash tool)
let sandbox = Sandbox::new(config);

if sandbox.can_bypass(explicitly_requested) {
    // Run without sandbox
}
```

## Settings File

In `.claude/settings.json`:

```json
{
  "sandbox": {
    "enabled": true,
    "autoAllowBashIfSandboxed": true,
    "excludedCommands": ["docker", "kubectl"],
    "allowUnsandboxedCommands": true,
    "network": {
      "allowUnixSockets": ["~/.ssh/agent"],
      "httpProxyPort": 8080,
      "socksProxyPort": 1080
    }
  }
}
```

## Best Practices

1. **Enable in production**: Always enable sandboxing for untrusted inputs
2. **Minimal allowed paths**: Only add paths that are necessary
3. **Exclude container tools**: Docker/Podman have their own isolation
4. **Use proxy for network**: Control outbound network access
5. **Auto-allow bash**: Enable for better UX when sandboxed
6. **Test on target OS**: Sandbox behavior differs between Linux/macOS

## Troubleshooting

### Landlock Not Available

```bash
# Check kernel version
uname -r  # Needs 5.13+

# Check if Landlock is enabled
cat /sys/kernel/security/lsm | grep landlock
```

### Seatbelt Permission Denied

```bash
# Check sandbox-exec exists
ls -la /usr/bin/sandbox-exec

# Check SIP status (may affect sandbox)
csrutil status
```

### Command Fails in Sandbox

```rust
// Add the required path to allowed_paths
let config = SandboxConfig::new(working_dir)
    .with_allowed_paths(vec![
        PathBuf::from("/path/command/needs"),
    ]);

// Or exclude the command entirely
let config = SandboxConfig::new(working_dir)
    .with_excluded_commands(vec!["problematic-cmd".into()]);
```
