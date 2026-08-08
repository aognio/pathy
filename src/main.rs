mod display;
mod path;
mod terminal;

use clap::Parser;
use display::AsciiFormat;

#[derive(Parser, Debug)]
#[command(
    name = "pathy",
    version,
    about = "A modern, visual explorer for your PATH"
)]
struct Cli {
    #[arg(long, value_enum)]
    ascii_format: Option<AsciiFormat>,

    #[arg(long)]
    monochrome: bool,
}

fn main() {
    let cli = Cli::parse();

    let entries = path::read_path();

    let config = display::DisplayConfig::new(cli.ascii_format, cli.monochrome);

    display::print_table(&entries, &config);
    if config.should_print_summary() {
        display::print_summary(&entries, &config);
    }
}

#[cfg(test)]
mod tests {
    use super::{AsciiFormat, Cli};
    use clap::Parser;

    #[test]
    fn accepts_compact_ascii_format() {
        let cli = Cli::try_parse_from(["pathy", "--ascii-format", "compact"]).unwrap();

        assert_eq!(cli.ascii_format, Some(AsciiFormat::Compact));
    }

    #[test]
    fn accepts_narrow_ascii_format() {
        let cli = Cli::try_parse_from(["pathy", "--ascii-format", "narrow"]).unwrap();

        assert_eq!(cli.ascii_format, Some(AsciiFormat::Narrow));
    }

    #[test]
    fn accepts_minimal_ascii_format() {
        let cli = Cli::try_parse_from(["pathy", "--ascii-format", "minimal"]).unwrap();

        assert_eq!(cli.ascii_format, Some(AsciiFormat::Minimal));
    }

    #[test]
    fn rejects_unknown_ascii_format() {
        let error = Cli::try_parse_from(["pathy", "--ascii-format", "wide"]).unwrap_err();
        let message = error.to_string();

        assert!(message.contains("invalid value 'wide'"));
        assert!(message.contains("compact"));
        assert!(message.contains("narrow"));
        assert!(message.contains("minimal"));
    }

    #[test]
    fn rejects_non_lowercase_ascii_format() {
        let error = Cli::try_parse_from(["pathy", "--ascii-format", "Compact"]).unwrap_err();

        assert!(error.to_string().contains("invalid value 'Compact'"));
    }

    #[test]
    fn accepts_monochrome_flag() {
        let cli = Cli::try_parse_from(["pathy", "--monochrome"]).unwrap();

        assert!(cli.monochrome);
    }
}
