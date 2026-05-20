use anyhow::Result;
use stat_rain::cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    match cli.command {
        Command::Run(_) => {
            println!("stat-rain run scaffold");
        }
        Command::Init(_) => {
            println!("stat-rain init scaffold");
        }
        Command::Send(_) => {
            println!("stat-rain send scaffold");
        }
    }

    Ok(())
}
