use std::io::{self, IsTerminal};

use terminal_size::{Width, terminal_size};

pub struct TerminalCapabilities {
    pub is_tty: bool,
    pub is_dumb: bool,
    pub width: Option<u16>,
    pub use_unicode: bool,
    pub use_colors: bool,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let is_tty = io::stdout().is_terminal();
        let is_dumb = is_dumb_terminal();
        let width = detect_terminal_width();

        let use_unicode = if !is_tty || is_dumb {
            false
        } else {
            is_unicode_supported()
        };

        let use_colors = supports_color(is_tty, is_dumb);

        Self {
            is_tty,
            is_dumb,
            width,
            use_unicode,
            use_colors,
        }
    }

    pub fn style(&self) -> Style {
        Style::new(self.use_colors)
    }
}

fn detect_terminal_width() -> Option<u16> {
    terminal_size().map(|(Width(width), _)| width)
}

fn is_unicode_supported() -> bool {
    if cfg!(windows) {
        return true;
    }

    std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_CTYPE"))
        .or_else(|_| std::env::var("LANG"))
        .map(|lang| {
            let lang = lang.to_lowercase();
            lang.contains("utf-8")
                || lang.contains("utf8")
                || lang.ends_with(".utf-8")
                || lang.ends_with(".utf8")
        })
        .unwrap_or(true)
}

fn is_dumb_terminal() -> bool {
    std::env::var("TERM")
        .map(|term| term.eq_ignore_ascii_case("dumb"))
        .unwrap_or(false)
}

fn supports_color(is_tty: bool, is_dumb: bool) -> bool {
    if std::env::var("CLICOLOR_FORCE")
        .map(|value| value != "0")
        .unwrap_or(false)
    {
        return true;
    }

    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }

    if !is_tty || is_dumb {
        return false;
    }

    if std::env::var("CLICOLOR")
        .map(|value| value == "0")
        .unwrap_or(false)
    {
        return false;
    }

    std::env::var("TERM")
        .map(|term| term != "dumb")
        .unwrap_or(true)
}

pub struct Style {
    pub use_colors: bool,
}

impl Style {
    fn new(use_colors: bool) -> Self {
        Self { use_colors }
    }

    pub fn plain() -> Self {
        Self { use_colors: false }
    }

    pub fn bold(&self, text: &str) -> String {
        if self.use_colors {
            format!("\x1b[1m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn green(&self, text: &str) -> String {
        if self.use_colors {
            format!("\x1b[32m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn red(&self, text: &str) -> String {
        if self.use_colors {
            format!("\x1b[31m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn yellow(&self, text: &str) -> String {
        if self.use_colors {
            format!("\x1b[33m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn cyan(&self, text: &str) -> String {
        if self.use_colors {
            format!("\x1b[36m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::supports_color;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_color_env(no_color: Option<&str>, clicolor_force: Option<&str>, test: impl FnOnce()) {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let previous_no_color = std::env::var_os("NO_COLOR");
        let previous_clicolor_force = std::env::var_os("CLICOLOR_FORCE");

        unsafe {
            match no_color {
                Some(value) => std::env::set_var("NO_COLOR", value),
                None => std::env::remove_var("NO_COLOR"),
            }

            match clicolor_force {
                Some(value) => std::env::set_var("CLICOLOR_FORCE", value),
                None => std::env::remove_var("CLICOLOR_FORCE"),
            }
        }

        test();

        unsafe {
            match previous_no_color {
                Some(value) => std::env::set_var("NO_COLOR", value),
                None => std::env::remove_var("NO_COLOR"),
            }

            match previous_clicolor_force {
                Some(value) => std::env::set_var("CLICOLOR_FORCE", value),
                None => std::env::remove_var("CLICOLOR_FORCE"),
            }
        }
    }

    #[test]
    fn clicolor_force_overrides_no_color() {
        with_color_env(Some("1"), Some("1"), || {
            assert!(supports_color(false, true));
        });
    }

    #[test]
    fn no_color_disables_color_without_force() {
        with_color_env(Some("1"), None, || {
            assert!(!supports_color(true, false));
        });
    }
}
