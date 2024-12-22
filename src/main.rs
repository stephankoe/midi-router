mod parser;
mod midi;
mod routing;
mod jack_router;
mod error_handler;
mod utils;

use crate::jack_router::JackRouter;
use crate::parser::{load_rules_from_file, RuleConfigError};
use crate::routing::RoutingTable;
use std::error::Error;
use std::io;
use clap::Parser;
use log::{debug, info};
use crate::error_handler::{handle_config_error, handle_io_error, handle_jack_router_error};

#[derive(Parser)]
struct Cli {
    config_file: std::path::PathBuf,
}


fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    info!("Starting Jack MIDI router");
    let args = Cli::parse();

    let rules = match load_rules_from_file(&args.config_file) {
        Ok(rules) => rules,
        Err(err) => {
            if let Some(io_error) = err.downcast_ref::<io::Error>() {
                eprintln!("{}", handle_io_error(&args.config_file, io_error));
                std::process::exit(3);
            } else if let Some(rule_config_error) = err.downcast_ref::<RuleConfigError>() {
                eprintln!("{}", handle_config_error(&args.config_file, rule_config_error));
                std::process::exit(2);
            } else {
                eprintln!("An unknown error occurred: {}", err);
                std::process::exit(1);
            }
        },
    };

    debug!("Rules: {:?}", rules);

    let routing_table = RoutingTable { rules, };
    let router = match JackRouter::new(routing_table, "midi_router") {
        Ok(router) => router,
        Err(err) => {
            eprintln!("{}", handle_jack_router_error(&err));
            std::process::exit(4);
        }
    };

    wait_for_keypress();
    router.stop()?;

    Ok(())
}

fn wait_for_keypress() {
    println!("Press any key to quit");
    let mut user_input = String::new();
    io::stdin().read_line(&mut user_input).ok();
}
