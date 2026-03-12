pub mod config;

mod cli;
mod wire;

use std::{env, error::Error, fmt};

#[derive(Debug)]
pub enum BootError {
    Cli(String),
    Io(std::io::Error),
    Store(String),
    Runtime(String),
}

impl fmt::Display for BootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cli(message) => f.write_str(message),
            Self::Io(err) => write!(f, "{err}"),
            Self::Store(err) => f.write_str(err),
            Self::Runtime(err) => f.write_str(err),
        }
    }
}

impl Error for BootError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cli(_) => None,
            Self::Io(err) => Some(err),
            Self::Store(_) | Self::Runtime(_) => None,
        }
    }
}

impl From<std::io::Error> for BootError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<crate::port::store::StoreError> for BootError {
    fn from(value: crate::port::store::StoreError) -> Self {
        Self::Store(value.to_string())
    }
}

impl From<crate::port::runtime::RuntimeError> for BootError {
    fn from(value: crate::port::runtime::RuntimeError) -> Self {
        Self::Runtime(value.to_string())
    }
}

pub fn run() -> Result<(), BootError> {
    let config = config::Config::from_env();
    let command = cli::parse(env::args().skip(1))?;

    wire::dispatch(command, &config)
}
