//! Command execution for custom hotkeys and external tools

use crate::{CustomHotkey, ExternalTool, TemplateContext};
use std::process::{Command, Stdio};

/// Execute a custom hotkey command
pub fn execute_hotkey(hotkey: &CustomHotkey, ctx: &TemplateContext) -> std::io::Result<()> {
    let command = hotkey.expand_command(ctx);
    tracing::info!("Executing custom hotkey command: {}", command);

    if hotkey.open_terminal {
        open_in_terminal(&command)
    } else {
        // Run in background without blocking
        #[cfg(unix)]
        {
            Command::new("sh")
                .arg("-c")
                .arg(&command)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }
        #[cfg(windows)]
        {
            Command::new("cmd")
                .args(["/C", &command])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
        }
        Ok(())
    }
}

/// Execute an external tool
pub fn execute_tool(tool: &ExternalTool, ctx: &TemplateContext) -> std::io::Result<()> {
    let args = tool.expand_args(ctx);
    tracing::info!("Launching external tool: {} {:?}", tool.command, args);

    // Pre-flight check: verify the command exists on PATH
    if !check_command_exists(&tool.command) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "'{}' not found on PATH. Install it or remove the '{}' plugin from config.",
                tool.command, tool.name
            ),
        ));
    }

    open_in_terminal_with_args(&tool.command, &args)
}

/// Check whether a command exists on PATH
pub fn check_command_exists(cmd: &str) -> bool {
    #[cfg(unix)]
    {
        Command::new("which")
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("where")
            .arg(cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Open a command in a new terminal window
#[cfg(target_os = "macos")]
fn open_in_terminal(command: &str) -> std::io::Result<()> {
    // Use osascript to open Terminal.app with the command
    let script = format!(
        r#"tell application "Terminal"
            do script "{}"
            activate
        end tell"#,
        command.replace('\\', "\\\\").replace('"', "\\\"")
    );

    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

/// Open a command with args in a new terminal window
#[cfg(target_os = "macos")]
fn open_in_terminal_with_args(cmd: &str, args: &[String]) -> std::io::Result<()> {
    // Build full command string
    let full_cmd = if args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, args.join(" "))
    };

    open_in_terminal(&full_cmd)
}

/// Open a command in a new terminal window (Linux)
#[cfg(target_os = "linux")]
fn open_in_terminal(command: &str) -> std::io::Result<()> {
    // Try common terminal emulators in order of preference
    let terminals = [
        ("gnome-terminal", vec!["--", "sh", "-c"]),
        ("konsole", vec!["-e", "sh", "-c"]),
        ("xterm", vec!["-e", "sh", "-c"]),
        ("x-terminal-emulator", vec!["-e", "sh", "-c"]),
    ];

    for (term, args) in terminals {
        if Command::new("which")
            .arg(term)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            let mut cmd = Command::new(term);
            for arg in &args {
                cmd.arg(arg);
            }
            cmd.arg(command)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            return Ok(());
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "No terminal emulator found",
    ))
}

#[cfg(target_os = "linux")]
fn open_in_terminal_with_args(cmd: &str, args: &[String]) -> std::io::Result<()> {
    let full_cmd = if args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, args.join(" "))
    };
    open_in_terminal(&full_cmd)
}

/// Open a command in a new terminal window (Windows)
#[cfg(target_os = "windows")]
fn open_in_terminal(command: &str) -> std::io::Result<()> {
    Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", command])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn open_in_terminal_with_args(cmd: &str, args: &[String]) -> std::io::Result<()> {
    let full_cmd = if args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, args.join(" "))
    };
    open_in_terminal(&full_cmd)
}

/// Parse a hotkey string like "Ctrl+Shift+L" into components
#[derive(Debug, Clone, Default)]
pub struct ParsedHotkey {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
    pub key: String,
}

