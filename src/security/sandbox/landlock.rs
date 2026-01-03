//! Linux Landlock LSM sandbox implementation.

use std::collections::HashMap;
use std::path::Path;

use super::{SandboxConfig, SandboxError, SandboxResult, SandboxRuntime};

#[cfg(target_os = "linux")]
use landlock::{
    ABI, Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr,
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
    for abi in [ABI::V4, ABI::V3, ABI::V2, ABI::V1] {
        if Ruleset::default()
            .handle_access(AccessFs::from_all(abi))
            .is_ok()
        {
            return Some(abi);
        }
    }
    None
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

        let mut ruleset = Ruleset::default()
            .handle_access(AccessFs::from_all(abi))
            .map_err(|e| SandboxError::Creation(e.to_string()))?
            .create()
            .map_err(|e| SandboxError::Creation(e.to_string()))?;

        ruleset = self.add_working_dir_rule(ruleset, abi)?;
        ruleset = self.add_allowed_paths_rules(ruleset, abi)?;
        ruleset = self.add_system_paths_rules(ruleset, abi)?;

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

            ruleset = match ruleset.add_rule(PathBeneath::new(fd, access)) {
                Ok(r) => r,
                Err(_) => continue,
            };
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
        let supported = is_landlock_supported();
        println!("Landlock supported: {}", supported);
    }

    #[test]
    fn test_landlock_sandbox_creation() {
        let config = SandboxConfig::new(PathBuf::from("/tmp"));
        let sandbox = LandlockSandbox::new(config);

        if sandbox.is_available() {
            println!("Landlock ABI: {:?}", sandbox.abi);
        }
    }
}
