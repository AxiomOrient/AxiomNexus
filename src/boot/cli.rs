use super::BootError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Serve,
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

fn usage() -> String {
    [
        "usage: axiomnexus <command>",
        "",
        "commands:",
        "  serve",
        "  migrate",
        "  doctor",
        "  replay",
        "  export",
        "  import",
        "  contract check",
    ]
    .join("\n")
}
