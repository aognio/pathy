use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub struct PathEntry {
    pub index: usize,
    pub path: PathBuf,
    pub entry_type: EntryType,
    pub exists: bool,
    pub executable_count: usize,
    pub non_executable_count: usize,
    pub size_bytes: u64,
    pub duplicate: bool,
    pub modified: Option<SystemTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryType {
    Directory,
    File,
    Other,
    Missing,
}

impl EntryType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Directory => "dir",
            Self::File => "file",
            Self::Other => "other",
            Self::Missing => "missing",
        }
    }
}

pub fn read_path() -> Vec<PathEntry> {
    let raw_path = match env::var_os("PATH") {
        Some(value) => value,
        None => return Vec::new(),
    };

    let paths = env::split_paths(&raw_path);
    let mut entries = Vec::new();

    for (index, path) in paths.enumerate() {
        let metadata = fs::metadata(&path).ok();
        let exists = metadata.is_some();
        let entry_type = metadata
            .as_ref()
            .map(entry_type_from_metadata)
            .unwrap_or(EntryType::Missing);
        let modified = metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok());

        let file_counts = if matches!(entry_type, EntryType::Directory) {
            count_directory_files(&path)
        } else {
            FileCounts::default()
        };

        let size_bytes = if matches!(entry_type, EntryType::Directory) {
            directory_size(&path)
        } else {
            metadata
                .as_ref()
                .map(|metadata| metadata.len())
                .unwrap_or(0)
        };

        let duplicate = entries.iter().any(|entry: &PathEntry| entry.path == path);

        entries.push(PathEntry {
            index: index + 1,
            path,
            entry_type,
            exists,
            executable_count: file_counts.executable,
            non_executable_count: file_counts.non_executable,
            size_bytes,
            duplicate,
            modified,
        });
    }

    entries
}

fn entry_type_from_metadata(metadata: &fs::Metadata) -> EntryType {
    if metadata.is_dir() {
        EntryType::Directory
    } else if metadata.is_file() {
        EntryType::File
    } else {
        EntryType::Other
    }
}

#[derive(Default)]
struct FileCounts {
    executable: usize,
    non_executable: usize,
}

fn count_directory_files(path: &Path) -> FileCounts {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return FileCounts::default(),
    };

    let mut counts = FileCounts::default();

    for entry in entries.flatten() {
        let file_path = entry.path();

        if !file_path.is_file() {
            continue;
        }

        if is_executable(&file_path) {
            counts.executable += 1;
        } else {
            counts.non_executable += 1;
        }
    }

    counts
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.extension()
        .map(|extension| {
            let extension = extension.to_string_lossy().to_ascii_lowercase();
            extension == "exe" || extension == "com" || extension == "bat"
        })
        .unwrap_or(false)
}

fn directory_size(path: &Path) -> u64 {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };

    let mut total = 0;

    for entry in entries.flatten() {
        let file_path = entry.path();

        let metadata = match fs::symlink_metadata(&file_path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        if metadata.is_file() {
            total += metadata.len();
        }
    }

    total
}

pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    let mut value = bytes as f64;
    let mut unit_index = 0;

    while value >= 1024.0 && unit_index < UNITS.len() - 1 {
        value /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} B", bytes)
    } else {
        format!("{:.1} {}", value, UNITS[unit_index])
    }
}

pub fn format_modified(modified: Option<SystemTime>) -> String {
    let Some(modified) = modified else {
        return "-".to_string();
    };

    let now = SystemTime::now();

    match now.duration_since(modified) {
        Ok(age) => format_age(age, "ago"),
        Err(error) => format_age(error.duration(), "from now"),
    }
}

fn format_age(age: Duration, suffix: &str) -> String {
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
        (seconds / MINUTE, "minute")
    } else if seconds < DAY {
        (seconds / HOUR, "hour")
    } else if seconds < MONTH {
        (seconds / DAY, "day")
    } else if seconds < YEAR {
        (seconds / MONTH, "month")
    } else {
        (seconds / YEAR, "year")
    };

    let plural = if value == 1 { "" } else { "s" };

    format!("{value} {unit}{plural} {suffix}")
}

#[cfg(test)]
mod tests {
    use super::{count_directory_files, directory_size, format_modified, format_size};
    use std::fs;
    use std::time::{Duration, SystemTime};

    #[test]
    fn formats_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
    }

    #[test]
    fn formats_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
    }

    #[test]
    fn formats_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn formats_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn directory_size_counts_direct_files_only() {
        let base = std::env::temp_dir().join(format!("pathy-size-test-{}", std::process::id()));

        let nested = base.join("nested");

        fs::create_dir_all(&nested).expect("create test directories");

        fs::write(base.join("one.bin"), vec![0u8; 1024]).expect("write direct file");

        fs::write(nested.join("two.bin"), vec![0u8; 2048]).expect("write nested file");

        assert_eq!(directory_size(&base), 1024);

        fs::remove_dir_all(&base).expect("remove test directory");
    }

    #[test]
    fn directory_file_counts_split_executable_and_non_executable_files() {
        let base =
            std::env::temp_dir().join(format!("pathy-file-count-test-{}", std::process::id()));

        fs::create_dir_all(&base).expect("create test directory");
        let executable = base.join("executable");
        let non_executable = base.join("notes.txt");

        fs::write(&executable, b"#!/bin/sh\n").expect("write executable");
        fs::write(&non_executable, b"notes").expect("write non executable");
        fs::create_dir_all(base.join("nested")).expect("create nested directory");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&executable, permissions).expect("set executable permissions");
        }

        let counts = count_directory_files(&base);

        assert_eq!(counts.executable, 1);
        assert_eq!(counts.non_executable, 1);

        fs::remove_dir_all(&base).expect("remove test directory");
    }

    #[test]
    fn formats_missing_modified_time() {
        assert_eq!(format_modified(None), "-");
    }

    #[test]
    fn formats_recent_modified_time_as_now() {
        assert_eq!(format_modified(Some(SystemTime::now())), "now");
    }

    #[test]
    fn formats_past_modified_time() {
        let modified = SystemTime::now() - Duration::from_secs(2 * 60 * 60);

        assert_eq!(format_modified(Some(modified)), "2 hours ago");
    }
}
