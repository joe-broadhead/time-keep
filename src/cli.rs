use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(
    name = crate::APP_NAME,
    version,
    about = "Agent clock CLI and MCP server",
    long_about = "Local-first agent clock CLI and MCP server for current time, timezone operations, calendar queries, business days, timers, and offline holiday lookups."
)]
pub(crate) struct Cli {
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Json, help = "Output format")]
    pub(crate) output: OutputFormat,
    #[arg(
        long,
        global = true,
        conflicts_with = "output",
        help = "Shortcut for --output table"
    )]
    pub(crate) table: bool,
    #[arg(long, global = true, help = "Path to the time-keep TOML config")]
    pub(crate) config: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        env = "TIME_KEEP_DATA_DIR",
        help = "Directory for local time-keep data"
    )]
    pub(crate) data_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub(crate) command: Command,
}

impl Cli {
    pub(crate) fn output_format(&self) -> OutputFormat {
        if self.table {
            OutputFormat::Table
        } else {
            self.output
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputFormat {
    Json,
    Table,
    Csv,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    #[command(about = "Show current time in one or more IANA timezones")]
    Now(NowArgs),
    #[command(about = "List and inspect IANA timezones")]
    Tz {
        #[command(subcommand)]
        command: TzCommand,
    },
    #[command(about = "Convert a datetime between IANA timezones")]
    Convert(ConvertArgs),
    #[command(about = "Parse and format a datetime")]
    Format(FormatArgs),
    #[command(about = "Run date arithmetic and date diff operations")]
    Calc {
        #[command(subcommand)]
        command: CalcCommand,
    },
    #[command(about = "Query calendar fields for a date")]
    Calendar(CalendarArgs),
    #[command(about = "Calculate business days")]
    Biz {
        #[command(subcommand)]
        command: BizCommand,
    },
    #[command(about = "Check or list offline holidays")]
    Holiday {
        #[command(subcommand)]
        command: HolidayCommand,
    },
    #[command(about = "Manage local SQLite timers")]
    Timer {
        #[command(subcommand)]
        command: TimerCommand,
    },
    #[command(about = "Inspect time-keep config and data paths")]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    #[command(about = "Run the MCP server")]
    Server(ServerArgs),
    #[command(about = "Print shell completion scripts")]
    Completions(CompletionsArgs),
}

#[derive(Args, Debug)]
pub(crate) struct NowArgs {
    #[arg(long = "tz", help = "IANA timezone name; repeatable")]
    pub(crate) timezones: Vec<String>,
    #[arg(long, value_enum, default_value_t = TimeFormat::Rfc3339, help = "Datetime display format")]
    pub(crate) format: TimeFormat,
}

#[derive(Subcommand, Debug)]
pub(crate) enum TzCommand {
    #[command(about = "Show timezone metadata")]
    Info(TzInfoArgs),
    #[command(about = "List IANA timezone names")]
    List(TzListArgs),
}

#[derive(Args, Debug)]
pub(crate) struct TzInfoArgs {
    #[arg(help = "IANA timezone name")]
    pub(crate) name: String,
}

#[derive(Args, Debug)]
pub(crate) struct TzListArgs {
    #[arg(long, help = "Optional region filter, such as europe or america")]
    pub(crate) region: Option<String>,
}

#[derive(Args, Debug)]
pub(crate) struct ConvertArgs {
    #[arg(help = "Datetime to convert")]
    pub(crate) datetime: String,
    #[arg(long = "from", help = "Source IANA timezone")]
    pub(crate) from_tz: String,
    #[arg(long = "to", help = "Target IANA timezone")]
    pub(crate) to_tz: String,
}

#[derive(Args, Debug)]
pub(crate) struct FormatArgs {
    #[arg(help = "Datetime input")]
    pub(crate) input: String,
    #[arg(long, value_enum, default_value_t = DateOutputFormat::Rfc3339, help = "Output datetime format")]
    pub(crate) output_format: DateOutputFormat,
    #[arg(long, help = "strftime pattern when output-format is strftime")]
    pub(crate) strftime: Option<String>,
    #[arg(long, help = "Optional input format hint")]
    pub(crate) input_format: Option<String>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum CalcCommand {
    #[command(about = "Add a duration to a date or datetime")]
    Add(CalcArgs),
    #[command(about = "Subtract a duration from a date or datetime")]
    Subtract(CalcArgs),
    #[command(about = "Calculate the difference between two dates or datetimes")]
    Diff(DiffArgs),
}

#[derive(Args, Debug)]
pub(crate) struct CalcArgs {
    #[arg(help = "Date or datetime")]
    pub(crate) date: String,
    #[arg(help = "Amount to add or subtract")]
    pub(crate) amount: i64,
    #[arg(help = "Unit to add or subtract")]
    pub(crate) unit: String,
}

#[derive(Args, Debug)]
pub(crate) struct DiffArgs {
    #[arg(help = "Start date or datetime")]
    pub(crate) from: String,
    #[arg(help = "End date or datetime")]
    pub(crate) to: String,
}

#[derive(Args, Debug)]
pub(crate) struct CalendarArgs {
    #[arg(help = "ISO date, such as 2026-06-18")]
    pub(crate) date: String,
}

#[derive(Subcommand, Debug)]
pub(crate) enum BizCommand {
    #[command(about = "Count business days between two dates")]
    Between(BizBetweenArgs),
    #[command(about = "Find the next business day after a date")]
    Next(BizDateArgs),
    #[command(about = "Find the previous business day before a date")]
    Prev(BizDateArgs),
}

#[derive(Args, Debug)]
pub(crate) struct BizBetweenArgs {
    #[arg(help = "Start ISO date")]
    pub(crate) from: String,
    #[arg(help = "End ISO date")]
    pub(crate) to: String,
    #[arg(long, help = "ISO 3166-1 alpha-2 country code")]
    pub(crate) country: Option<String>,
    #[arg(long, help = "Skip country holidays in addition to weekends")]
    pub(crate) skip_holidays: bool,
}

#[derive(Args, Debug)]
pub(crate) struct BizDateArgs {
    #[arg(help = "ISO date")]
    pub(crate) date: String,
    #[arg(long, help = "ISO 3166-1 alpha-2 country code")]
    pub(crate) country: Option<String>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum HolidayCommand {
    #[command(about = "Check whether a date is an offline holiday")]
    Check(HolidayCheckArgs),
    #[command(about = "List offline holidays for a country/year")]
    List(HolidayListArgs),
}

#[derive(Args, Debug)]
pub(crate) struct HolidayCheckArgs {
    #[arg(help = "ISO date")]
    pub(crate) date: String,
    #[arg(long, help = "ISO 3166-1 alpha-2 country code")]
    pub(crate) country: String,
}

#[derive(Args, Debug)]
pub(crate) struct HolidayListArgs {
    #[arg(help = "Year")]
    pub(crate) year: i32,
    #[arg(long, help = "ISO 3166-1 alpha-2 country code")]
    pub(crate) country: String,
}

#[derive(Subcommand, Debug)]
pub(crate) enum TimerCommand {
    #[command(about = "Create or update a named timer")]
    Set(TimerSetArgs),
    #[command(about = "Read one timer")]
    Get(TimerNameArgs),
    #[command(about = "List timers")]
    List(TimerListArgs),
    #[command(about = "Delete a named timer")]
    Delete(TimerNameArgs),
    #[command(about = "List overdue timers")]
    Check,
}

#[derive(Args, Debug)]
pub(crate) struct TimerSetArgs {
    #[arg(help = "Timer name")]
    pub(crate) name: String,
    #[arg(help = "Deadline as ISO 8601/RFC3339 datetime")]
    pub(crate) deadline: String,
    #[arg(long, help = "Optional timer description")]
    pub(crate) description: Option<String>,
    #[arg(long = "tag", help = "Timer tag; repeatable")]
    pub(crate) tags: Vec<String>,
}

#[derive(Args, Debug)]
pub(crate) struct TimerNameArgs {
    #[arg(help = "Timer name")]
    pub(crate) name: String,
}

#[derive(Args, Debug)]
pub(crate) struct TimerListArgs {
    #[arg(long, help = "Filter by normalized tag")]
    pub(crate) tag: Option<String>,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigCommand {
    #[command(about = "Print resolved config and data paths")]
    Path,
}

#[derive(Args, Debug)]
pub(crate) struct ServerArgs {
    #[command(subcommand)]
    pub(crate) command: ServerCommand,
}

#[derive(Subcommand, Debug)]
pub(crate) enum ServerCommand {
    #[command(about = "Start an MCP server transport")]
    Start(ServerStartArgs),
}

#[derive(Args, Debug)]
pub(crate) struct ServerStartArgs {
    #[arg(long, value_enum, default_value_t = Transport::Stdio, help = "MCP transport")]
    pub(crate) transport: Transport,
    #[arg(
        long,
        default_value = "127.0.0.1",
        help = "HTTP host for streamable HTTP"
    )]
    pub(crate) http_host: String,
    #[arg(long, default_value_t = 8769, help = "HTTP port for streamable HTTP")]
    pub(crate) http_port: u16,
    #[arg(long, default_value = "/mcp", help = "HTTP MCP path")]
    pub(crate) http_path: String,
}

#[derive(Args, Debug)]
pub(crate) struct CompletionsArgs {
    #[arg(value_enum, help = "Shell to generate completions for")]
    pub(crate) shell: Shell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Transport {
    Stdio,
    StreamableHttp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum TimeFormat {
    Rfc3339,
    Iso8601,
    Epoch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum DateOutputFormat {
    Iso8601,
    Rfc3339,
    Rfc2822,
    Epoch,
    UnixTimestamp,
    Strftime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum DateUnit {
    #[value(alias = "second")]
    Seconds,
    #[value(alias = "minute")]
    Minutes,
    #[value(alias = "hour")]
    Hours,
    #[value(alias = "day")]
    Days,
    #[value(alias = "week")]
    Weeks,
    #[value(alias = "month")]
    Months,
    #[value(alias = "year")]
    Years,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn defaults_to_json_output() {
        let cli = Cli::try_parse_from(["time-keep", "config", "path"]).expect("valid cli");
        assert_eq!(cli.output_format(), OutputFormat::Json);
    }

    #[test]
    fn table_shortcut_overrides_output_default() {
        let cli =
            Cli::try_parse_from(["time-keep", "--table", "config", "path"]).expect("valid cli");
        assert_eq!(cli.output_format(), OutputFormat::Table);
    }

    #[test]
    fn parses_global_config_and_data_dir() {
        let cli = Cli::try_parse_from([
            "time-keep",
            "--config",
            "/tmp/time-keep.toml",
            "--data-dir",
            "/tmp/time-keep-data",
            "config",
            "path",
        ])
        .expect("valid cli");
        assert_eq!(
            cli.config.as_ref().and_then(|path| path.to_str()),
            Some("/tmp/time-keep.toml")
        );
        assert_eq!(
            cli.data_dir.as_ref().and_then(|path| path.to_str()),
            Some("/tmp/time-keep-data")
        );
    }

    #[test]
    fn parses_documented_singular_date_unit() {
        let cli = Cli::try_parse_from(["time-keep", "calc", "add", "2026-01-31", "1", "month"])
            .expect("documented singular unit should parse");
        match cli.command {
            Command::Calc {
                command: CalcCommand::Add(args),
            } => assert_eq!(args.unit, "month"),
            other => panic!("expected calc add, got {other:?}"),
        }
    }
}
