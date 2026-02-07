//! macOS Seatbelt sandbox implementation using sandbox-exec.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use super::{SandboxConfig, SandboxError, SandboxResult, SandboxRuntime};

pub struct SeatbeltSandbox {
    profile: String,
    profile_path: Option<PathBuf>,
}

impl SeatbeltSandbox {
    pub fn new(config: &SandboxConfig) -> Self {
        let profile = generate_profile(config);
        Self {
            profile,
            profile_path: None,
        }
    }
}

impl Drop for SeatbeltSandbox {
    fn drop(&mut self) {
        if let Some(path) = self.profile_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn is_seatbelt_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        Path::new("/usr/bin/sandbox-exec").exists()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

fn escape_seatbelt_string(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\0' && *c != '\n' && *c != '\r')
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn generate_profile(config: &SandboxConfig) -> String {
    let working_dir = escape_seatbelt_string(&config.working_dir.display().to_string());
    let home = escape_seatbelt_string(&std::env::var("HOME").unwrap_or_else(|_| "/".into()));

    let mut allowed_subpaths = String::new();
    for path in &config.allowed_paths {
        let escaped = escape_seatbelt_string(&path.display().to_string());
        allowed_subpaths.push_str(&format!("(allow file-read* (subpath \"{}\"))\n", escaped));
    }

    let network = &config.network;
    let network_rules = if network.has_proxy() {
        let http_port = network.http_proxy_port.unwrap_or(0);
        let socks_port = network.socks_proxy_port.unwrap_or(0);
        format!(
            r#"(allow network-outbound (remote tcp "localhost:{http_port}"))
(allow network-outbound (remote tcp "127.0.0.1:{http_port}"))
(allow network-outbound (remote tcp "localhost:{socks_port}"))
(allow network-outbound (remote tcp "127.0.0.1:{socks_port}"))
(allow network-outbound (remote unix-socket))"#
        )
    } else {
        "(allow network-outbound)".into()
    };

    let mut socket_rules = String::new();
    if !network.allow_unix_sockets.is_empty() {
        socket_rules.push_str("(allow network* (local unix-socket))");
    }
    if network.allow_local_binding {
        socket_rules.push_str("\n(allow network-bind (local ip \"localhost:*\"))");
    }

    format!(
        r#"(version 1)
(deny default)

;; Allow reading system paths
(allow file-read* (subpath "/usr"))
(allow file-read* (subpath "/bin"))
(allow file-read* (subpath "/sbin"))
(allow file-read* (subpath "/Library"))
(allow file-read* (subpath "/System"))
(allow file-read* (subpath "/private/etc"))
(allow file-read* (subpath "/private/var/db"))
(allow file-read* (subpath "/var"))
(allow file-read* (subpath "/etc"))
(allow file-read* (subpath "/dev"))
(allow file-read* (subpath "/tmp"))
(allow file-read* (subpath "/private/tmp"))

;; Allow reading home directory essentials
(allow file-read* (subpath "{home}/.cargo"))
(allow file-read* (subpath "{home}/.rustup"))
(allow file-read* (subpath "{home}/.npm"))
(allow file-read* (subpath "{home}/.nvm"))
(allow file-read* (subpath "{home}/.local"))

;; Working directory: full access
(allow file-read* file-write* file-ioctl (subpath "{working_dir}"))

;; Temp directories: full access
(allow file-read* file-write* file-ioctl (subpath "/tmp"))
(allow file-read* file-write* file-ioctl (subpath "/private/tmp"))
(allow file-read* file-write* file-ioctl (subpath "/var/folders"))
(allow file-read* file-write* file-ioctl (subpath "/private/var/folders"))

;; Additional allowed paths
{allowed_subpaths}

;; Process execution
(allow process-exec (subpath "/bin"))
(allow process-exec (subpath "/usr/bin"))
(allow process-exec (subpath "/usr/local/bin"))
(allow process-exec (subpath "{home}/.cargo/bin"))
(allow process-exec (subpath "{working_dir}"))
(allow process-fork)

;; Basic syscalls
(allow sysctl-read)
(allow mach-lookup)
(allow ipc-posix-shm-read-data)
(allow ipc-posix-shm-write-data)
(allow signal)

;; Network rules
{network_rules}
{socket_rules}

;; Allow DNS resolution
(allow network-outbound (remote udp "*:53"))
(allow network-outbound (remote tcp "*:53"))
"#
    )
}

impl SandboxRuntime for SeatbeltSandbox {
    fn is_available(&self) -> bool {
        is_seatbelt_supported()
    }

    fn apply(&self) -> SandboxResult<()> {
        Err(SandboxError::InvalidConfig(
            "Seatbelt requires command wrapping, cannot apply to current process".into(),
        ))
    }

    fn wrap_command(&self, command: &str) -> SandboxResult<String> {
        let profile_path = write_profile_to_temp(&self.profile)?;
        let profile_str = profile_path.display().to_string();

        Ok(format!(
            "/usr/bin/sandbox-exec -f {} bash -c {}; rm -f {}",
            shell_escape(&profile_str),
            shell_escape(command),
            shell_escape(&profile_str)
        ))
    }

    fn environment_vars(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

fn write_profile_to_temp(profile: &str) -> SandboxResult<PathBuf> {
    use std::fs::OpenOptions;

    let path = PathBuf::from(format!("/tmp/claude-sandbox-{}.sb", uuid::Uuid::new_v4()));

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(SandboxError::Io)?;

    file.write_all(profile.as_bytes())
        .map_err(SandboxError::Io)?;
    file.sync_all().map_err(SandboxError::Io)?;

    Ok(path)
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_profile_generation() {
        let config = SandboxConfig::new(PathBuf::from("/tmp/test"));
        let sandbox = SeatbeltSandbox::new(&config);

        assert!(sandbox.profile.contains("(version 1)"));
        assert!(sandbox.profile.contains("/tmp/test"));
    }

    #[test]
    fn test_wrap_command() {
        if !is_seatbelt_supported() {
            return;
        }

        let config = SandboxConfig::new(PathBuf::from("/tmp"));
        let sandbox = SeatbeltSandbox::new(&config);

        let wrapped = sandbox.wrap_command("echo hello").unwrap();
        assert!(wrapped.contains("sandbox-exec"));
        assert!(wrapped.contains("-f"));
        assert!(wrapped.contains("rm -f"));
    }

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("simple"), "'simple'");
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[test]
    fn test_seatbelt_escape() {
        assert_eq!(escape_seatbelt_string("simple"), "simple");
        assert_eq!(escape_seatbelt_string("with\"quote"), "with\\\"quote");
        assert_eq!(escape_seatbelt_string("back\\slash"), "back\\\\slash");
        assert_eq!(escape_seatbelt_string("new\nline"), "newline");
        assert_eq!(escape_seatbelt_string("null\0char"), "nullchar");
    }
}
