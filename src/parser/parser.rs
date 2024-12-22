/*
 * Parse configuration files
 */

use std::cmp::{max, min};
use std::error::Error;
use std::fs::File;
use std::{io, mem};
use std::io::BufRead;
use std::path::Path;
use lazy_static::lazy_static;
use regex::{Captures, Match, Regex, RegexBuilder};
use crate::parser::{FieldFormatError, FieldParseError, RuleConfigError, RuleParseError};
use crate::routing::{Action, Condition, NumericRange, Rule};

lazy_static! {
    static ref FIELD_PAT: Regex = RegexBuilder::new(r"^(?P<type>ch|vel|ctrl)?(?:(?P<wildcard>[*])|(?P<start>-?\d+)-(?P<end>-?\d+)|>(?P<lower_bound>-?\d+)|<(?P<upper_bound>-?\d+)|(?P<exact_value>-?\d+))$")
        .case_insensitive(true)
        .build()
        .unwrap();
}

const FORWARD_SYMBOL: &str = "=>";

pub fn load_rules_from_file<P: AsRef<Path>>(file_path: &P) -> Result<Vec<Rule>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut rules = Vec::new();
    let mut errors = Vec::new();
    for (line_no, line_result) in io::BufReader::new(file).lines().enumerate() {
        let line = line_result?.trim().to_owned();
        if line.is_empty() {
            continue;
        }
        match parse_rule(line_no, line) {
            Ok(rule) => rules.push(rule),
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() {
        Ok(rules)
    } else {
        Err(RuleConfigError { errors }.into())
    }
}

fn _parse_version(line_no: usize, line: &String) -> Option<String> {
    if line_no == 0 && line.trim().starts_with("version: ") {
        match line.split_once(":") {
            Some((_, version_no)) => Some(version_no.to_string()),
            None => None,
        }
    } else {
        None
    }
}

fn parse_rule(line_no: usize, line: String) -> Result<Rule, RuleParseError> {
    RuleParser::new().parse(line_no, line)
}

struct RuleParser {
    condition_builder: ConditionBuilder,
    errors: Vec<FieldParseError>,
    output_names: Vec<String>,
    state: RuleParserState,
}

impl RuleParser {
    fn new() -> Self {
        RuleParser {
            condition_builder: ConditionBuilder::new(),
            errors: Vec::new(),
            output_names: Vec::new(),
            state: RuleParserState::ParseLeftHandSide,
        }
    }

    fn parse(&mut self, line_no: usize, line: String) -> Result<Rule, RuleParseError> {
        for (field_id, value) in line.trim().split_whitespace().enumerate() {
            if value == FORWARD_SYMBOL {
                self.state = RuleParserState::ParseRightHandSide;
                continue;
            }
            match self.state {
                RuleParserState::ParseLeftHandSide => self.parse_lhs(field_id, value),
                RuleParserState::ParseRightHandSide => self.parse_rhs(field_id, value),
            }
        }

        if self.errors.len() > 0 {
            Err(RuleParseError::InvalidFields {
                line_no,
                invalid_fields: mem::take(&mut self.errors),
            })?
        }

        let actions = mem::take(&mut self.output_names).into_iter()
            .map(|name| Action::ForwardTo { output_port: name })
            .collect();

        Ok(Rule {
            condition: self.condition_builder.build(),
            actions,
        })
    }

    fn parse_lhs(&mut self, field_id: usize, value: &str) {
        match parse_field_lhs(field_id, value) {
            Ok(Field::NameField { name_pattern }) => {
                self.condition_builder.event_pattern = Some(name_pattern);
            },
            Ok(Field::ValueField {start, end}) => {
                self.condition_builder.value_pattern = Some(NumericRange { start, end });
            },
            Ok(Field::ChannelField {start, end}) => {
                self.condition_builder.channel_pattern = Some(NumericRange {start, end });
            },
            Ok(Field::VelocityField {start, end}) => {
                self.condition_builder.velocity_pattern = Some(NumericRange {start, end });
            },
            Ok(Field::ControlNoField {start, end}) => {
                self.condition_builder.control_no_pattern = Some(NumericRange {start, end });
            },
            Err(error) => self.errors.push(error),
        }
    }

    fn parse_rhs(&mut self, _: usize, value: &str) {
        self.output_names.push(value.to_string());
    }
}

#[derive(Debug)]
struct ConditionBuilder {
    pub event_pattern: Option<Regex>,
    pub channel_pattern: Option<NumericRange<u8>>,
    pub value_pattern: Option<NumericRange<i16>>,
    pub velocity_pattern: Option<NumericRange<u8>>,
    pub control_no_pattern: Option<NumericRange<u8>>,
}

impl ConditionBuilder {
    fn new() -> Self {
        ConditionBuilder {
            event_pattern: None,
            channel_pattern: None,
            value_pattern: None,
            velocity_pattern: None,
            control_no_pattern: None,
        }
    }

    fn build(&mut self) -> Condition {
        Condition {
            event_pattern: mem::take(&mut self.event_pattern),
            channel_pattern: mem::take(&mut self.channel_pattern),
            value_pattern: mem::take(&mut self.value_pattern),
            velocity_pattern: mem::take(&mut self.velocity_pattern),
            controller_pattern: mem::take(&mut self.control_no_pattern),
        }
    }
}

enum RuleParserState {
    ParseLeftHandSide,
    ParseRightHandSide,
}

fn parse_field_lhs(field_id: usize, value: &str) -> Result<Field, FieldParseError> {
    if field_id == 0 {
        parse_name_pattern_field(field_id, value)
    } else if let Some(captures) = FIELD_PAT.captures(value) {
        parse_value_field(field_id, value, captures)
    } else {
        Err(FieldParseError {
            field_id,
            content: value.to_string(),
            reason: Some(FieldFormatError::InvalidFormat.into()),
        })
    }
}

fn parse_name_pattern_field(field_id: usize, value: &str) -> Result<Field, FieldParseError> {
    match Regex::new(value) {
        Ok(name_pattern) => Ok(Field::NameField { name_pattern }),
        Err(err) => Err(FieldParseError {
            field_id,
            content: value.to_string(),
            reason: Some(err.into()),
        }),
    }
}

fn parse_value_field(field_id: usize, value: &str, captures: Captures) -> Result<Field, FieldParseError> {
    let value_type_str = captures.name("type").map_or("", |m| m.as_str());

    let match_to_i16 = |m: Match| m.as_str()
        .parse::<i16>()
        .map_err(|err| FieldParseError {
            field_id,
            content: value.into(),
            reason: Some(err.into()),
        });
    let get_match_as_i16 = |name: &str| {
        let opt_value = captures.name(name).map(match_to_i16);
        switch_option_and_result(opt_value)
    };

    let default_start = if value_type_str == "" { i16::MIN } else { u8::MIN as i16 };
    let default_end = if value_type_str == "" { i16::MAX } else { u8::MAX as i16 };

    let start = get_match_as_i16("start")?.unwrap_or(default_start);
    let end = get_match_as_i16("end")?.unwrap_or(default_end);
    let lower_bound = get_match_as_i16("lower_bound")?.map(|b| b + 1).unwrap_or(default_start);
    let upper_bound = get_match_as_i16("upper_bound")?.map(|b| b - 1).unwrap_or(default_end);
    let exact_value = get_match_as_i16("exact_value")?;

    let start = exact_value.unwrap_or(max(start, lower_bound));
    let end = exact_value.unwrap_or(min(end, upper_bound));

    if value_type_str != "" && !(0 <= start && start <= end && end <= 0xff) {
        Err(FieldParseError {
            field_id,
            content: value.to_string(),
            reason: Some(FieldFormatError::NumberOutOfRange { min: 0, max: 0xff }.into()),
        })?
    }

    Ok(match value_type_str {
        "ch" => Field::ChannelField {start: start as u8, end: end as u8},
        "vel" => Field::VelocityField {start: start as u8, end: end as u8},
        "ctrl" => Field::ControlNoField {start: start as u8, end: end as u8},
        _ => Field::ValueField { start, end },
    })
}

fn switch_option_and_result<T, E>(item: Option<Result<T, E>>) -> Result<Option<T>, E> {
    match item {
        None => Ok(None),
        Some(Ok(value)) => Ok(Some(value)),
        Some(Err(e)) => Err(e),
    }
}

#[derive(Debug)]
enum Field {
    NameField {
        name_pattern: Regex,
    },
    ValueField {
        start: i16,
        end: i16,
    },
    ChannelField {
        start: u8,
        end: u8,
    },
    VelocityField {
        start: u8,
        end: u8,
    },
    ControlNoField {
        start: u8,
        end: u8,
    },
}

////////////////////////////////////////////////////////////////////////////////
//                                    Tests                                   //
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::io::Write;
    use tempfile::NamedTempFile;
    use super::*;

    #[test]
    fn test_load_rules_from_file() {
        let file_content = r#"
        note-.* ch<8 <40 vel*       => drums-out

        note-(on|off) ch0-10 >39 vel* => kb-out
        .*-aftertouch 127 =>
        "#;
        let file = write_tmp_file_content(file_content);
        let rules = load_rules_from_file(&file).unwrap();

        assert_eq!(rules.len(), 3);
        check_rule_ok(
            &rules[0],
            vec!["note-on", "note-off", "note-pikachu"],
            vec!["polyphonic-aftertouch", "control-change", "program-change"],
            Some(NumericRange { start: u8::MIN, end: 7 }),
            Some(NumericRange { start: i16::MIN, end: 39 }),
            Some(NumericRange { start: u8::MIN, end: u8::MAX }),
            None,
            vec![Action::ForwardTo { output_port: "drums-out".into() }],
        );
        check_rule_ok(
            &rules[1],
            vec!["note-on", "note-off"],
            vec!["note-pikachu", "polyphonic-aftertouch", "control-change", "program-change"],
            Some(NumericRange { start: 0, end: 10 }),
            Some(NumericRange { start: 40, end: i16::MAX }),
            Some(NumericRange { start: u8::MIN, end: u8::MAX }),
            None,
            vec![Action::ForwardTo { output_port: "kb-out".into() }],
        );
        check_rule_ok(
            &rules[2],
            vec!["polyphonic-aftertouch", "channel-aftertouch"],
            vec!["note-on", "control-change", "program-change"],
            None,
            Some(NumericRange { start: 127, end: 127 }),
            None,
            None,
            Vec::<Action>::new(),
        )
    }

    fn write_tmp_file_content(file_content: &str) -> NamedTempFile {
        let mut file = tempfile::Builder::new()
            .prefix("midi-router-test")
            .suffix(".config")
            .rand_bytes(6)
            .tempfile()
            .unwrap();
        write!(file, "{}", file_content).unwrap();
        file
    }

    fn check_rule_ok(
        rule: &Rule,
        event_names: Vec<&str>,
        wrong_names: Vec<&str>,
        expected_channel_range: Option<NumericRange<u8>>,
        expected_value_range: Option<NumericRange<i16>>,
        expected_velocity_range: Option<NumericRange<u8>>,
        expected_controller_range: Option<NumericRange<u8>>,
        expected_actions: Vec<Action>,
    ) {
        let name_pattern = rule.condition.event_pattern.as_ref().unwrap();
        assert!(rule.condition.event_pattern.is_some());
        for event_name in event_names {
            assert!(name_pattern.is_match(event_name), "'{}' unexpectedly didn't match pattern {}", event_name, name_pattern);
        }
        for event_name in wrong_names {
            assert!(!name_pattern.is_match(event_name), "'{}' unexpectedly matched pattern {}", event_name, name_pattern);
        }
        assert_eq!(rule.condition.channel_pattern, expected_channel_range);
        assert_eq!(rule.condition.value_pattern, expected_value_range);
        assert_eq!(rule.condition.velocity_pattern, expected_velocity_range);
        assert_eq!(rule.condition.controller_pattern, expected_controller_range);
        assert_eq!(rule.actions, expected_actions);
    }

    #[test]
    fn test_load_rules_from_file_with_io_error() {
        let result = load_rules_from_file(&"/this/path/does/not/exist");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_rules_from_file_with_parser_errors() {
        let file_content = r#"
        note-.* ch<100000 <40 vel*       => drums-out
        *** ch0-10 >39 vel* => kb-out
        ((( v0 ch-1 =>
        "#;
        let file = write_tmp_file_content(file_content);
        let result = load_rules_from_file(&file);

        assert!(result.is_err());
        let error = result.err().unwrap();
        assert!(error.is::<RuleConfigError>());
        let rule_config_err = error.downcast_ref::<RuleConfigError>().unwrap();
        assert_eq!(rule_config_err.errors.len(), 3);
    }

    #[test]
    fn test_parse_rule_valid_multi_forward() {
        let line_no = 0;
        let line = r"note-.* <64 ch0-8 vel>100 ctrl44 => out1 out2";
        let result = parse_rule(line_no, line.into());

        assert!(result.is_ok());
        if let Ok(Rule { condition, actions }) = result {
            assert!(condition.event_pattern.is_some());
            if let Some(pattern) = condition.event_pattern {
                assert!(pattern.is_match("note-on"));
                assert!(!pattern.is_match("notey-on"));
            }

            assert_eq!(condition.channel_pattern, Some(NumericRange {
                start: 0,
                end: 8,
            }));

            assert_eq!(condition.value_pattern, Some(NumericRange {
                start: i16::MIN,
                end: 63,
            }));

            assert_eq!(condition.velocity_pattern, Some(NumericRange {
                start: 101,
                end: u8::MAX,
            }));

            assert_eq!(condition.controller_pattern, Some(NumericRange {
                start: 44,
                end: 44,
            }));

            assert_eq!(actions, vec![
                Action::ForwardTo {
                    output_port: "out1".into(),
                },
                Action::ForwardTo {
                    output_port: "out2".into()
                }
            ]);
        } else {
            panic!("Unexpected result type {:?}", result);
        }
    }

    #[test]
    fn test_parse_rule_valid_drop() {
        let line_no = 0;
        let line = r".*-aftertouch =>";
        let result = parse_rule(line_no, line.into());

        assert!(result.is_ok());
        if let Ok(Rule { condition, actions }) = result {
            assert!(condition.event_pattern.is_some());
            if let Some(pattern) = condition.event_pattern {
                assert!(pattern.is_match("hello-aftertouch"));
                assert!(!pattern.is_match("note-on"));
            }

            assert!(condition.channel_pattern.is_none());
            assert!(condition.velocity_pattern.is_none());
            assert!(condition.value_pattern.is_none());
            assert!(condition.controller_pattern.is_none());

            assert!(actions.is_empty());
        } else {
            panic!("Unexpected result type {:?}", result);
        }
    }

    #[test]
    fn test_parse_rule_invalid() {
        let line_no = 127;
        let line = r"*-aftertouch 300000 v0 ch-1";
        let result = parse_rule(line_no, line.into());

        assert!(result.is_err());
        if let Err(RuleParseError::InvalidFields { line_no: err_line_no, invalid_fields }) = result {
            assert_eq!(err_line_no, line_no);
            assert_eq!(invalid_fields.len(), 4);
            assert_eq!(invalid_fields[0].field_id, 0);
            assert_eq!(invalid_fields[0].content, "*-aftertouch");
            assert!(invalid_fields[0].reason.is_some());
            assert_eq!(invalid_fields[1].field_id, 1);
            assert_eq!(invalid_fields[1].content, "300000");
            assert!(invalid_fields[1].reason.is_some());
            assert_eq!(invalid_fields[2].field_id, 2);
            assert_eq!(invalid_fields[2].content, "v0");
            assert!(invalid_fields[2].reason.is_some());
            assert_eq!(invalid_fields[3].field_id, 3);
            assert_eq!(invalid_fields[3].content, "ch-1");
            assert!(invalid_fields[3].reason.is_some());
        }
        else {
            panic!("Unexpected error type {:?}", result);
        }

    }

    #[test]
    fn test_parse_field_lhs_name_pattern() {
        let field_id = 0;
        let value = "note-on";
        let result = parse_field_lhs(field_id, value);

        assert!(result.is_ok());
        if let Ok(Field::NameField { name_pattern }) = result {
            assert!(name_pattern.is_match("note-on"));
            assert!(!name_pattern.is_match("note-off"));
        } else {
            panic!("Expected NameField variant");
        }
    }

    #[test]
    fn test_parse_field_lhs_value() {
        let field_id = 1;
        let value = "vel253";
        let result = parse_field_lhs(field_id, value);

        assert!(result.is_ok());
        if let Ok(Field::VelocityField { start, end }) = result {
            assert_eq!(start, 253);
            assert_eq!(end, 253);
        } else {
            panic!("Expected VelocityField variant");
        }
    }

    #[test]
    fn test_parse_field_lhs_error() {
        let field_id = 1;
        let value = ">.<";
        let result = parse_field_lhs(field_id, value);

        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err.field_id, field_id);
            assert_eq!(err.content, value);
            assert!(err.reason.is_some());
        }
    }

    #[test]
    fn test_parse_name_pattern_field_ok() {
        let field_id = 1;
        let value = r"no.*-(on|off)";
        let result = parse_name_pattern_field(field_id, value);

        assert!(result.is_ok());
        if let Ok(Field::NameField { name_pattern }) = result {
            assert!(name_pattern.is_match("note-on"));
            assert!(!name_pattern.is_match("123"));
        } else {
            panic!("Expected NameField variant");
        }
    }

    #[test]
    fn test_parse_name_pattern_field_invalid_pattern() {
        let field_id = 2;
        let value = r"no[te-*";
        let result = parse_name_pattern_field(field_id, value);

        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err.field_id, field_id);
            assert_eq!(err.content, value);
            assert!(err.reason.is_some());
        }
    }

    #[test]
    fn test_parse_value_field_ch_range() {
        let field_id = 1;
        let value = "ch5-12";
        let captures = FIELD_PAT.captures(value).unwrap();
        let result = parse_value_field(field_id, value, captures);

        assert!(result.is_ok());
        if let Ok(field) = result {
            match field {
                Field::ChannelField { start, end } => {
                    assert_eq!(start, 5);
                    assert_eq!(end, 12);
                },
                _ => panic!("Expected ChannelField variant"),
            }
        }
    }

    #[test]
    fn test_parse_value_field_vel_exact() {
        let field_id = 1;
        let value = "vel127";
        let captures = FIELD_PAT.captures(value).unwrap();
        let result = parse_value_field(field_id, value, captures);

        assert!(result.is_ok());
        if let Ok(field) = result {
            match field {
                Field::VelocityField { start, end } => {
                    assert_eq!(start, 127);
                    assert_eq!(end, 127);
                },
                _ => panic!("Expected VelocityField variant"),
            }
        }
    }

    #[test]
    fn test_parse_value_field_value_lower() {
        let field_id = 1;
        let value = "<300";
        let captures = FIELD_PAT.captures(value).unwrap();
        let result = parse_value_field(field_id, value, captures);

        assert!(result.is_ok());
        if let Ok(field) = result {
            match field {
                Field::ValueField { start, end } => {
                    assert_eq!(start, i16::MIN);
                    assert_eq!(end, 299);
                },
                _ => panic!("Expected ValueField variant"),
            }
        }
    }

    #[test]
    fn test_parse_value_field_ctrl_greater() {
        let field_id = 1;
        let value = "ctrl>5";
        let captures = FIELD_PAT.captures(value).unwrap();
        let result = parse_value_field(field_id, value, captures);

        assert!(result.is_ok());
        if let Ok(field) = result {
            match field {
                Field::ControlNoField { start, end } => {
                    assert_eq!(start, 6);
                    assert_eq!(end, u8::MAX);
                },
                _ => panic!("Expected ControlNoField variant"),
            }
        }
    }

    #[test]
    fn test_parse_value_field_ch_out_of_bounds() {
        let field_id = 1;
        let value = "ch300";
        let captures = FIELD_PAT.captures(value).unwrap();
        let result = parse_value_field(field_id, value, captures);

        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err.field_id, field_id);
            assert_eq!(err.content, value);
            assert!(err.reason.is_some());
        }
    }

    #[test]
    fn test_parse_value_field_vel_negative() {
        let field_id = 1;
        let value = "vel-5";
        let captures = FIELD_PAT.captures(value).unwrap();
        let result = parse_value_field(field_id, value, captures);

        assert!(result.is_err());
        if let Err(err) = result {
            assert_eq!(err.field_id, field_id);
            assert_eq!(err.content, value);
            assert!(err.reason.is_some());
        }
    }
}
