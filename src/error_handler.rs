/* 
 * Top-level error handling methods
 */
use std::io;
use std::path::Path;
use crate::jack_router::JackRouterError;
use crate::parser::RuleConfigError;

pub fn handle_io_error<P: AsRef<Path>>(filepath: &P, e: &io::Error) -> String {
    let filepath_str = filepath.as_ref().display().to_string();
    match e.kind() {
        io::ErrorKind::NotFound => format!("The file '{}' was not found.", filepath_str),
        io::ErrorKind::PermissionDenied => format!("Permission denied. You may not have the necessary permissions to access the file '{}'.", filepath_str),
        io::ErrorKind::AlreadyExists => format!("The file '{}' already exists.", filepath_str),
        io::ErrorKind::WriteZero => format!("An attempt was made to write zero bytes to '{}'.", filepath_str),
        io::ErrorKind::UnexpectedEof => format!("An unexpected end of file was encountered at '{}'.", filepath_str),
        _ => format!("An unknown I/O error occurred when accessing '{}': {}", filepath_str, e),
    }
}

pub fn handle_config_error<P: AsRef<Path>>(filepath: &P, e: &RuleConfigError) -> String {
    let filepath_str = filepath.as_ref().display().to_string();
    format!("Error in config file '{}': {}", filepath_str, e)
}

pub fn handle_jack_router_error(e: &JackRouterError) -> String {
    let error_msgs = e.reasons.iter()
        .map(|jack_error| format!("{}", jack_error))
        .collect::<Vec<String>>()
        .join("\n  - ");
    format!("The following Jack-related errors occurred:\n  - {}", error_msgs)
}