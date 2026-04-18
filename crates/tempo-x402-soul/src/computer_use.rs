//! Computer use: screenshot capture, mouse/keyboard simulation, screen understanding.
//!
//! Each node runs in a VM with display capabilities. This module provides:
//! - Screenshot capture via shell commands (scrot, import, or xdotool)
//! - Mouse click/move/drag simulation
//! - Keyboard typing and key combos
//! - Screen region description (sent to LLM for understanding)
//! - Action sequences for multi-step UI automation
//!
//! The vision understanding is handled by the LLM (Gemini's multimodal input).
//! This module handles the I/O: capturing what's on screen and sending inputs.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::SoulError;

// ── Types ────────────────────────────────────────────────────────────

/// A screenshot captured from the VM display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    /// Path to the saved image file.
    pub path: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Base64-encoded PNG data (for LLM input).
    pub base64_png: String,
    /// Timestamp of capture.
    pub captured_at: i64,
}

/// A point on the screen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// Mouse button.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard modifier.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

/// A computer action the agent can take.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ComputerAction {
    /// Take a screenshot of the full screen or a region.
    Screenshot {
        #[serde(default)]
        region: Option<ScreenRegion>,
    },
    /// Move the mouse to a position.
    MouseMove { point: Point },
    /// Click at a position.
    MouseClick {
        point: Point,
        #[serde(default = "default_left")]
        button: MouseButton,
        #[serde(default)]
        double: bool,
    },
    /// Drag from one point to another.
    MouseDrag { from: Point, to: Point },
    /// Type text.
    TypeText { text: String },
    /// Press a key combination.
    KeyPress {
        key: String,
        #[serde(default)]
        modifiers: Vec<Modifier>,
    },
    /// Scroll the mouse wheel.
    Scroll {
        point: Point,
        /// Positive = down, negative = up.
        amount: i32,
    },
    /// Wait for a duration (milliseconds).
    Wait { ms: u64 },
    /// Open a URL in the browser.
    OpenUrl { url: String },
    /// Run a shell command and capture output.
    Shell { command: String },
}

/// A rectangular region of the screen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScreenRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Result of executing a computer action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub action: String,
    /// Screenshot after action (if applicable).
    pub screenshot: Option<Screenshot>,
    /// Text output (for shell commands).
    pub output: Option<String>,
    /// Error message if failed.
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// An action sequence — multi-step UI automation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSequence {
    pub name: String,
    pub description: String,
    pub actions: Vec<ComputerAction>,
    /// Whether to take a screenshot after each action (for debugging).
    pub screenshot_each_step: bool,
}

fn default_left() -> MouseButton {
    MouseButton::Left
}

// ── Executor ─────────────────────────────────────────────────────────

/// Computer use executor — interfaces with the VM's display.
pub struct ComputerExecutor {
    /// Directory to store screenshots.
    screenshot_dir: PathBuf,
    /// Display identifier (e.g., ":0" or ":99" for Xvfb).
    display: String,
    /// Whether a display is available.
    display_available: bool,
}

impl ComputerExecutor {
    /// Create a new executor, auto-detecting display availability.
    pub fn new(screenshot_dir: PathBuf) -> Self {
        let display = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
        let display_available = std::env::var("DISPLAY").is_ok();

        if !display_available {
            tracing::info!("No DISPLAY set — computer use will use virtual framebuffer");
        }

        // Ensure screenshot directory exists
        let _ = std::fs::create_dir_all(&screenshot_dir);

        Self {
            screenshot_dir,
            display,
            display_available,
        }
    }

    /// Check if display is available (has DISPLAY env var).
    pub fn is_available(&self) -> bool {
        self.display_available
    }

