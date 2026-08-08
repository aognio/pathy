use std::time::{Duration, SystemTime};

use clap::ValueEnum;

use crate::path::{EntryType, PathEntry, format_modified, format_size};
use crate::terminal::{Style, TerminalCapabilities};

const UNICODE_COLUMNS: [ColumnSpec; 9] = [
    ColumnSpec::new("#", Alignment::Right),
    ColumnSpec::new("OK", Alignment::Right),
    ColumnSpec::new("PATH", Alignment::Left),
    ColumnSpec::new("TYPE", Alignment::Center),
    ColumnSpec::new("BIN", Alignment::Right),
    ColumnSpec::new("!BIN", Alignment::Right),
    ColumnSpec::new("SIZE", Alignment::Right),
    ColumnSpec::new("MODIFIED", Alignment::Right),
    ColumnSpec::new("DUP", Alignment::Left),
];
const MIN_PATH_WIDTH: usize = 16;
const MAX_UNICODE_TABLE_WIDTH: usize = 120;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AsciiFormat {
    Compact,
    Narrow,
    Minimal,
}

pub struct DisplayConfig {
    pub style: Style,
    pub use_unicode: bool,
    pub ascii_format: AsciiFormat,
    pub terminal_width: Option<usize>,
    pub long_paths: bool,
    pub folder_wrap: bool,
}

impl DisplayConfig {
    pub fn new(
        ascii_format: Option<AsciiFormat>,
        monochrome: bool,
        long_paths: bool,
        folder_wrap: bool,
    ) -> Self {
        let caps = TerminalCapabilities::detect();

        Self::from_capabilities(caps, ascii_format, monochrome, long_paths, folder_wrap)
    }

    fn from_capabilities(
        caps: TerminalCapabilities,
        ascii_format: Option<AsciiFormat>,
        monochrome: bool,
        long_paths: bool,
        folder_wrap: bool,
    ) -> Self {
        let force_ascii = ascii_format.is_some();
        let ascii_format = ascii_format.unwrap_or_else(|| auto_ascii_format(&caps));
        let use_unicode = caps.use_unicode
            && !force_ascii
            && caps.width.map(|width| width >= 100).unwrap_or(true);
        let style = if monochrome || matches!(ascii_format, AsciiFormat::Minimal) {
            Style::plain()
        } else if use_unicode || !matches!(ascii_format, AsciiFormat::Minimal) {
            caps.style()
        } else {
            Style::plain()
        };

        Self {
            style,
            use_unicode,
            ascii_format,
            terminal_width: caps.width.map(usize::from),
            long_paths,
            folder_wrap,
        }
    }

    pub fn should_print_summary(&self) -> bool {
        self.use_unicode || !matches!(self.ascii_format, AsciiFormat::Minimal)
    }

    #[cfg(test)]
    fn unicode() -> Self {
        Self {
            style: Style::plain(),
            use_unicode: true,
            ascii_format: AsciiFormat::Compact,
            terminal_width: Some(120),
            long_paths: false,
            folder_wrap: false,
        }
    }

    #[cfg(test)]
    fn ascii(ascii_format: AsciiFormat) -> Self {
        Self {
            style: Style::plain(),
            use_unicode: false,
            ascii_format,
            terminal_width: Some(120),
            long_paths: false,
            folder_wrap: false,
        }
    }
}

fn auto_ascii_format(caps: &TerminalCapabilities) -> AsciiFormat {
    if !caps.is_tty || caps.is_dumb {
        return AsciiFormat::Minimal;
    }

    match caps.width {
        Some(width) if width < 60 => AsciiFormat::Minimal,
        Some(width) if width < 100 => AsciiFormat::Narrow,
        _ => AsciiFormat::Compact,
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self::new(None, false, false, false)
    }
}

pub fn print_table(entries: &[PathEntry], config: &DisplayConfig) {
    let output = render_table(entries, config);

    if !output.is_empty() {
        println!("{output}");
    }
}

pub fn render_table(entries: &[PathEntry], config: &DisplayConfig) -> String {
    if config.use_unicode {
        return render_unicode_table(entries, config);
    }

    let rows = collect_display_rows(entries, SystemTime::now());

    match config.ascii_format {
        AsciiFormat::Compact => render_ascii_compact(&rows, config),
        AsciiFormat::Narrow => render_ascii_narrow(&rows, config),
        AsciiFormat::Minimal => render_ascii_minimal(&rows, config),
    }
}

fn render_unicode_table(entries: &[PathEntry], config: &DisplayConfig) -> String {
    let rows = collect_unicode_rows(entries);
    let widths = unicode_column_widths(
        &rows,
        config.terminal_width,
        config.long_paths,
        config.folder_wrap,
    );
    let rows = rows
        .into_iter()
        .map(|mut row| {
            if !config.long_paths && !config.folder_wrap {
                row.path = truncate_to_width(&row.path, widths[2]);
            }
            row
        })
        .collect::<Vec<_>>();
    let mut output = String::new();

    push_unicode_rule(&mut output, &widths, '╭', '┬', '╮');
    push_unicode_row(
        &mut output,
        &unicode_headers(),
        &widths,
        &config.style,
        RowStyle::Header,
    );
    push_unicode_rule(&mut output, &widths, '├', '┼', '┤');

    for row in rows {
        if config.folder_wrap {
            push_wrapped_unicode_row(&mut output, &row, &widths, &config.style);
        } else {
            push_unicode_row(
                &mut output,
                &row.cells(),
                &widths,
                &config.style,
                RowStyle::Data {
                    problem: row.problem,
                },
            );
        }
    }

    push_unicode_rule(&mut output, &widths, '╰', '┴', '╯');
    trim_final_newline(output)
}

