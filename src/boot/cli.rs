use super::BootError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Serve,
    SchedulerOnce,
    RunOnce(String),
    Migrate,
    Doctor,
    Replay,
    Export,
    Import,
    ContractCheck,
}

pub fn parse<I>(args: I) -> Result<Command, BootError>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let command = match args.next().as_deref() {
        Some("serve") => Command::Serve,
        Some("scheduler") => match args.next().as_deref() {
            Some("once") => Command::SchedulerOnce,
            _ => return Err(BootError::Cli(usage())),
        },
        Some("run") => match args.next() {
            Some(action) if action == "once" => match args.next() {
                Some(run_id) if !run_id.is_empty() => Command::RunOnce(run_id),
                _ => return Err(BootError::Cli(usage())),
            },
            _ => return Err(BootError::Cli(usage())),
        },
        Some("migrate") => Command::Migrate,
        Some("doctor") => Command::Doctor,
        Some("replay") => Command::Replay,
        Some("export") => Command::Export,
        Some("import") => Command::Import,
        Some("contract") => match args.next().as_deref() {
            Some("check") => Command::ContractCheck,
            _ => return Err(BootError::Cli(usage())),
        },
        _ => return Err(BootError::Cli(usage())),
    };

    if args.next().is_some() {
        return Err(BootError::Cli(usage()));
    }

    Ok(command)
}

#[cfg(test)]
pub(crate) fn usage_text() -> String {
    usage()
}

fn usage() -> String {
    [
        "usage: axiomnexus <command>",
        "",
        "commands:",
        "  serve",
        "  scheduler once",
        "  run once <run_id>",
        "  migrate",
        "  doctor",
        "  replay",
        "  export",
        "  import",
        "  contract check",
    ]
    .join("\n")
}