impl ParsedHotkey {
    /// Parse a hotkey string like "Ctrl+Shift+L" or "Alt+K"
    pub fn parse(hotkey: &str) -> Self {
        let mut result = Self::default();
        let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();

        for part in parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => result.ctrl = true,
                "shift" => result.shift = true,
                "alt" | "option" => result.alt = true,
                "meta" | "cmd" | "command" | "win" | "super" => result.meta = true,
                _ => result.key = part.to_string(),
            }
        }

        result
    }

    /// Check if this hotkey matches the given keyboard event modifiers and key.
    ///
    /// Only modifiers explicitly defined in the hotkey string are required.
    /// For example, `"Ctrl+L"` requires Ctrl to be held and the key to be "L",
    /// but does not care about Shift/Alt/Meta state. If `"Ctrl+Shift+L"` is
    /// defined, both Ctrl and Shift must be held.
    pub fn matches(&self, key: &str, ctrl: bool, shift: bool, alt: bool, meta: bool) -> bool {
        // Required modifiers must be pressed
        if self.ctrl && !ctrl {
            return false;
        }
        if self.shift && !shift {
            return false;
        }
        if self.alt && !alt {
            return false;
        }
        if self.meta && !meta {
            return false;
        }
        // Prevent matching when extra modifiers are held that weren't specified.
        // e.g., "L" (no modifiers) should not match Ctrl+L.
        if !self.ctrl && ctrl {
            return false;
        }
        if !self.alt && alt {
            return false;
        }
        if !self.meta && meta {
            return false;
        }
        // Note: we intentionally do NOT reject extra Shift, because pressing
        // Shift is often implicit with uppercase letters and varies by platform.
        self.key.eq_ignore_ascii_case(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hotkey() {
        let hotkey = ParsedHotkey::parse("Ctrl+Shift+L");
        assert!(hotkey.ctrl);
        assert!(hotkey.shift);
        assert!(!hotkey.alt);
        assert!(!hotkey.meta);
        assert_eq!(hotkey.key, "L");

        let hotkey2 = ParsedHotkey::parse("Alt+K");
        assert!(!hotkey2.ctrl);
        assert!(!hotkey2.shift);
        assert!(hotkey2.alt);
        assert!(!hotkey2.meta);
        assert_eq!(hotkey2.key, "K");
    }

    #[test]
    fn test_parse_hotkey_aliases() {
        let h = ParsedHotkey::parse("Control+Option+X");
        assert!(h.ctrl);
        assert!(h.alt);
        assert_eq!(h.key, "X");

        let h2 = ParsedHotkey::parse("Command+S");
        assert!(h2.meta);
        assert_eq!(h2.key, "S");

        let h3 = ParsedHotkey::parse("Super+W");
        assert!(h3.meta);
        assert_eq!(h3.key, "W");
    }

    #[test]
    fn test_hotkey_matches_required_modifiers() {
        let hotkey = ParsedHotkey::parse("Ctrl+Shift+L");
        // Exact match
        assert!(hotkey.matches("l", true, true, false, false));
        assert!(hotkey.matches("L", true, true, false, false));
        // Missing required modifier (Shift)
        assert!(!hotkey.matches("L", true, false, false, false));
        // Missing required modifier (Ctrl)
        assert!(!hotkey.matches("L", false, true, false, false));
    }

    #[test]
    fn test_hotkey_shift_tolerance() {
        // "Ctrl+L" should match even if Shift happens to be pressed (platform variance)
        let hotkey = ParsedHotkey::parse("Ctrl+L");
        assert!(hotkey.matches("L", true, false, false, false));
        assert!(hotkey.matches("L", true, true, false, false)); // extra Shift OK
    }

    #[test]
    fn test_hotkey_rejects_extra_ctrl_alt_meta() {
        // "L" alone should NOT match Ctrl+L
        let hotkey = ParsedHotkey::parse("L");
        assert!(hotkey.matches("L", false, false, false, false));
        assert!(!hotkey.matches("L", true, false, false, false)); // extra Ctrl rejected
        assert!(!hotkey.matches("L", false, false, true, false)); // extra Alt rejected
        assert!(!hotkey.matches("L", false, false, false, true)); // extra Meta rejected
    }

    #[test]
    fn test_check_command_exists() {
        // "echo" should exist on all platforms
        assert!(check_command_exists("echo"));
        // nonsense command should not exist
        assert!(!check_command_exists("kubestudio_nonexistent_command_xyz"));
    }
}