#[derive(Debug, Eq, PartialEq)]
struct UnicodeRow {
    index: String,
    ok: String,
    path: String,
    entry_type: String,
    binaries: String,
    non_binary: String,
    size: String,
    modified: String,
    duplicate: String,
    problem: bool,
}

impl UnicodeRow {
    fn cells(&self) -> [&str; 9] {
        [
            &self.index,
            &self.ok,
            &self.path,
            &self.entry_type,
            &self.binaries,
            &self.non_binary,
            &self.size,
            &self.modified,
            &self.duplicate,
        ]
    }
}

fn collect_unicode_rows(entries: &[PathEntry]) -> Vec<UnicodeRow> {
    entries
        .iter()
        .map(|entry| UnicodeRow {
            index: entry.index.to_string(),
            ok: if entry.exists { "Y" } else { "N" }.to_string(),
            path: entry.path.display().to_string(),
            entry_type: entry.entry_type.label().to_string(),
            binaries: entry.executable_count.to_string(),
            non_binary: entry.non_executable_count.to_string(),
            size: format_size(entry.size_bytes),
            modified: format_modified(entry.modified),
            duplicate: if entry.duplicate { "YES" } else { "" }.to_string(),
            problem: row_has_problem(entry),
        })
        .collect()
}

fn unicode_column_widths(
    rows: &[UnicodeRow],
    terminal_width: Option<usize>,
    long_paths: bool,
    folder_wrap: bool,
) -> [usize; 9] {
    let mut widths = std::array::from_fn(|index| UNICODE_COLUMNS[index].header.chars().count());

    for row in rows {
        for (index, cell) in row.cells().iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    if !long_paths || folder_wrap {
        let target_width = terminal_width.unwrap_or(MAX_UNICODE_TABLE_WIDTH);
        let table_width = unicode_table_width(&widths);

        if table_width > target_width && widths[2] > MIN_PATH_WIDTH {
            let excess = table_width - target_width;
            widths[2] = widths[2].saturating_sub(excess).max(MIN_PATH_WIDTH);
        }
    }

    widths
}

fn unicode_table_width(widths: &[usize; 9]) -> usize {
    widths.iter().sum::<usize>() + (widths.len() * 3) + 1
}

fn push_unicode_rule(
    output: &mut String,
    widths: &[usize; 9],
    left: char,
    middle: char,
    right: char,
) {
    output.push(left);

    for (index, width) in widths.iter().enumerate() {
        output.push_str(&"─".repeat(width + 2));
        output.push(if index == widths.len() - 1 {
            right
        } else {
            middle
        });
    }

    output.push('\n');
}

fn push_unicode_row(
    output: &mut String,
    cells: &[&str; 9],
    widths: &[usize; 9],
    style: &Style,
    row_style: RowStyle,
) {
    output.push('│');

    for (index, cell) in cells.iter().enumerate() {
        output.push(' ');
        let cell = aligned_cell(cell, widths[index], UNICODE_COLUMNS[index].alignment);
        output.push_str(&style_unicode_cell(&cell, index, style, row_style));
        output.push(' ');
        output.push('│');
    }

    output.push('\n');
}

fn push_wrapped_unicode_row(
    output: &mut String,
    row: &UnicodeRow,
    widths: &[usize; 9],
    style: &Style,
) {
    let path_lines = wrap_path_by_folder(&row.path, widths[2]);
    let row_style = RowStyle::Data {
        problem: row.problem,
    };

    for (line_index, path_line) in path_lines.iter().enumerate() {
        let is_last_line = line_index == path_lines.len() - 1;
        let cells = if is_last_line {
            [
                row.index.as_str(),
                row.ok.as_str(),
                path_line.as_str(),
                row.entry_type.as_str(),
                row.binaries.as_str(),
                row.non_binary.as_str(),
                row.size.as_str(),
                row.modified.as_str(),
                row.duplicate.as_str(),
            ]
        } else {
            ["", "", path_line.as_str(), "", "", "", "", "", ""]
        };

        push_unicode_row(output, &cells, widths, style, row_style);
    }
}

#[derive(Clone, Copy)]
enum RowStyle {
    Header,
    Data { problem: bool },
}

fn style_unicode_cell(cell: &str, index: usize, style: &Style, row_style: RowStyle) -> String {
    if cell.trim().is_empty() {
        return cell.to_string();
    }

    match row_style {
        RowStyle::Header => style.cyan(cell),
        RowStyle::Data { problem: _ } if index == 0 => style.yellow(cell),
        RowStyle::Data { problem: true } => style.red(cell),
        RowStyle::Data { problem: false } => style.green(cell),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ColumnSpec {
    header: &'static str,
    alignment: Alignment,
}

impl ColumnSpec {
    const fn new(header: &'static str, alignment: Alignment) -> Self {
        Self { header, alignment }
    }
}

fn unicode_headers() -> [&'static str; 9] {
    std::array::from_fn(|index| UNICODE_COLUMNS[index].header)
}

fn aligned_cell(cell: &str, width: usize, alignment: Alignment) -> String {
    let visible_width = cell.chars().count();
    let padding = width.saturating_sub(visible_width);

    let (left_padding, right_padding) = match alignment {
        Alignment::Left => (0, padding),
        Alignment::Center => (padding / 2, padding - padding / 2),
        Alignment::Right => (padding, 0),
    };

    format!(
        "{}{}{}",
        " ".repeat(left_padding),
        cell,
        " ".repeat(right_padding)
    )
}

fn truncate_to_width(value: &str, width: usize) -> String {
    let length = value.chars().count();

    if length <= width {
        return value.to_string();
    }

    if width <= 1 {
        return "…".to_string();
    }

    let kept = width - 1;
    let mut truncated = value.chars().take(kept).collect::<String>();
    truncated.push('…');
    truncated
}

fn wrap_path_by_folder(path: &str, width: usize) -> Vec<String> {
    if path.is_empty() {
        return vec![String::new()];
    }

    if width == 0 {
        return vec![path.to_string()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for component in folder_components(path) {
        let candidate_len = current.chars().count() + component.chars().count();

        if !current.is_empty() && candidate_len > width {
            lines.push(current);
            current = component;
        } else {
            current.push_str(&component);
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(path.to_string());
    }

    lines
}

fn folder_components(path: &str) -> Vec<String> {
    if path == "/" {
        return vec!["/".to_string()];
    }

    let mut components = Vec::new();

    if path.starts_with('/') {
        components.push("/".to_string());
    }

    let mut parts = path.split('/').filter(|part| !part.is_empty()).peekable();

    while let Some(part) = parts.next() {
        let mut component = part.to_string();

        if parts.peek().is_some() || path.ends_with('/') {
            component.push('/');
        }

        components.push(component);
    }

    if components.is_empty() {
        components.push(path.to_string());
    }

    components
}

#[derive(Debug, Eq, PartialEq)]
struct DisplayRow {
    index: String,
    name: String,
    size: String,
    minimal_size: String,
    modified: String,
    problem: bool,
}

fn collect_display_rows(entries: &[PathEntry], now: SystemTime) -> Vec<DisplayRow> {
    entries
        .iter()
        .map(|entry| DisplayRow {
            index: entry.index.to_string(),
            name: format_ascii_name(entry),
            size: format_ascii_size(entry.size_bytes, true),
            minimal_size: format_ascii_size(entry.size_bytes, false),
            modified: format_ascii_modified(entry.modified, now),
            problem: row_has_problem(entry),
        })
        .collect()
}

fn row_has_problem(entry: &PathEntry) -> bool {
    !entry.exists
        || entry.duplicate
        || matches!(entry.entry_type, EntryType::Directory) && entry.executable_count == 0
}

fn format_ascii_name(entry: &PathEntry) -> String {
    let mut name = entry.path.display().to_string();

    if matches!(entry.entry_type, EntryType::Directory) && !name.ends_with('/') {
        name.push('/');
    }

    name
}

fn format_ascii_size(bytes: u64, include_unit_space: bool) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];

    let mut value = bytes as f64;
    let mut unit_index = 0;

    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    let value = if unit_index == 0 {
        bytes.to_string()
    } else {
        format!("{value:.1}")
    };

    if include_unit_space {
        format!("{value} {}", UNITS[unit_index])
    } else {
        format!("{value}{}", UNITS[unit_index])
    }
}

fn format_ascii_modified(modified: Option<SystemTime>, now: SystemTime) -> String {
    let Some(modified) = modified else {
        return "-".to_string();
    };

    match now.duration_since(modified) {
        Ok(age) => format_ascii_age(age, "ago"),
        Err(error) => format_ascii_age(error.duration(), "from now"),
    }
}

fn format_ascii_age(age: Duration, suffix: &str) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const MONTH: u64 = 30 * DAY;
    const YEAR: u64 = 365 * DAY;

    let seconds = age.as_secs();

    if seconds < MINUTE {
        return "now".to_string();
    }

    let (value, unit) = if seconds < HOUR {
        (seconds / MINUTE, "min")
    } else if seconds < DAY {
        (seconds / HOUR, "hour")
    } else if seconds < MONTH {
        (seconds / DAY, "day")
    } else if seconds < YEAR {
        (seconds / MONTH, "month")
    } else {
        (seconds / YEAR, "year")
    };

    let plural = if value == 1 || unit == "min" { "" } else { "s" };

    format!("{value} {unit}{plural} {suffix}")
}

fn render_ascii_compact(rows: &[DisplayRow], config: &DisplayConfig) -> String {
    let mut widths = [
        column_width(" #", rows.iter().map(|row| row.index.as_str())),
        column_width("Name", rows.iter().map(|row| row.name.as_str())),
        column_width("Size", rows.iter().map(|row| row.size.as_str())),
        column_width("Modified", rows.iter().map(|row| row.modified.as_str())),
    ];

    if config.folder_wrap {
        widths[1] = capped_ascii_name_width(
            config.terminal_width,
            widths[1],
            &[widths[0], widths[2], widths[3]],
            13,
        );
    }

    let mut output = String::new();

    push_compact_rule(&mut output, &widths);
    push_compact_row(
        &mut output,
        &["#", "Name", "Size", "Modified"],
        &widths,
        &config.style,
        RowStyle::Header,
    );
    push_compact_rule(&mut output, &widths);

    for row in rows {
        if config.folder_wrap {
            push_wrapped_compact_row(&mut output, row, &widths, &config.style);
        } else {
            push_compact_row(
                &mut output,
                &[&row.index, &row.name, &row.size, &row.modified],
                &widths,
                &config.style,
                RowStyle::Data {
                    problem: row.problem,
                },
            );
        }
    }

    push_compact_rule(&mut output, &widths);
    trim_final_newline(output)
}

fn capped_ascii_name_width(
    terminal_width: Option<usize>,
    current_name_width: usize,
    other_widths: &[usize],
    fixed_width: usize,
) -> usize {
    let target_width = terminal_width.unwrap_or(MAX_UNICODE_TABLE_WIDTH);
    let other_width = other_widths.iter().sum::<usize>() + fixed_width;

    if target_width <= other_width + MIN_PATH_WIDTH {
        return MIN_PATH_WIDTH.min(current_name_width.max(MIN_PATH_WIDTH));
    }

    current_name_width
        .min(target_width - other_width)
        .max(MIN_PATH_WIDTH)
}

fn push_compact_rule(output: &mut String, widths: &[usize; 4]) {
    output.push('+');

    for width in widths {
        output.push_str(&"-".repeat(width + 2));
        output.push('+');
    }

    output.push('\n');
}

fn push_compact_row(
    output: &mut String,
    cells: &[&str; 4],
    widths: &[usize; 4],
    style: &Style,
    row_style: RowStyle,
) {
    output.push('|');

    let index = aligned_cell(cells[0], widths[0], Alignment::Right);
    let name = aligned_cell(cells[1], widths[1], Alignment::Left);
    let size = aligned_cell(cells[2], widths[2], Alignment::Left);
    let modified = aligned_cell(cells[3], widths[3], Alignment::Left);

    for (cell_index, cell) in [index, name, size, modified].iter().enumerate() {
        output.push(' ');
        output.push_str(&style_unicode_cell(cell, cell_index, style, row_style));
        output.push_str(" |");
    }

    output.push('\n');
}

fn push_wrapped_compact_row(
    output: &mut String,
    row: &DisplayRow,
    widths: &[usize; 4],
    style: &Style,
) {
    let name_lines = wrap_path_by_folder(&row.name, widths[1]);
    let row_style = RowStyle::Data {
        problem: row.problem,
    };

    for (line_index, name_line) in name_lines.iter().enumerate() {
        let is_last_line = line_index == name_lines.len() - 1;
        let cells = if is_last_line {
            [
                row.index.as_str(),
                name_line.as_str(),
                row.size.as_str(),
                row.modified.as_str(),
            ]
        } else {
            ["", name_line.as_str(), "", ""]
        };

        push_compact_row(output, &cells, widths, style, row_style);
    }
}

fn render_ascii_narrow(rows: &[DisplayRow], config: &DisplayConfig) -> String {
    let mut widths = [
        column_width("#", rows.iter().map(|row| row.index.as_str())),
        column_width("Name", rows.iter().map(|row| row.name.as_str())),
        column_width("Size", rows.iter().map(|row| row.size.as_str())),
    ];

    if config.folder_wrap {
        widths[1] =
            capped_ascii_name_width(config.terminal_width, widths[1], &[widths[0], widths[2]], 6);
    }

    let mut output = String::new();

    let header = [
        aligned_cell("#", widths[0], Alignment::Right),
        aligned_cell("Name", widths[1], Alignment::Left),
        aligned_cell("Size", widths[2], Alignment::Right),
    ];
    output.push_str(&format!(
        "{}   {}   {}\n",
        config.style.cyan(&header[0]),
        config.style.cyan(&header[1]),
        config.style.cyan(&header[2])
    ));
    output.push_str(&format!(
        "{}   {}   {}\n",
        "-".repeat(widths[0]),
        "-".repeat(widths[1]),
        "-".repeat(widths[2])
    ));

    for row in rows {
        if config.folder_wrap {
            push_wrapped_narrow_row(&mut output, row, &widths, &config.style);
        } else {
            push_narrow_row(&mut output, row, &row.name, &widths, &config.style, true);
        }
    }

    trim_final_newline(output)
}

fn push_narrow_row(
    output: &mut String,
    row: &DisplayRow,
    name: &str,
    widths: &[usize; 3],
    style: &Style,
    include_metadata: bool,
) {
    let index_value = if include_metadata {
        row.index.as_str()
    } else {
        ""
    };
    let size_value = if include_metadata {
        row.size.as_str()
    } else {
        ""
    };
    let index = aligned_cell(index_value, widths[0], Alignment::Right);
    let name = aligned_cell(name, widths[1], Alignment::Left);
    let size = aligned_cell(size_value, widths[2], Alignment::Right);
    let row_style = RowStyle::Data {
        problem: row.problem,
    };

    output.push_str(&format!(
        "{}   {}   {}\n",
        style_unicode_cell(&index, 0, style, row_style),
        style_unicode_cell(&name, 1, style, row_style),
        style_unicode_cell(&size, 2, style, row_style)
    ));
}

fn push_wrapped_narrow_row(
    output: &mut String,
    row: &DisplayRow,
    widths: &[usize; 3],
    style: &Style,
) {
    let name_lines = wrap_path_by_folder(&row.name, widths[1]);

    for (line_index, name_line) in name_lines.iter().enumerate() {
        let is_last_line = line_index == name_lines.len() - 1;
        push_narrow_row(output, row, name_line, widths, style, is_last_line);
    }
}

fn render_ascii_minimal(rows: &[DisplayRow], config: &DisplayConfig) -> String {
    let index_width = rows
        .iter()
        .map(|row| row.index.len())
        .max()
        .unwrap_or(1)
        .max("#".len());
    let mut name_width = rows
        .iter()
        .map(|row| row.name.len())
        .max()
        .unwrap_or("Name".len())
        .max("Name".len());
    let size_width = rows
        .iter()
        .map(|row| row.minimal_size.len())
        .max()
        .unwrap_or("Size".len())
        .max("Size".len());

    if config.folder_wrap {
        name_width = capped_ascii_name_width(
            config.terminal_width,
            name_width,
            &[index_width, size_width],
            4,
        );
    }

    let mut output = String::new();
    output.push_str(&format!(
        "{:<index_width$}  {:<name_width$}  {:>size_width$}\n",
        "#", "Name", "Size",
    ));

    for row in rows {
        if config.folder_wrap {
            let name_lines = wrap_path_by_folder(&row.name, name_width);

            for (line_index, name_line) in name_lines.iter().enumerate() {
                let is_last_line = line_index == name_lines.len() - 1;
                let index = if is_last_line { row.index.as_str() } else { "" };
                let size = if is_last_line {
                    row.minimal_size.as_str()
                } else {
                    ""
                };
                output.push_str(&format!(
                    "{:<index_width$}  {:<name_width$}  {:>size_width$}\n",
                    index, name_line, size,
                ));
            }
        } else {
            output.push_str(&format!(
                "{:<index_width$}  {:<name_width$}  {:>size_width$}\n",
                row.index, row.name, row.minimal_size,
            ));
        }
    }

    trim_final_newline(output)
}

fn column_width<'a>(header: &str, values: impl Iterator<Item = &'a str>) -> usize {
    values.map(str::len).max().unwrap_or(0).max(header.len())
}

fn trim_final_newline(mut output: String) -> String {
    output.pop();
    output
}

pub fn print_summary(entries: &[PathEntry], config: &DisplayConfig) {
    let existing = entries.iter().filter(|entry| entry.exists).count();

    let missing = entries.len() - existing;

    let duplicates = entries.iter().filter(|entry| entry.duplicate).count();

    let binaries: usize = entries.iter().map(|entry| entry.executable_count).sum();

    let total_size: u64 = entries.iter().map(|entry| entry.size_bytes).sum();

    println!();
    println!(
        "{} PATH entries | {} existing | {} missing | {} duplicates",
        config.style.bold(&entries.len().to_string()),
        config.style.green(&existing.to_string()),
        config.style.red(&missing.to_string()),
        if duplicates > 0 {
            config.style.yellow(&duplicates.to_string())
        } else {
            duplicates.to_string()
        }
    );

    println!(
        "{} executable files | {} total",
        config.style.cyan(&binaries.to_string()),
        format_size(total_size)
    );
}

#[cfg(test)]
mod tests {
    use super::{
        AsciiFormat, DisplayConfig, collect_display_rows, render_ascii_compact,
        render_ascii_minimal, render_ascii_narrow, render_table,
    };
    use crate::path::{EntryType, PathEntry};
    use crate::terminal::TerminalCapabilities;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn entry(index: usize, name: &str, entry_type: EntryType, size_bytes: u64) -> PathEntry {
        let now = fixture_now();

        PathEntry {
            index,
            path: PathBuf::from(name),
            entry_type,
            exists: !matches!(entry_type, EntryType::Missing),
            executable_count: 0,
            non_executable_count: 2,
            size_bytes,
            duplicate: false,
            modified: Some(now - Duration::from_secs(7 * 60)),
        }
    }

    fn fixture_entries() -> Vec<PathEntry> {
        vec![
            entry(0, "AGENTS.md", EntryType::File, 764),
            entry(1, "CHANGELOG.md", EntryType::File, 1024),
            entry(2, "assets", EntryType::Directory, 96),
            entry(
                10,
                "a-very-long-file-name-that-must-not-break.txt",
                EntryType::File,
                63_488,
            ),
        ]
    }

    fn fixture_now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)
    }

    fn ascii_config(ascii_format: AsciiFormat) -> DisplayConfig {
        DisplayConfig {
            style: crate::terminal::Style::plain(),
            use_unicode: false,
            ascii_format,
            terminal_width: Some(120),
            long_paths: false,
            folder_wrap: false,
        }
    }

    fn wrapped_ascii_config(ascii_format: AsciiFormat, terminal_width: usize) -> DisplayConfig {
        DisplayConfig {
            style: crate::terminal::Style::plain(),
            use_unicode: false,
            ascii_format,
            terminal_width: Some(terminal_width),
            long_paths: false,
            folder_wrap: true,
        }
    }

    #[test]
    fn ascii_format_forces_ascii_rendering() {
        let config = DisplayConfig::from_capabilities(
            TerminalCapabilities {
                is_tty: true,
                is_dumb: false,
                width: Some(120),
                use_unicode: true,
                use_colors: false,
            },
            Some(AsciiFormat::Narrow),
            false,
            false,
            false,
        );

        let output = render_table(&fixture_entries(), &config);

        assert!(!config.use_unicode);
        assert!(output.starts_with(" #   Name"));
        assert!(!output.contains('╭'));
    }

    #[test]
    fn no_ascii_format_preserves_detected_unicode() {
        let config = DisplayConfig::from_capabilities(
            TerminalCapabilities {
                is_tty: true,
                is_dumb: false,
                width: Some(120),
                use_unicode: true,
                use_colors: false,
            },
            None,
            false,
            false,
            false,
        );

        assert!(config.use_unicode);
    }

    #[test]
    fn monochrome_disables_color_without_changing_layout_selection() {
        let config = DisplayConfig::from_capabilities(
            TerminalCapabilities {
                is_tty: true,
                is_dumb: false,
                width: Some(120),
                use_unicode: true,
                use_colors: true,
            },
            None,
            true,
            false,
            false,
        );

        assert!(config.use_unicode);
        assert!(!config.style.use_colors);
    }

    #[test]
    fn auto_ascii_format_uses_compact_for_wide_terminals() {
        let config = DisplayConfig::from_capabilities(
            TerminalCapabilities {
                is_tty: true,
                is_dumb: false,
                width: Some(120),
                use_unicode: false,
                use_colors: false,
            },
            None,
            false,
            false,
            false,
        );

        assert_eq!(config.ascii_format, AsciiFormat::Compact);
    }

    #[test]
    fn auto_ascii_format_uses_narrow_for_constrained_terminals() {
        let config = DisplayConfig::from_capabilities(
            TerminalCapabilities {
                is_tty: true,
                is_dumb: false,
                width: Some(80),
                use_unicode: false,
                use_colors: false,
            },
            None,
            false,
            false,
            false,
        );

        assert_eq!(config.ascii_format, AsciiFormat::Narrow);
    }

    #[test]
    fn auto_ascii_format_uses_minimal_for_dumb_or_non_tty_output() {
        for caps in [
            TerminalCapabilities {
                is_tty: true,
                is_dumb: true,
                width: Some(120),
                use_unicode: false,
                use_colors: false,
            },
            TerminalCapabilities {
                is_tty: false,
                is_dumb: false,
                width: None,
                use_unicode: false,
                use_colors: false,
            },
        ] {
            let config = DisplayConfig::from_capabilities(caps, None, false, false, false);

            assert_eq!(config.ascii_format, AsciiFormat::Minimal);
        }
    }

    #[test]
    fn rows_append_directory_suffix_and_preserve_files() {
        let rows = collect_display_rows(&fixture_entries(), fixture_now());

        assert_eq!(rows[0].name, "AGENTS.md");
        assert_eq!(rows[2].name, "assets/");
    }

    #[test]
    fn compact_has_expected_columns_and_exact_output() {
        let rows = collect_display_rows(&fixture_entries(), fixture_now());
        let output = render_ascii_compact(&rows, &ascii_config(AsciiFormat::Compact));

        assert!(output.contains("|  # | Name"));
        assert!(output.contains("| Size    | Modified  |"));
        assert!(!output.contains("TYPE"));
        assert_eq!(
            output,
            "\
+----+-----------------------------------------------+---------+-----------+
|  # | Name                                          | Size    | Modified  |
+----+-----------------------------------------------+---------+-----------+
|  0 | AGENTS.md                                     | 764 B   | 7 min ago |
|  1 | CHANGELOG.md                                  | 1.0 kB  | 7 min ago |
|  2 | assets/                                       | 96 B    | 7 min ago |
| 10 | a-very-long-file-name-that-must-not-break.txt | 62.0 kB | 7 min ago |
+----+-----------------------------------------------+---------+-----------+"
        );
    }

    #[test]
    fn narrow_has_expected_columns_and_exact_output() {
        let rows = collect_display_rows(&fixture_entries(), fixture_now());
        let output = render_ascii_narrow(&rows, &ascii_config(AsciiFormat::Narrow));

        assert!(output.starts_with(" #   Name"));
        assert!(!output.contains("Modified"));
        assert!(!output.contains('|'));
        assert_eq!(
            output,
            concat!(
                " #   Name                                               Size\n",
                "--   ---------------------------------------------   -------\n",
                " 0   AGENTS.md                                         764 B\n",
                " 1   CHANGELOG.md                                     1.0 kB\n",
                " 2   assets/                                            96 B\n",
                "10   a-very-long-file-name-that-must-not-break.txt   62.0 kB"
            )
        );
    }

    #[test]
    fn minimal_has_headers_but_no_borders_and_exact_output() {
        let rows = collect_display_rows(&fixture_entries(), fixture_now());
        let output = render_ascii_minimal(&rows, &ascii_config(AsciiFormat::Minimal));

        assert!(!output.contains('|'));
        assert!(!output.contains('+'));
        assert_eq!(
            output,
            "#   Name                                             Size\n\
0   AGENTS.md                                        764B\n\
1   CHANGELOG.md                                    1.0kB\n\
2   assets/                                           96B\n\
10  a-very-long-file-name-that-must-not-break.txt  62.0kB"
        );
    }

    #[test]
    fn empty_results_are_rendered_gracefully() {
        let rows = collect_display_rows(&[], fixture_now());

        assert_eq!(
            render_ascii_compact(&rows, &ascii_config(AsciiFormat::Compact)),
            "\
+----+------+------+----------+
|  # | Name | Size | Modified |
+----+------+------+----------+
+----+------+------+----------+"
        );
        assert_eq!(
            render_ascii_narrow(&rows, &ascii_config(AsciiFormat::Narrow)),
            "\
#   Name   Size
-   ----   ----"
        );
        assert_eq!(
            render_ascii_minimal(&rows, &ascii_config(AsciiFormat::Minimal)),
            "#  Name  Size"
        );
    }

    #[test]
    fn preserves_existing_unicode_renderer() {
        let output = render_table(&fixture_entries(), &DisplayConfig::unicode());

        assert!(output.starts_with('╭'));
        let header = output.lines().nth(1).unwrap();
        let path_position = header.find("PATH").unwrap();
        let type_position = header.find("TYPE").unwrap();

        assert!(path_position < type_position);
        assert!(output.contains("MODIFIED"));
    }

    #[test]
    fn unicode_renderer_caps_long_paths_to_target_width() {
        let entries = vec![entry(
            123,
            "/this/is/a/very/long/path/that/would/otherwise/stretch/the/table/far/past/a/readable/terminal/width/bin",
            EntryType::Directory,
            1024 * 1024,
        )];
        let config = DisplayConfig {
            style: crate::terminal::Style::plain(),
            use_unicode: true,
            ascii_format: AsciiFormat::Compact,
            terminal_width: Some(120),
            long_paths: false,
            folder_wrap: false,
        };
        let output = render_table(&entries, &config);

        assert!(output.contains('…'));
        assert!(output.lines().all(|line| line.chars().count() <= 120));
    }

    #[test]
    fn long_paths_disable_unicode_path_truncation() {
        let long_path = "/this/is/a/very/long/path/that/would/otherwise/stretch/the/table/far/past/a/readable/terminal/width/bin";
        let entries = vec![entry(123, long_path, EntryType::Directory, 1024 * 1024)];
        let config = DisplayConfig {
            style: crate::terminal::Style::plain(),
            use_unicode: true,
            ascii_format: AsciiFormat::Compact,
            terminal_width: Some(120),
            long_paths: true,
            folder_wrap: false,
        };
        let output = render_table(&entries, &config);

        assert!(output.contains(long_path));
        assert!(!output.contains('…'));
    }

    #[test]
    fn wide_detected_terminal_avoids_default_truncation() {
        let long_path = "/this/is/a/very/long/path/that/would/otherwise/stretch/the/table/far/past/a/readable/terminal/width/bin";
        let entries = vec![entry(123, long_path, EntryType::Directory, 1024 * 1024)];
        let config = DisplayConfig {
            style: crate::terminal::Style::plain(),
            use_unicode: true,
            ascii_format: AsciiFormat::Compact,
            terminal_width: Some(240),
            long_paths: false,
            folder_wrap: false,
        };
        let output = render_table(&entries, &config);

        assert!(output.contains(long_path));
        assert!(!output.contains('…'));
    }

    #[test]
    fn folder_wrap_splits_paths_at_folder_boundaries() {
        assert_eq!(
            super::wrap_path_by_folder("/alpha/beta/gamma", 12),
            vec!["/alpha/beta/".to_string(), "gamma".to_string(),]
        );
    }

    #[test]
    fn folder_wrap_keeps_row_metadata_on_last_physical_line() {
        let path = "/alpha/beta/gamma/delta";
        let entries = vec![entry(7, path, EntryType::Directory, 1024)];
        let config = DisplayConfig {
            style: crate::terminal::Style::plain(),
            use_unicode: true,
            ascii_format: AsciiFormat::Compact,
            terminal_width: Some(72),
            long_paths: false,
            folder_wrap: true,
        };
        let output = render_table(&entries, &config);
        let data_lines = output
            .lines()
            .filter(|line| line.starts_with('│') && !line.contains("PATH"))
            .collect::<Vec<_>>();

        assert!(data_lines.len() > 1);
        assert!(!data_lines[0].contains("  7 "));
        assert!(!data_lines[0].contains(" dir "));
        assert!(data_lines.last().expect("last data line").contains(" 7 "));
        assert!(data_lines.last().expect("last data line").contains(" dir "));
        assert!(!output.contains('…'));
    }

    #[test]
    fn ascii_folder_wrap_keeps_metadata_on_last_physical_line() {
        let rows = collect_display_rows(
            &[entry(
                7,
                "/alpha/beta/gamma/delta",
                EntryType::Directory,
                1024,
            )],
            fixture_now(),
        );
        let output = render_ascii_narrow(&rows, &wrapped_ascii_config(AsciiFormat::Narrow, 28));
        let lines = output.lines().collect::<Vec<_>>();

        assert!(lines.len() > 3);
        assert!(!lines[2].contains("7"));
        assert!(lines.last().expect("last line").contains("7"));
        assert!(lines.last().expect("last line").contains("1.0 kB"));
    }

    #[test]
    fn compact_folder_wrap_keeps_table_structure_and_last_line_metadata() {
        let rows = collect_display_rows(
            &[entry(
                7,
                "/alpha/beta/gamma/delta",
                EntryType::Directory,
                1024,
            )],
            fixture_now(),
        );
        let output = render_ascii_compact(&rows, &wrapped_ascii_config(AsciiFormat::Compact, 42));
        let lines = output.lines().collect::<Vec<_>>();

        assert!(
            lines
                .iter()
                .all(|line| line.starts_with('+') || line.starts_with('|'))
        );
        assert!(lines.len() > 5);
        assert!(!lines[3].contains("7"));
        assert!(lines[4].contains("7"));
        assert!(lines[4].contains("1.0 kB"));
    }

    #[test]
    fn ascii_long_paths_keep_full_name_without_wrapping() {
        let long_path = "/alpha/beta/gamma/delta/epsilon/zeta";
        let rows = collect_display_rows(
            &[entry(7, long_path, EntryType::Directory, 1024)],
            fixture_now(),
        );
        let mut config = ascii_config(AsciiFormat::Narrow);
        config.terminal_width = Some(24);
        config.long_paths = true;
        let output = render_ascii_narrow(&rows, &config);

        assert!(output.contains("/alpha/beta/gamma/delta/epsilon/zeta/"));
        assert!(!output.contains('…'));
    }

    #[test]
    fn minimal_folder_wrap_has_headers_and_last_line_metadata() {
        let rows = collect_display_rows(
            &[entry(
                7,
                "/alpha/beta/gamma/delta",
                EntryType::Directory,
                1024,
            )],
            fixture_now(),
        );
        let output = render_ascii_minimal(&rows, &wrapped_ascii_config(AsciiFormat::Minimal, 28));
        let lines = output.lines().collect::<Vec<_>>();

        assert!(lines[0].contains("Name"));
        assert!(!lines[1].contains("7"));
        assert!(lines.last().expect("last line").contains("7"));
        assert!(lines.last().expect("last line").contains("1.0kB"));
    }

    #[test]
    fn unicode_columns_have_declared_alignment() {
        assert_eq!(super::UNICODE_COLUMNS[0].alignment, super::Alignment::Right);
        assert_eq!(super::UNICODE_COLUMNS[1].alignment, super::Alignment::Right);
        assert_eq!(super::UNICODE_COLUMNS[2].alignment, super::Alignment::Left);
        assert_eq!(
            super::UNICODE_COLUMNS[3].alignment,
            super::Alignment::Center
        );
        assert_eq!(super::UNICODE_COLUMNS[4].alignment, super::Alignment::Right);
        assert_eq!(super::UNICODE_COLUMNS[5].alignment, super::Alignment::Right);
        assert_eq!(super::UNICODE_COLUMNS[6].alignment, super::Alignment::Right);
        assert_eq!(super::UNICODE_COLUMNS[7].alignment, super::Alignment::Right);
        assert_eq!(super::UNICODE_COLUMNS[8].alignment, super::Alignment::Left);
    }

    #[test]
    fn unicode_non_binary_column_is_a_counter() {
        let output = render_table(&fixture_entries(), &DisplayConfig::unicode());

        assert!(output.contains("│   0 │    2 │"));
        assert!(!output.contains("│ 0   │ Y    │"));
        assert!(!output.contains("│ 0   │ N    │"));
    }

    #[test]
    fn aligned_cells_pad_left_center_and_right() {
        assert_eq!(super::aligned_cell("x", 5, super::Alignment::Left), "x    ");
        assert_eq!(
            super::aligned_cell("x", 5, super::Alignment::Center),
            "  x  "
        );
        assert_eq!(
            super::aligned_cell("x", 5, super::Alignment::Right),
            "    x"
        );
    }

    #[test]
    fn unicode_renderer_applies_color_roles_after_padding() {
        let mut ok = entry(1, "/ok", EntryType::Directory, 1024);
        ok.executable_count = 1;
        let mut problem = entry(2, "/missing", EntryType::Missing, 0);
        problem.exists = false;
        problem.executable_count = 0;

        let config = DisplayConfig {
            style: crate::terminal::Style { use_colors: true },
            use_unicode: true,
            ascii_format: AsciiFormat::Compact,
            terminal_width: Some(120),
            long_paths: false,
            folder_wrap: false,
        };
        let output = render_table(&[ok, problem], &config);

        assert!(output.contains("\x1b[33m"));
        assert!(output.contains("\x1b[36m"));
        assert!(output.contains("\x1b[32m"));
        assert!(output.contains("\x1b[31m"));
        assert!(
            output.find("\x1b[36m").expect("cyan header")
                < output.find("\x1b[33m").expect("yellow row number")
        );
    }

    #[test]
    fn ascii_config_renders_all_formats() {
        for format in [
            AsciiFormat::Compact,
            AsciiFormat::Narrow,
            AsciiFormat::Minimal,
        ] {
            let output = render_table(&fixture_entries(), &DisplayConfig::ascii(format));

            assert!(!output.contains('╭'));
        }
    }
}