    /// Execute a single computer action.
    pub async fn execute(&self, action: &ComputerAction) -> ActionResult {
        let start = std::time::Instant::now();

        let result = match action {
            ComputerAction::Screenshot { region } => self.take_screenshot(region.as_ref()).await,
            ComputerAction::MouseMove { point } => self.mouse_move(*point).await,
            ComputerAction::MouseClick {
                point,
                button,
                double,
            } => self.mouse_click(*point, *button, *double).await,
            ComputerAction::MouseDrag { from, to } => self.mouse_drag(*from, *to).await,
            ComputerAction::TypeText { text } => self.type_text(text).await,
            ComputerAction::KeyPress { key, modifiers } => self.key_press(key, modifiers).await,
            ComputerAction::Scroll { point, amount } => self.scroll(*point, *amount).await,
            ComputerAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(*ms)).await;
                Ok(ActionResult {
                    success: true,
                    action: "wait".into(),
                    screenshot: None,
                    output: None,
                    error: None,
                    duration_ms: *ms,
                })
            }
            ComputerAction::OpenUrl { url } => self.open_url(url).await,
            ComputerAction::Shell { command } => self.run_shell(command).await,
        };

        match result {
            Ok(mut r) => {
                r.duration_ms = start.elapsed().as_millis() as u64;
                r
            }
            Err(e) => ActionResult {
                success: false,
                action: action_name(action),
                screenshot: None,
                output: None,
                error: Some(e.to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        }
    }

    /// Execute an action sequence, optionally screenshotting each step.
    pub async fn execute_sequence(&self, sequence: &ActionSequence) -> Vec<ActionResult> {
        let mut results = Vec::new();

        for (i, action) in sequence.actions.iter().enumerate() {
            tracing::debug!(
                step = i,
                action = action_name(action),
                "Executing action sequence step"
            );

            let result = self.execute(action).await;
            let failed = !result.success;
            results.push(result);

            // Take screenshot after each step if requested
            if sequence.screenshot_each_step && !matches!(action, ComputerAction::Screenshot { .. })
            {
                let ss = self.take_screenshot(None).await;
                if let Ok(ss_result) = ss {
                    results.push(ss_result);
                }
            }

            // Stop on failure
            if failed {
                tracing::warn!(
                    step = i,
                    name = &sequence.name,
                    "Action sequence failed at step"
                );
                break;
            }
        }

        results
    }

    // ── Internal methods ─────────────────────────────────────────────

    async fn take_screenshot(
        &self,
        region: Option<&ScreenRegion>,
    ) -> Result<ActionResult, SoulError> {
        let filename = format!("screenshot_{}.png", chrono::Utc::now().timestamp_millis());
        let path = self.screenshot_dir.join(&filename);
        let path_str = path.to_string_lossy().to_string();

        // Build the capture command
        // Try scrot first (lightweight), fall back to import (ImageMagick), then xdotool+xwd
        let cmd = if let Some(r) = region {
            format!(
                "DISPLAY={} scrot -a {},{},{},{} '{}' 2>/dev/null || \
                 DISPLAY={} import -window root -crop {}x{}+{}+{} '{}' 2>/dev/null",
                self.display,
                r.x,
                r.y,
                r.width,
                r.height,
                path_str,
                self.display,
                r.width,
                r.height,
                r.x,
                r.y,
                path_str,
            )
        } else {
            format!(
                "DISPLAY={} scrot '{}' 2>/dev/null || \
                 DISPLAY={} import -window root '{}' 2>/dev/null",
                self.display, path_str, self.display, path_str,
            )
        };

        let output = run_command(&cmd).await?;

        // Read the file and base64 encode it
        let base64_png = if path.exists() {
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|e| SoulError::ToolError(format!("Failed to read screenshot: {e}")))?;

            use base64::Engine;
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        } else {
            return Err(SoulError::ToolError(format!(
                "Screenshot capture failed: {output}"
            )));
        };

        // Get image dimensions (from file, or default)
        let (width, height) = get_image_dimensions(&path).unwrap_or((1920, 1080));

        let screenshot = Screenshot {
            path: path_str,
            width,
            height,
            base64_png,
            captured_at: chrono::Utc::now().timestamp(),
        };

        Ok(ActionResult {
            success: true,
            action: "screenshot".into(),
            screenshot: Some(screenshot),
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn mouse_move(&self, point: Point) -> Result<ActionResult, SoulError> {
        let cmd = format!(
            "DISPLAY={} xdotool mousemove {} {}",
            self.display, point.x, point.y
        );
        run_command(&cmd).await?;
        Ok(ActionResult {
            success: true,
            action: "mouse_move".into(),
            screenshot: None,
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn mouse_click(
        &self,
        point: Point,
        button: MouseButton,
        double: bool,
    ) -> Result<ActionResult, SoulError> {
        let btn = match button {
            MouseButton::Left => "1",
            MouseButton::Right => "3",
            MouseButton::Middle => "2",
        };
        let repeat = if double { "--repeat 2 --delay 100" } else { "" };
        let cmd = format!(
            "DISPLAY={} xdotool mousemove {} {} click {repeat} {btn}",
            self.display, point.x, point.y
        );
        run_command(&cmd).await?;
        Ok(ActionResult {
            success: true,
            action: "mouse_click".into(),
            screenshot: None,
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn mouse_drag(&self, from: Point, to: Point) -> Result<ActionResult, SoulError> {
        let cmd = format!(
            "DISPLAY={} xdotool mousemove {} {} mousedown 1 mousemove {} {} mouseup 1",
            self.display, from.x, from.y, to.x, to.y
        );
        run_command(&cmd).await?;
        Ok(ActionResult {
            success: true,
            action: "mouse_drag".into(),
            screenshot: None,
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn type_text(&self, text: &str) -> Result<ActionResult, SoulError> {
        // xdotool type needs special handling for certain characters
        let cmd = format!(
            "DISPLAY={} xdotool type --clearmodifiers -- '{}'",
            self.display,
            text.replace('\'', "'\\''")
        );
        run_command(&cmd).await?;
        Ok(ActionResult {
            success: true,
            action: "type_text".into(),
            screenshot: None,
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn key_press(
        &self,
        key: &str,
        modifiers: &[Modifier],
    ) -> Result<ActionResult, SoulError> {
        let mut combo = String::new();
        for m in modifiers {
            let mod_str = match m {
                Modifier::Ctrl => "ctrl",
                Modifier::Alt => "alt",
                Modifier::Shift => "shift",
                Modifier::Super => "super",
            };
            combo.push_str(mod_str);
            combo.push('+');
        }
        combo.push_str(key);

        let cmd = format!("DISPLAY={} xdotool key {combo}", self.display);
        run_command(&cmd).await?;
        Ok(ActionResult {
            success: true,
            action: "key_press".into(),
            screenshot: None,
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn scroll(&self, point: Point, amount: i32) -> Result<ActionResult, SoulError> {
        let btn = if amount > 0 { "5" } else { "4" }; // 5 = down, 4 = up
        let clicks = amount.unsigned_abs();
        let cmd = format!(
            "DISPLAY={} xdotool mousemove {} {} click --repeat {} {}",
            self.display, point.x, point.y, clicks, btn
        );
        run_command(&cmd).await?;
        Ok(ActionResult {
            success: true,
            action: "scroll".into(),
            screenshot: None,
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn open_url(&self, url: &str) -> Result<ActionResult, SoulError> {
        // Validate URL to prevent command injection
        if url.contains('\'') || url.contains('"') || url.contains('`') || url.contains('$') {
            return Err(SoulError::ToolError("Invalid URL characters".into()));
        }
        let cmd = format!(
            "DISPLAY={} xdg-open '{}' 2>/dev/null || firefox '{}' 2>/dev/null || chromium '{}' 2>/dev/null",
            self.display, url, url, url
        );
        run_command(&cmd).await?;
        // Wait for browser to load
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        Ok(ActionResult {
            success: true,
            action: "open_url".into(),
            screenshot: None,
            output: None,
            error: None,
            duration_ms: 0,
        })
    }

    async fn run_shell(&self, command: &str) -> Result<ActionResult, SoulError> {
        let output = run_command(command).await?;
        Ok(ActionResult {
            success: true,
            action: "shell".into(),
            screenshot: None,
            output: Some(output),
            error: None,
            duration_ms: 0,
        })
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Run a shell command and return stdout.
async fn run_command(cmd: &str) -> Result<String, SoulError> {
    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .await
        .map_err(|e| SoulError::ToolError(format!("Command failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && stdout.is_empty() {
        return Err(SoulError::ToolError(format!(
            "Command failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.chars().take(500).collect::<String>()
        )));
    }

    Ok(stdout)
}

/// Get image dimensions from a PNG file (reads header bytes).
fn get_image_dimensions(path: &std::path::Path) -> Option<(u32, u32)> {
    let data = std::fs::read(path).ok()?;
    // PNG IHDR chunk: bytes 16-23 contain width (4 bytes BE) and height (4 bytes BE)
    if data.len() < 24 || &data[0..4] != b"\x89PNG" {
        return None;
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some((width, height))
}

/// Get action name for logging.
fn action_name(action: &ComputerAction) -> String {
    match action {
        ComputerAction::Screenshot { .. } => "screenshot".into(),
        ComputerAction::MouseMove { .. } => "mouse_move".into(),
        ComputerAction::MouseClick { .. } => "mouse_click".into(),
        ComputerAction::MouseDrag { .. } => "mouse_drag".into(),
        ComputerAction::TypeText { .. } => "type_text".into(),
        ComputerAction::KeyPress { .. } => "key_press".into(),
        ComputerAction::Scroll { .. } => "scroll".into(),
        ComputerAction::Wait { .. } => "wait".into(),
        ComputerAction::OpenUrl { .. } => "open_url".into(),
        ComputerAction::Shell { .. } => "shell".into(),
    }
}

// ── Plan step integration ────────────────────────────────────────────

/// Format a screenshot for LLM description.
/// Returns a prompt section that includes the base64 image for multimodal input.
pub fn screenshot_for_llm(screenshot: &Screenshot) -> String {
    format!(
        "Screenshot captured ({}x{} at {}).\n\
         Describe what you see on screen and identify interactive elements (buttons, text fields, links).\n\
         Image data is attached as base64 PNG.",
        screenshot.width, screenshot.height, screenshot.captured_at,
    )
}

/// Build common action sequences for typical tasks.
pub fn browse_url_sequence(url: &str) -> ActionSequence {
    ActionSequence {
        name: format!("Browse: {}", url.chars().take(50).collect::<String>()),
        description: format!("Open URL and capture page: {url}"),
        actions: vec![
            ComputerAction::OpenUrl {
                url: url.to_string(),
            },
            ComputerAction::Wait { ms: 3000 },
            ComputerAction::Screenshot { region: None },
        ],
        screenshot_each_step: false,
    }
}

/// Setup virtual framebuffer if no display is available.
/// Returns true if Xvfb was started successfully.
pub async fn ensure_display() -> bool {
    if std::env::var("DISPLAY").is_ok() {
        return true;
    }

    tracing::info!("No DISPLAY — starting Xvfb virtual framebuffer");

    // Start Xvfb on display :99
    let result = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("Xvfb :99 -screen 0 1920x1080x24 &")
        .spawn();

    match result {
        Ok(_) => {
            std::env::set_var("DISPLAY", ":99");
            // Wait for Xvfb to initialize
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            tracing::info!("Xvfb started on :99");
            true
        }
        Err(e) => {
            tracing::warn!("Failed to start Xvfb: {e}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_serialization() {
        let action = ComputerAction::MouseClick {
            point: Point { x: 100, y: 200 },
            button: MouseButton::Left,
            double: false,
        };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("mouse_click"));
        assert!(json.contains("100"));

        let parsed: ComputerAction = serde_json::from_str(&json).unwrap();
        match parsed {
            ComputerAction::MouseClick { point, .. } => {
                assert_eq!(point.x, 100);
                assert_eq!(point.y, 200);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_sequence_serialization() {
        let seq = browse_url_sequence("https://example.com");
        assert_eq!(seq.actions.len(), 3);
        let json = serde_json::to_string(&seq).unwrap();
        assert!(json.contains("example.com"));
    }

    #[test]
    fn test_png_dimensions() {
        // Create a minimal valid PNG header (1x1 pixel)
        let png_header: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
            0x49, 0x48, 0x44, 0x52, // IHDR
            0x00, 0x00, 0x00, 0x01, // width = 1
            0x00, 0x00, 0x00, 0x01, // height = 1
            0x08, 0x02, 0x00, 0x00, 0x00, // bit depth, color type, etc
        ];

        let dir = std::env::temp_dir();
        let path = dir.join("test_dims.png");
        std::fs::write(&path, &png_header).unwrap();

        let dims = get_image_dimensions(&path);
        assert_eq!(dims, Some((1, 1)));

        std::fs::remove_file(&path).ok();
    }
}
