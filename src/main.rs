use std::process;

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use tracing_subscriber::{EnvFilter, fmt};

mod app;
mod calendar;
mod cli;
mod config;
mod db;
mod error;
mod holidays;
mod mcp;
mod models;
mod output;
mod timezones;
mod util;

use app::App;
use cli::{
    BizCommand, CalcCommand, Cli, Command, ConfigCommand, HolidayCommand, ServerCommand,
    TimerCommand, TzCommand,
};
use error::Result;

pub(crate) const APP_NAME: &str = "time-keep";

fn main() {
    init_tracing();

    let cli = Cli::parse();
    let output_format = cli.output_format();

    if let Err(err) = run(&cli) {
        if let Err(render_err) = output::render_error(output_format, &err) {
            eprintln!("error: failed to render error: {render_err}");
        }
        process::exit(1);
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn run(cli: &Cli) -> Result<()> {
    if let Command::Completions(args) = &cli.command {
        let mut command = Cli::command();
        generate(args.shell, &mut command, APP_NAME, &mut std::io::stdout());
        return Ok(());
    }

    let app = App::new(cli)?;
    match &cli.command {
        Command::Now(args) => {
            let zones = app.now_timezones(&args.timezones);
            let response = timezones::current_time(&zones, args.format)?;
            output::render(
                cli.output_format(),
                &response,
                output::TableData::TimeResponse(&response),
            )
        }
        Command::Tz {
            command: TzCommand::Info(args),
        } => {
            let info = timezones::timezone_info(&args.name)?;
            output::render(
                cli.output_format(),
                &info,
                output::TableData::TimezoneInfo(&info),
            )
        }
        Command::Tz {
            command: TzCommand::List(args),
        } => {
            let list = timezones::list_timezones(args.region.as_deref());
            output::render(
                cli.output_format(),
                &list,
                output::TableData::TimezoneList(&list),
            )
        }
        Command::Convert(args) => {
            let conversion =
                timezones::convert_timezone(&args.datetime, &args.from_tz, &args.to_tz)?;
            output::render(
                cli.output_format(),
                &conversion,
                output::TableData::TimezoneConversion(&conversion),
            )
        }
        Command::Calendar(args) => {
            let calendar = calendar::calendar_query(&args.date)?;
            output::render(
                cli.output_format(),
                &calendar,
                output::TableData::CalendarQuery(&calendar),
            )
        }
        Command::Calc {
            command: CalcCommand::Add(args),
        } => {
            let arithmetic = calendar::add(&args.date, args.amount, &args.unit)?;
            output::render(
                cli.output_format(),
                &arithmetic,
                output::TableData::DateArithmetic(&arithmetic),
            )
        }
        Command::Calc {
            command: CalcCommand::Subtract(args),
        } => {
            let arithmetic = calendar::subtract(&args.date, args.amount, &args.unit)?;
            output::render(
                cli.output_format(),
                &arithmetic,
                output::TableData::DateArithmetic(&arithmetic),
            )
        }
        Command::Calc {
            command: CalcCommand::Diff(args),
        } => {
            let diff = calendar::diff(&args.from, &args.to)?;
            output::render(
                cli.output_format(),
                &diff,
                output::TableData::DateDiff(&diff),
            )
        }
        Command::Format(args) => {
            let formatted = calendar::format_datetime(
                &args.input,
                args.output_format,
                args.strftime.as_deref(),
                args.input_format.as_deref(),
            )?;
            output::render(
                cli.output_format(),
                &formatted,
                output::TableData::DateFormatResult(&formatted),
            )
        }
        Command::Holiday {
            command: HolidayCommand::Check(args),
        } => {
            let check = holidays::holiday_check(&args.date, &args.country)?;
            output::render(
                cli.output_format(),
                &check,
                output::TableData::HolidayCheck(&check),
            )
        }
        Command::Holiday {
            command: HolidayCommand::List(args),
        } => {
            let list = holidays::holiday_list(args.year, &args.country)?;
            output::render(
                cli.output_format(),
                &list,
                output::TableData::HolidayList(&list),
            )
        }
        Command::Biz {
            command: BizCommand::Between(args),
        } => {
            let count = holidays::business_days_between(
                &args.from,
                &args.to,
                args.country.as_deref(),
                args.skip_holidays,
            )?;
            output::render(
                cli.output_format(),
                &count,
                output::TableData::BusinessDayCount(&count),
            )
        }
        Command::Biz {
            command: BizCommand::Next(args),
        } => {
            let search = holidays::next_business_day(&args.date, args.country.as_deref())?;
            output::render(
                cli.output_format(),
                &search,
                output::TableData::BusinessDaySearch(&search),
            )
        }
        Command::Biz {
            command: BizCommand::Prev(args),
        } => {
            let search = holidays::previous_business_day(&args.date, args.country.as_deref())?;
            output::render(
                cli.output_format(),
                &search,
                output::TableData::BusinessDaySearch(&search),
            )
        }
        Command::Config {
            command: ConfigCommand::Path,
        } => {
            let paths = app.config_paths();
            output::render(
                cli.output_format(),
                &paths,
                output::TableData::ConfigPaths(&paths),
            )
        }
        Command::Timer {
            command: TimerCommand::Set(args),
        } => {
            let mut store = app.timer_store()?;
            let timer = store.set_timer(
                &args.name,
                &args.deadline,
                args.description.as_deref(),
                &args.tags,
            )?;
            output::render(
                cli.output_format(),
                &timer,
                output::TableData::TimerRecord(&timer),
            )
        }
        Command::Timer {
            command: TimerCommand::Get(args),
        } => {
            let store = app.timer_store()?;
            let timer = store.get_timer(&args.name)?;
            output::render(
                cli.output_format(),
                &timer,
                output::TableData::TimerRecord(&timer),
            )
        }
        Command::Timer {
            command: TimerCommand::List(args),
        } => {
            let store = app.timer_store()?;
            let timers = store.list_timers(args.tag.as_deref())?;
            output::render(
                cli.output_format(),
                &timers,
                output::TableData::TimerList(&timers),
            )
        }
        Command::Timer {
            command: TimerCommand::Delete(args),
        } => {
            let mut store = app.timer_store()?;
            let deleted = store.delete_timer(&args.name)?;
            output::render(
                cli.output_format(),
                &deleted,
                output::TableData::TimerDelete(&deleted),
            )
        }
        Command::Timer {
            command: TimerCommand::Check,
        } => {
            let store = app.timer_store()?;
            let check = store.check_timers()?;
            output::render(
                cli.output_format(),
                &check,
                output::TableData::TimerCheck(&check),
            )
        }
        Command::Server(args) => match &args.command {
            ServerCommand::Start(args) => mcp::run_server(&app, args),
        },
        Command::Completions(_) => Ok(()),
    }
}
