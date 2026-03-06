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

    open_in_terminal_with_args(&tool.command, &args)
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

    /// Check if this hotkey matches the given keyboard event modifiers and key
    pub fn matches(&self, key: &str, ctrl: bool, shift: bool, alt: bool, meta: bool) -> bool {
        self.ctrl == ctrl
            && self.shift == shift
            && self.alt == alt
            && self.meta == meta
            && self.key.eq_ignore_ascii_case(key)
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
    fn test_hotkey_matches() {
        let hotkey = ParsedHotkey::parse("Ctrl+Shift+L");
        assert!(hotkey.matches("l", true, true, false, false));
        assert!(hotkey.matches("L", true, true, false, false));
        assert!(!hotkey.matches("L", true, false, false, false));
    }
}
