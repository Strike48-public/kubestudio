// Embedded terminal/exec viewer component

use crate::hooks::ClusterContext;
use dioxus::prelude::*;

/// Simple terminal buffer that maintains a 2D grid of characters
/// Handles cursor movement, backspace, and basic escape sequences
#[derive(Clone)]
struct TerminalBuffer {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
    max_lines: usize,
}

impl Default for TerminalBuffer {
    fn default() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            max_lines: 1000,
        }
    }
}

impl TerminalBuffer {
    /// Process incoming data and update the buffer
    fn process(&mut self, data: &[u8]) {
        let text = String::from_utf8_lossy(data);
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '\x1b' => {
                    // ESC - start of escape sequence
                    if chars.peek() == Some(&'[') {
                        chars.next(); // consume '['
                        self.process_csi(&mut chars);
                    } else if chars.peek() == Some(&']') {
                        // OSC sequence - skip until BEL or ST
                        chars.next();
                        for ch in chars.by_ref() {
                            if ch == '\x07' || ch == '\x1b' {
                                break;
                            }
                        }
                    }
                    // Ignore other escape sequences
                }
                '\n' => {
                    // Newline - move to next line
                    self.cursor_row += 1;
                    self.cursor_col = 0;
                    self.ensure_line_exists();
                }
                '\r' => {
                    // Carriage return - move to start of line
                    self.cursor_col = 0;
                }
                '\x08' | '\x7f' => {
                    // Backspace or DEL - move cursor back and delete character
                    if self.cursor_col > 0 {
                        self.cursor_col -= 1;
                        self.ensure_line_exists();
                        let line = &mut self.lines[self.cursor_row];
                        if self.cursor_col < line.len() {
                            line.remove(self.cursor_col);
                        }
                    }
                }
                '\t' => {
                    // Tab - move to next tab stop (every 8 columns)
                    let next_tab = ((self.cursor_col / 8) + 1) * 8;
                    while self.cursor_col < next_tab {
                        self.put_char(' ');
                    }
                }
                c if c.is_control() => {
                    // Ignore other control characters
                }
                c => {
                    // Regular character - write at cursor position
                    self.put_char(c);
                }
            }
        }

        // Trim to max lines
        while self.lines.len() > self.max_lines {
            self.lines.remove(0);
            if self.cursor_row > 0 {
                self.cursor_row -= 1;
            }
        }
    }

    /// Process CSI (Control Sequence Introducer) escape sequence
    fn process_csi(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>) {
        // Parse parameters (numbers separated by semicolons)
        let mut params: Vec<usize> = Vec::new();
        let mut current_param = String::new();

        loop {
            match chars.peek() {
                Some(&c) if c.is_ascii_digit() => {
                    current_param.push(c);
                    chars.next();
                }
                Some(&';') => {
                    params.push(current_param.parse().unwrap_or(0));
                    current_param.clear();
                    chars.next();
                }
                Some(&c) if (0x40..=0x7E).contains(&(c as u8)) => {
                    // Final byte - command
                    if !current_param.is_empty() {
                        params.push(current_param.parse().unwrap_or(0));
                    }
                    chars.next();
                    self.execute_csi(c, &params);
                    break;
                }
                Some(&c) if (0x20..=0x3F).contains(&(c as u8)) => {
                    // Intermediate byte - skip
                    chars.next();
                }
                _ => break, // Malformed or end
            }
        }
    }

    /// Execute a CSI command
    fn execute_csi(&mut self, cmd: char, params: &[usize]) {
        match cmd {
            'A' => {
                // Cursor Up
                let n = params.first().copied().unwrap_or(1).max(1);
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            'B' => {
                // Cursor Down
                let n = params.first().copied().unwrap_or(1).max(1);
                self.cursor_row += n;
                self.ensure_line_exists();
            }
            'C' => {
                // Cursor Forward (Right)
                let n = params.first().copied().unwrap_or(1).max(1);
                self.cursor_col += n;
            }
            'D' => {
                // Cursor Back (Left)
                let n = params.first().copied().unwrap_or(1).max(1);
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            'H' | 'f' => {
                // Cursor Position (row;col) - 1-based
                let row = params.first().copied().unwrap_or(1).max(1) - 1;
                let col = params.get(1).copied().unwrap_or(1).max(1) - 1;
                self.cursor_row = row;
                self.cursor_col = col;
                self.ensure_line_exists();
            }
            'J' => {
                // Erase in Display
                let mode = params.first().copied().unwrap_or(0);
                match mode {
                    0 => {
                        // Clear from cursor to end of screen
                        self.ensure_line_exists();
                        self.lines[self.cursor_row].truncate(self.cursor_col);
                        self.lines.truncate(self.cursor_row + 1);
                    }
                    1 => {
                        // Clear from start to cursor
                        for i in 0..self.cursor_row {
                            self.lines[i].clear();
                        }
                        if self.cursor_row < self.lines.len() {
                            let line = &mut self.lines[self.cursor_row];
                            let spaces = " ".repeat(self.cursor_col.min(line.len()));
                            line.replace_range(..self.cursor_col.min(line.len()), &spaces);
                        }
                    }
                    2 | 3 => {
                        // Clear entire screen
                        self.lines.clear();
                        self.lines.push(String::new());
                        self.cursor_row = 0;
                        self.cursor_col = 0;
                    }
                    _ => {}
                }
            }
            'K' => {
                // Erase in Line
                let mode = params.first().copied().unwrap_or(0);
                self.ensure_line_exists();
                let line = &mut self.lines[self.cursor_row];
                match mode {
                    0 => {
                        // Clear from cursor to end of line
                        line.truncate(self.cursor_col);
                    }
                    1 => {
                        // Clear from start to cursor
                        let spaces = " ".repeat(self.cursor_col.min(line.len()));
                        line.replace_range(..self.cursor_col.min(line.len()), &spaces);
                    }
                    2 => {
                        // Clear entire line
                        line.clear();
                    }
                    _ => {}
                }
            }
            'P' => {
                // Delete characters
                let n = params.first().copied().unwrap_or(1).max(1);
                self.ensure_line_exists();
                let line = &mut self.lines[self.cursor_row];
                for _ in 0..n {
                    if self.cursor_col < line.len() {
                        line.remove(self.cursor_col);
                    }
                }
            }
            'm' => {
                // SGR (Select Graphic Rendition) - colors/styles
                // We ignore these for now (no color support)
            }
            _ => {
                // Ignore unknown commands
            }
        }
    }

    /// Ensure the current cursor row exists
    fn ensure_line_exists(&mut self) {
        while self.lines.len() <= self.cursor_row {
            self.lines.push(String::new());
        }
    }

    /// Put a character at the cursor position and advance cursor
    fn put_char(&mut self, c: char) {
        self.ensure_line_exists();
        let line = &mut self.lines[self.cursor_row];

        // Pad with spaces if cursor is beyond line length
        while line.len() < self.cursor_col {
            line.push(' ');
        }

        if self.cursor_col < line.len() {
            // Replace character at cursor position
            line.remove(self.cursor_col);
            line.insert(self.cursor_col, c);
        } else {
            line.push(c);
        }
        self.cursor_col += 1;
    }

    /// Get all lines for display
    fn get_lines(&self) -> &[String] {
        &self.lines
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct ExecViewerProps {
    /// Pod name to exec into
    pub pod_name: String,
    /// Namespace of the pod
    pub namespace: String,
    /// Optional container name (if None, uses first/default container)
    pub container: Option<String>,
    /// Cluster connection
    pub cluster: Signal<Option<ClusterContext>>,
    /// Called when user presses Escape
    pub on_back: EventHandler<()>,
}

#[component]
pub fn ExecViewer(props: ExecViewerProps) -> Element {
    // Terminal state - using proper terminal buffer
    let mut term_buffer = use_signal(TerminalBuffer::default);
    let mut connecting = use_signal(|| true);
    let mut connected = use_signal(|| false);
    let mut error_message = use_signal(|| None::<String>);
    let mut should_scroll = use_signal(|| false);

    // Store the stdin sender for input
    let mut stdin_tx = use_signal(|| None::<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>);

    // Clone props for effects
    let pod_name = props.pod_name.clone();
    let namespace = props.namespace.clone();
    let container = props.container.clone();
    let cluster = props.cluster;

    // Start exec session on mount
    use_effect(move || {
        let pod_name = pod_name.clone();
        let namespace = namespace.clone();
        let container = container.clone();

        spawn(async move {
            connecting.set(true);
            error_message.set(None);

            // Initialize buffer with connection message
            let init_msg = format!(
                "Connecting to {}{}...\n",
                pod_name,
                container
                    .as_ref()
                    .map(|c| format!(" ({})", c))
                    .unwrap_or_default()
            );
            term_buffer.write().process(init_msg.as_bytes());

            if let Some(ctx) = cluster.read().clone() {
                // Start exec session with shell
                match ctx
                    .client
                    .exec(&pod_name, &namespace, container.as_deref(), vec!["/bin/sh"])
                    .await
                {
                    Ok(handle) => {
                        // Store the stdin sender
                        stdin_tx.set(Some(handle.stdin_tx.clone()));

                        // Mark as connected
                        connecting.set(false);
                        connected.set(true);

                        let connected_msg = format!(
                            "Connected to {} in {}\n\n",
                            handle.pod_name, handle.namespace
                        );
                        term_buffer.write().process(connected_msg.as_bytes());
                        should_scroll.set(true);

                        // Spawn task to read stdout
                        let mut stdout_rx = handle.stdout_rx;
                        spawn(async move {
                            while let Some(data) = stdout_rx.recv().await {
                                // Process raw data through terminal buffer
                                term_buffer.write().process(&data);
                                should_scroll.set(true);
                            }

                            // Stream ended
                            connected.set(false);
                            term_buffer.write().process(b"\n--- Session ended ---\n");
                        });
                    }
                    Err(e) => {
                        connecting.set(false);
                        error_message.set(Some(format!("Failed to exec: {}", e)));
                    }
                }
            } else {
                connecting.set(false);
                error_message.set(Some("No cluster connection".to_string()));
            }
        });
    });

    // Scroll effect - use peek() to avoid subscribing to the signal we're writing to
    use_effect(move || {
        let do_scroll = *should_scroll.peek();
        if do_scroll {
            should_scroll.set(false);
            spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                let _ = document::eval(
                    r#"
                    const termContent = document.querySelector('.exec-content');
                    if (termContent) {
                        termContent.scrollTop = termContent.scrollHeight;
                    }
                    "#,
                );
            });
        }
    });

    let on_back = props.on_back;
    let on_back_key = props.on_back;
    let pod_name_display = props.pod_name.clone();
    let namespace_display = props.namespace.clone();
    let container_display = props.container.clone();

    // Get lines from terminal buffer for display
    let lines: Vec<String> = term_buffer.read().get_lines().to_vec();
    let line_count = lines.len();

    rsx! {
        div { class: "exec-viewer-container",
            div { class: "exec-viewer-header",
                h3 { "Shell: {pod_name_display}" }
                if let Some(c) = &container_display {
                    span { class: "container-name", "({c})" }
                }
                span { class: "container-name", "in {namespace_display}" }

                // Status indicator
                div { class: "exec-status",
                    if *connecting.read() {
                        span { class: "status-badge status-warning", "Connecting..." }
                    } else if *connected.read() {
                        span { class: "status-badge status-success", "Connected" }
                    } else {
                        span { class: "status-badge status-error", "Disconnected" }
                    }
                }

                button {
                    class: "back-btn",
                    onclick: move |_| on_back.call(()),
                    "Back (Esc)"
                }
            }

            // Hint bar
            div { class: "exec-hints",
                "Type commands and press Enter • Ctrl+C: Interrupt • Ctrl+D: Exit shell • Esc: Close view"
            }

            div { class: "exec-viewer",
                if let Some(err) = error_message.read().as_ref() {
                    div { class: "exec-error", "{err}" }
                } else {
                    pre {
                        class: "exec-content",
                        tabindex: 0,
                        onmounted: move |e| {
                            let data = e.data();
                            spawn(async move {
                                let _ = data.set_focus(true).await;
                            });
                        },
                        onkeydown: move |e: KeyboardEvent| {
                            // Handle escape to go back
                            if e.key() == Key::Escape {
                                on_back_key.call(());
                                e.stop_propagation();
                                e.prevent_default();
                                return;
                            }

                            // Don't handle input if not connected
                            if !*connected.read() {
                                return;
                            }

                            let stdin = stdin_tx.read().clone();

                            match e.key() {
                                Key::Enter => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(b"\n".to_vec());
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::Backspace => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x7f]); // DEL character
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                // Also handle backspace that comes through as character
                                Key::Character(ref c) if c == "\u{8}" || c == "\x7f" => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x7f]); // DEL character
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::Character(ref c) => {
                                    // Handle Ctrl combinations
                                    if e.modifiers().ctrl() {
                                        let ctrl_char = match c.to_lowercase().as_str() {
                                            "c" => Some(0x03), // ETX
                                            "d" => Some(0x04), // EOT
                                            "h" => Some(0x7f), // Backspace (Ctrl+H)
                                            "l" => Some(0x0c), // FF
                                            "z" => Some(0x1a), // SUB
                                            "a" => Some(0x01), // SOH (beginning of line)
                                            "e" => Some(0x05), // ENQ (end of line)
                                            "u" => Some(0x15), // NAK (clear line)
                                            "k" => Some(0x0b), // VT (kill to end)
                                            "w" => Some(0x17), // ETB (delete word)
                                            _ => None,
                                        };
                                        if let Some(ch) = ctrl_char
                                            && let Some(tx) = &stdin {
                                                let _ = tx.send(vec![ch]);
                                            }
                                        e.stop_propagation();
                                        e.prevent_default();
                                        return;
                                    }

                                    // Regular character input - send directly to stdin
                                    if !e.modifiers().meta() && !e.modifiers().alt()
                                        && let Some(tx) = &stdin {
                                            let _ = tx.send(c.as_bytes().to_vec());
                                        }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::Tab => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x09]);
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::ArrowUp => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x1b, 0x5b, 0x41]); // ESC [ A
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::ArrowDown => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x1b, 0x5b, 0x42]); // ESC [ B
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::ArrowLeft => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x1b, 0x5b, 0x44]); // ESC [ D
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::ArrowRight => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x1b, 0x5b, 0x43]); // ESC [ C
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::Home => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x1b, 0x5b, 0x48]); // ESC [ H
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::End => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x1b, 0x5b, 0x46]); // ESC [ F
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                Key::Delete => {
                                    if let Some(tx) = &stdin {
                                        let _ = tx.send(vec![0x1b, 0x5b, 0x33, 0x7e]); // ESC [ 3 ~
                                    }
                                    e.stop_propagation();
                                    e.prevent_default();
                                }
                                _ => {}
                            }
                        },
                        div {
                            for line in lines.iter() {
                                div { class: "term-line", "{line}" }
                            }
                        }
                    }
                }
            }

            // Footer
            div { class: "exec-footer",
                span { class: "line-count", "{line_count} lines" }
            }
        }
    }
}
