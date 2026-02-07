//! Linux Landlock LSM sandbox implementation.

use std::collections::HashMap;
use std::path::Path;

use super::{SandboxConfig, SandboxError, SandboxResult, SandboxRuntime};

#[cfg(target_os = "linux")]
use landlock::{
    ABI, Access, AccessFs, AccessNet, NetPort, PathBeneath, PathFd, Ruleset, RulesetAttr,
    RulesetCreatedAttr,
};

pub struct LandlockSandbox {
    config: SandboxConfig,
    abi: Option<ABI>,
}

impl LandlockSandbox {
    pub fn new(config: SandboxConfig) -> Self {
        let abi = best_effort_abi();
        Self { config, abi }
    }
}

#[cfg(target_os = "linux")]
fn best_effort_abi() -> Option<ABI> {
    [ABI::V4, ABI::V3, ABI::V2, ABI::V1]
        .into_iter()
        .find(|&abi| {
            Ruleset::default()
                .handle_access(AccessFs::from_all(abi))
                .is_ok()
        })
}

#[cfg(not(target_os = "linux"))]
fn best_effort_abi() -> Option<()> {
    None
}

pub fn is_landlock_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        best_effort_abi().is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(target_os = "linux")]
impl SandboxRuntime for LandlockSandbox {
    fn is_available(&self) -> bool {
        self.abi.is_some()
    }

    fn apply(&self) -> SandboxResult<()> {
        let abi = self
            .abi
            .ok_or_else(|| SandboxError::NotAvailable("Landlock not supported".into()))?;

        let mut base_ruleset = Ruleset::default()
            .handle_access(AccessFs::from_all(abi))
            .map_err(|e| SandboxError::Creation(e.to_string()))?;

        let net_access = AccessNet::from_all(abi);
        if !net_access.is_empty() {
            base_ruleset = base_ruleset
                .handle_access(net_access)
                .map_err(|e| SandboxError::Creation(e.to_string()))?;
        }

        let mut ruleset = base_ruleset
            .create()
            .map_err(|e| SandboxError::Creation(e.to_string()))?;

        ruleset = self.add_working_dir_rule(ruleset, abi)?;
        ruleset = self.add_allowed_paths_rules(ruleset, abi)?;
        ruleset = self.add_system_paths_rules(ruleset, abi)?;

        if !net_access.is_empty() {
            ruleset = self.add_network_rules(ruleset)?;
        }

        ruleset
            .restrict_self()
            .map_err(|e| SandboxError::RuleApplication(e.to_string()))?;

        Ok(())
    }

    fn wrap_command(&self, command: &str) -> SandboxResult<String> {
        Ok(command.to_string())
    }

    fn environment_vars(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

#[cfg(target_os = "linux")]
impl LandlockSandbox {
    fn add_working_dir_rule(
        &self,
        ruleset: landlock::RulesetCreated,
        abi: ABI,
    ) -> SandboxResult<landlock::RulesetCreated> {
        if !self.config.working_dir.exists() {
            return Ok(ruleset);
        }

        let fd = PathFd::new(&self.config.working_dir)
            .map_err(|_| SandboxError::PathNotAccessible(self.config.working_dir.clone()))?;

        ruleset
            .add_rule(PathBeneath::new(fd, AccessFs::from_all(abi)))
            .map_err(|e| SandboxError::RuleApplication(e.to_string()))
    }

    fn add_allowed_paths_rules(
        &self,
        mut ruleset: landlock::RulesetCreated,
        abi: ABI,
    ) -> SandboxResult<landlock::RulesetCreated> {
        for path in &self.config.allowed_paths {
            if !path.exists() {
                continue;
            }

            let fd =
                PathFd::new(path).map_err(|_| SandboxError::PathNotAccessible(path.clone()))?;

            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, AccessFs::from_read(abi)))
                .map_err(|e| SandboxError::RuleApplication(e.to_string()))?;
        }
        Ok(ruleset)
    }

    fn add_network_rules(
        &self,
        mut ruleset: landlock::RulesetCreated,
    ) -> SandboxResult<landlock::RulesetCreated> {
        let allowed_ports: &[u16] = &[
            53,  // DNS
            80,  // HTTP
            443, // HTTPS
        ];

        for &port in allowed_ports {
            ruleset = ruleset
                .add_rule(NetPort::new(port, AccessNet::ConnectTcp))
                .map_err(|e| SandboxError::RuleApplication(e.to_string()))?;
        }

        let network = &self.config.network;
        if let Some(http_port) = network.http_proxy_port {
            ruleset = ruleset
                .add_rule(NetPort::new(http_port, AccessNet::ConnectTcp))
                .map_err(|e| SandboxError::RuleApplication(e.to_string()))?;
        }
        if let Some(socks_port) = network.socks_proxy_port {
            ruleset = ruleset
                .add_rule(NetPort::new(socks_port, AccessNet::ConnectTcp))
                .map_err(|e| SandboxError::RuleApplication(e.to_string()))?;
        }

        if network.allow_local_binding {
            ruleset = ruleset
                .add_rule(NetPort::new(0, AccessNet::BindTcp))
                .map_err(|e| SandboxError::RuleApplication(e.to_string()))?;
        }

        Ok(ruleset)
    }

    fn add_system_paths_rules(
        &self,
        mut ruleset: landlock::RulesetCreated,
        abi: ABI,
    ) -> SandboxResult<landlock::RulesetCreated> {
        let system_paths = [
            "/usr", "/lib", "/lib64", "/lib32", "/bin", "/sbin", "/etc", "/proc", "/sys", "/dev",
            "/tmp", "/var/tmp",
        ];

        for path in system_paths {
            let path = Path::new(path);
            if !path.exists() {
                continue;
            }

            let fd = match PathFd::new(path) {
                Ok(fd) => fd,
                Err(_) => continue,
            };

            let access = if path.starts_with("/tmp") || path.starts_with("/var/tmp") {
                AccessFs::from_all(abi)
            } else {
                AccessFs::from_read(abi)
            };

            ruleset = ruleset
                .add_rule(PathBeneath::new(fd, access))
                .map_err(|e| SandboxError::RuleApplication(e.to_string()))?;
        }

        Ok(ruleset)
    }
}

#[cfg(not(target_os = "linux"))]
impl SandboxRuntime for LandlockSandbox {
    fn is_available(&self) -> bool {
        false
    }

    fn apply(&self) -> SandboxResult<()> {
        Err(SandboxError::NotSupported)
    }

    fn wrap_command(&self, command: &str) -> SandboxResult<String> {
        Ok(command.to_string())
    }

    fn environment_vars(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_landlock_availability() {
        let _supported = is_landlock_supported();
    }

    #[test]
    fn test_landlock_sandbox_creation() {
        let config = SandboxConfig::new(PathBuf::from("/tmp"));
        let sandbox = LandlockSandbox::new(config);
        let _available = sandbox.is_available();
    }
}
