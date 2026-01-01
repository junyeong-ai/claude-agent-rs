//! Process manager for background shell execution.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Unique identifier for a managed process.
pub type ProcessId = String;

/// Information about a running process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Unique process identifier.
    pub id: ProcessId,
    /// The command that was executed.
    pub command: String,
    /// When the process was started.
    pub started_at: Instant,
    /// OS process ID if available.
    pub pid: Option<u32>,
}

struct ManagedProcess {
    child: Child,
    info: ProcessInfo,
    output_buffer: String,
}

/// Manager for background shell processes.
#[derive(Clone)]
pub struct ProcessManager {
    processes: Arc<Mutex<HashMap<ProcessId, ManagedProcess>>>,
}

impl ProcessManager {
    /// Create a new process manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new background process.
    pub async fn spawn(&self, command: &str, working_dir: &Path) -> Result<ProcessId, String> {
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(command);
        cmd.current_dir(working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;

        let id = uuid::Uuid::new_v4().to_string();
        let pid = child.id();

        let info = ProcessInfo {
            id: id.clone(),
            command: command.to_string(),
            started_at: Instant::now(),
            pid,
        };

        let managed = ManagedProcess {
            child,
            info,
            output_buffer: String::new(),
        };

        self.processes.lock().await.insert(id.clone(), managed);
        Ok(id)
    }

    /// Kill a background process.
    pub async fn kill(&self, id: &ProcessId) -> Result<(), String> {
        let mut processes = self.processes.lock().await;

        if let Some(mut proc) = processes.remove(id) {
            proc.child
                .kill()
                .await
                .map_err(|e| format!("Failed to kill: {}", e))
        } else {
            Err(format!("Process '{}' not found", id))
        }
    }

    /// Get output from a background process (non-blocking read of available output).
    pub async fn get_output(&self, id: &ProcessId) -> Result<String, String> {
        let mut processes = self.processes.lock().await;

        let proc = processes
            .get_mut(id)
            .ok_or_else(|| format!("Process '{}' not found", id))?;

        // Try to read available stdout
        if let Some(ref mut stdout) = proc.child.stdout {
            let mut buf = vec![0u8; 8192];
            match tokio::time::timeout(std::time::Duration::from_millis(100), stdout.read(&mut buf))
                .await
            {
                Ok(Ok(n)) if n > 0 => {
                    let s = String::from_utf8_lossy(&buf[..n]);
                    proc.output_buffer.push_str(&s);
                }
                _ => {}
            }
        }

        // Try to read available stderr
        if let Some(ref mut stderr) = proc.child.stderr {
            let mut buf = vec![0u8; 8192];
            match tokio::time::timeout(std::time::Duration::from_millis(100), stderr.read(&mut buf))
                .await
            {
                Ok(Ok(n)) if n > 0 => {
                    let s = String::from_utf8_lossy(&buf[..n]);
                    proc.output_buffer.push_str(&s);
                }
                _ => {}
            }
        }

        Ok(proc.output_buffer.clone())
    }

    /// Check if a process is still running.
    pub async fn is_running(&self, id: &ProcessId) -> bool {
        let mut processes = self.processes.lock().await;

        if let Some(proc) = processes.get_mut(id) {
            matches!(proc.child.try_wait(), Ok(None))
        } else {
            false
        }
    }

    /// List all tracked processes.
    pub async fn list(&self) -> Vec<ProcessInfo> {
        self.processes
            .lock()
            .await
            .values()
            .map(|p| p.info.clone())
            .collect()
    }

    /// Clean up finished processes and return their final output.
    pub async fn cleanup_finished(&self) -> Vec<(ProcessInfo, String)> {
        let mut processes = self.processes.lock().await;
        let mut finished = Vec::new();

        let ids: Vec<_> = processes.keys().cloned().collect();
        for id in ids {
            if let Some(proc) = processes.get_mut(&id)
                && let Ok(Some(_status)) = proc.child.try_wait()
                && let Some(proc) = processes.remove(&id)
            {
                finished.push((proc.info, proc.output_buffer));
            }
        }

        finished
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_spawn_and_list() {
        let mgr = ProcessManager::new();
        let id = mgr
            .spawn("sleep 0.1", &PathBuf::from("/tmp"))
            .await
            .unwrap();

        let list = mgr.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(!mgr.is_running(&id).await);
    }

    #[tokio::test]
    async fn test_kill() {
        let mgr = ProcessManager::new();
        let id = mgr.spawn("sleep 10", &PathBuf::from("/tmp")).await.unwrap();

        assert!(mgr.is_running(&id).await);
        mgr.kill(&id).await.unwrap();
        assert!(!mgr.is_running(&id).await);
    }

    #[tokio::test]
    async fn test_get_output() {
        let mgr = ProcessManager::new();
        let id = mgr
            .spawn("echo hello", &PathBuf::from("/tmp"))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let output = mgr.get_output(&id).await.unwrap();
        assert!(output.contains("hello"));
    }
}
