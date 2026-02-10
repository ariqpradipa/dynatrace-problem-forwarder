use crate::error::{ForwarderError, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Get the PID file path based on config location
pub fn get_pid_file_path(config_path: &Path) -> PathBuf {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    config_dir.join("dtpf.pid")
}

/// Get the log file path for nohup output
pub fn get_log_file_path(config_path: &Path) -> PathBuf {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    config_dir.join("dtpf.log")
}

/// Start the forwarder in background using nohup
pub fn start_background(config_path: &Path) -> Result<u32> {
    let pid_file = get_pid_file_path(config_path);
    let log_file = get_log_file_path(config_path);

    // Check if already running
    if pid_file.exists() {
        if let Ok(pid_str) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                if is_process_running(pid) {
                    return Err(ForwarderError::Config(format!(
                        "dtpf is already running with PID {}. Use 'dtpf stop' to stop it first.",
                        pid
                    )));
                }
            }
        }
        // Stale PID file, remove it
        let _ = fs::remove_file(&pid_file);
    }

    // Get the current executable path
    let exe_path = std::env::current_exe()
        .map_err(|e| ForwarderError::Config(format!("Failed to get executable path: {}", e)))?;

    // Build the command
    let mut cmd = Command::new(&exe_path);
    cmd.arg("run")
        .arg("--config")
        .arg(config_path.canonicalize().unwrap_or_else(|_| config_path.to_path_buf()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Spawn the process
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // Create a new process group to detach from parent
        unsafe {
            cmd.pre_exec(|| {
                // Create new session
                libc::setsid();
                Ok(())
            });
        }
    }

    let child = cmd.spawn()
        .map_err(|e| ForwarderError::Config(format!("Failed to start background process: {}", e)))?;

    let pid = child.id();

    // Write PID to file
    let mut pid_file_handle = fs::File::create(&pid_file)
        .map_err(|e| ForwarderError::Config(format!("Failed to create PID file: {}", e)))?;

    writeln!(pid_file_handle, "{}", pid)
        .map_err(|e| ForwarderError::Config(format!("Failed to write PID file: {}", e)))?;

    println!("✓ dtpf started in background with PID {}", pid);
    println!("  PID file: {}", pid_file.display());
    println!("  Log file: {}", log_file.display());
    println!("\nUse 'dtpf stop' to stop the service");

    Ok(pid)
}

/// Stop the background forwarder process
pub fn stop_background(config_path: &Path) -> Result<()> {
    let pid_file = get_pid_file_path(config_path);

    if !pid_file.exists() {
        return Err(ForwarderError::Config(
            "No PID file found. dtpf may not be running in background.".to_string()
        ));
    }

    let pid_str = fs::read_to_string(&pid_file)
        .map_err(|e| ForwarderError::Config(format!("Failed to read PID file: {}", e)))?;

    let pid = pid_str.trim().parse::<u32>()
        .map_err(|e| ForwarderError::Config(format!("Invalid PID in file: {}", e)))?;

    if !is_process_running(pid) {
        println!("⚠ Process with PID {} is not running", pid);
        fs::remove_file(&pid_file)
            .map_err(|e| ForwarderError::Config(format!("Failed to remove PID file: {}", e)))?;
        return Ok(());
    }

    // Send SIGTERM to gracefully stop the process
    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        let nix_pid = Pid::from_raw(pid as i32);
        kill(nix_pid, Signal::SIGTERM)
            .map_err(|e| ForwarderError::Config(format!("Failed to send SIGTERM: {}", e)))?;
    }

    #[cfg(windows)]
    {
        // On Windows, use taskkill
        Command::new("taskkill")
            .args(&["/PID", &pid.to_string(), "/F"])
            .output()
            .map_err(|e| ForwarderError::Config(format!("Failed to kill process: {}", e)))?;
    }

    println!("✓ Sent termination signal to PID {}", pid);
    println!("  Waiting for process to exit...");

    // Wait up to 10 seconds for graceful shutdown
    for i in 0..10 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if !is_process_running(pid) {
            break;
        }
        if i == 9 {
            println!("⚠ Process did not exit gracefully, you may need to kill it manually");
            return Ok(());
        }
    }

    // Clean up PID file
    fs::remove_file(&pid_file)
        .map_err(|e| ForwarderError::Config(format!("Failed to remove PID file: {}", e)))?;

    println!("✓ dtpf stopped successfully");

    Ok(())
}

/// Check if a process is running
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        let nix_pid = Pid::from_raw(pid as i32);
        // Passing None as signal doesn't send a signal, just checks if process exists
        // If the process doesn't exist, kill will return ESRCH error
        kill(nix_pid, None).is_ok()
    }

    #[cfg(windows)]
    {
        // On Windows, use tasklist to check if process exists
        if let Ok(output) = Command::new("tasklist")
            .args(&["/FI", &format!("PID eq {}", pid)])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(&pid.to_string())
        } else {
            false
        }
    }
}
