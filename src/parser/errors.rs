/*
 * List of errors in the rule configuration (file)
 */
use std::error::Error;
use std::fmt::{Display, Formatter};
use crate::utils::indent;

#[derive(Debug)]
pub struct  RuleConfigError {
    pub errors: Vec<RuleParseError>,
}

impl Display for RuleConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let error_strs = self.errors.iter()
            .map(|error| format!("{}", error))
            .map(|msg| indent(msg, 4))
            .collect::<Vec<String>>()
            .join("\n  - ");
        write!(
            formatter,
            "Rule parsing failed. {} errors were found: \n  - {}",
            self.errors.len(),
            error_strs,
        )
    }
}

impl Error for RuleConfigError {}


/*
 * Error in a rule
 */

#[derive(Debug)]
pub enum RuleParseError {
    InvalidFields {
        line_no: usize,
        invalid_fields: Vec<FieldParseError>,
    },
}

impl Display for RuleParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleParseError::InvalidFields { line_no, invalid_fields } => {
                let invalid_fields_strs = invalid_fields.iter()
                    .map(|field| format!("{}", field))
                    .map(|msg| indent(msg, 4))
                    .collect::<Vec<String>>()
                    .join("\n  - ");
                write!(formatter, "Invalid field in line {}:\n  - {}", line_no + 1, invalid_fields_strs)
            }
        }
    }
}

impl Error for RuleParseError {}


/*
 * Error in a field within a rule
 */

#[derive(Debug)]
pub struct FieldParseError {
    pub field_id: usize,
    pub content: String,
    pub reason: Option<Box<dyn Error>>,
}

impl Display for FieldParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let reason_str = match self.reason {
            Some(ref reason) => format!("{}", reason),
            None => "Reason unknown".into(),
        };
        write!(
            formatter,
            "Parsing '{}' in field {} failed: {}",
            self.content,
            self.field_id,
            reason_str,
        )
    }
}

impl Error for FieldParseError {}


/*
 * Invalid field format
 */

#[derive(Debug)]
pub enum FieldFormatError {
    InvalidFormat,
    NumberOutOfRange { min: i16, max: i16 },
}


impl Display for FieldFormatError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let reason_str = match self {
            FieldFormatError::InvalidFormat => "Invalid format".to_string(),
            FieldFormatError::NumberOutOfRange { min, max } => format!(
                "Value must be between {} and {}",
                min + 1, 
                max - 1,
            ),
        };
        write!(formatter, "{}", reason_str)
    }
}

impl Error for FieldFormatError {}