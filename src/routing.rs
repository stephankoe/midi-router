/*
 * Core MIDI signal routing logic
 */

use crate::midi::MidiEvent;
use regex::Regex;
use std::collections::HashSet;
use log::debug;

#[derive(Debug, PartialEq)]
pub struct NumericRange<T> {
    pub start: T,
    pub end: T,
}

impl<T: PartialOrd> NumericRange<T> {
    pub fn is_within(&self, value: T) -> bool {
        value >= self.start && value <= self.end
    }
}

#[derive(Debug, Default)]
pub struct Condition {
    pub event_pattern: Option<Regex>,
    pub channel_pattern: Option<NumericRange<u8>>,
    pub value_pattern: Option<NumericRange<i16>>,
    pub velocity_pattern: Option<NumericRange<u8>>,
    pub controller_pattern: Option<NumericRange<u8>>,
}

impl Condition {
    pub fn matches(&self, midi_event: &MidiEvent) -> bool {
        let event_name: &'static str = midi_event.into();
        if !self.event_pattern.as_ref().map(|p| p.is_match(event_name)).unwrap_or(true) {
            return false
        }

        match midi_event {
            MidiEvent::NoteOff { channel, note, velocity } |
            MidiEvent::NoteOn { channel, note, velocity } |
            MidiEvent::PolyphonicAftertouch { channel, note, pressure: velocity } => {
                self.match_velocity(*velocity)
                    && self.match_value_u8(*note)
                    && self.match_channel(*channel)
            },
            MidiEvent::ControlChange { channel, control_no, value } => {
                self.match_channel(*channel)
                    && self.match_control_no(*control_no)
                    && self.match_value_u8(*value)
            },
            MidiEvent::ProgramChange { channel, program: value } |
            MidiEvent::ChannelAftertouch {channel, pressure: value}=> {
                self.match_channel(*channel) && self.match_value_u8(*value)
            },
            MidiEvent::PitchBendChange { channel, value } => {
                self.match_channel(*channel) && self.match_value(*value)
            },
            MidiEvent::SongSelect { song_num } => {
                self.match_value_u8(*song_num)
            },
            _ => true,
        }
    }

    fn match_channel(&self, channel: u8) -> bool {
        self.match_range(&self.channel_pattern, channel)
    }

    fn match_value(&self, value: i16) -> bool {
        self.match_range(&self.value_pattern, value)
    }

    fn match_value_u8(&self, value: u8) -> bool {
        self.match_value(value as i16)
    }

    fn match_velocity(&self, velocity: u8) -> bool {
        self.match_range(&self.velocity_pattern, velocity)
    }

    fn match_control_no(&self, controller: u8) -> bool {
        self.match_range(&self.controller_pattern, controller)
    }

    fn match_range<T: PartialOrd>(&self, range: &Option<NumericRange<T>>, value: T) -> bool {
        range.as_ref().map(|c| c.is_within(value)).unwrap_or(true)
    }
}

#[derive(Debug, PartialEq)]
pub enum Action {
    ForwardTo {
        output_port: String,
    },
}

#[derive(Debug)]
pub struct Rule {
    pub condition: Condition,
    pub actions: Vec<Action>,
}

pub struct RoutingTable {
    pub rules: Vec<Rule>,
}

impl RoutingTable {
    pub fn get_all_output_ports(&self) -> HashSet<&String> {
        let output_port_names = self.rules.iter()
            .flat_map(|rule| &rule.actions)
            .map(|action| match action {
                Action::ForwardTo { output_port } => output_port,
            });
        HashSet::from_iter(output_port_names)
    }

    pub fn get_output_ports(&self, midi_event: MidiEvent) -> Vec<&str> {
        let mut ports = Vec::new();
        for rule in &self.rules {
            if rule.condition.matches(&midi_event) {
                debug!("Rule {:?} matches event {:?}", rule, midi_event);
                let p = self.get_ports_from_actions(&rule.actions);
                ports.extend(p);
            } else {
                debug!("Rule {:?} does not match event {:?}", rule, midi_event);
            }
        }
        ports
    }

    fn get_ports_from_actions<'a>(&self, actions: &'a Vec<Action>) -> Vec<&'a str> {
        let mut ports = Vec::new();
        for action in actions {
            if let Some(port) = self.get_port_from_action(action) {
                ports.push(port);
            }
        }
        ports
    }

    fn get_port_from_action<'a>(&self, action: &'a Action) -> Option<&'a str> {
        match action {
            Action::ForwardTo { output_port } => {
                Some(&output_port)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_condition_matches_values() {
        let condition = Condition {
            event_pattern: None,
            channel_pattern: Some(NumericRange {start: 0, end: 8}),
            value_pattern: Some(NumericRange {start: -16, end: 15}),
            velocity_pattern: Some(NumericRange {start: 20, end: 40}), // a.k.a. pressure
            controller_pattern: Some(NumericRange {start: 5, end: 10}),
        };
        
        let note_off_event_ch0 = MidiEvent::NoteOff {
            note: 0,
            channel: 0,
            velocity: 20,
        };
        assert!(condition.matches(&note_off_event_ch0));
        
        let note_off_event_ch8 = MidiEvent::NoteOff {
            note: 0,
            channel: 8,
            velocity: 20,
        };
        assert!(condition.matches(&note_off_event_ch8));
        
        let note_off_event_ch9 = MidiEvent::NoteOff {
            note: 0,
            channel: 9,
            velocity: 20,
        };
        assert!(!condition.matches(&note_off_event_ch9));
        
        let note_on_event = MidiEvent::NoteOn {
            note: 0,
            channel: 0,
            velocity: 20,
        };
        assert!(condition.matches(&note_on_event));
        
        let note_on_event_val_15 = MidiEvent::NoteOn {
            note: 15,
            channel: 0,
            velocity: 20,
        };
        assert!(condition.matches(&note_on_event_val_15));
        
        let note_on_event_val_16 = MidiEvent::NoteOn {
            note: 16,
            channel: 0,
            velocity: 20,
        };
        assert!(!condition.matches(&note_on_event_val_16));
        
        let note_on_event_vel_15 = MidiEvent::NoteOn {
            note: 0,
            channel: 0,
            velocity: 20,
        };
        assert!(condition.matches(&note_on_event_vel_15));
        
        let note_on_event_vel_19 = MidiEvent::NoteOn {
            note: 0,
            channel: 0,
            velocity: 19,
        };
        assert!(!condition.matches(&note_on_event_vel_19));
        
        let note_on_event_vel_40 = MidiEvent::NoteOn {
            note: 0,
            channel: 0,
            velocity: 40,
        };
        assert!(condition.matches(&note_on_event_vel_40));
        
        let note_on_event_vel_41 = MidiEvent::NoteOn {
            note: 0,
            channel: 0,
            velocity: 41,
        };
        assert!(!condition.matches(&note_on_event_vel_41));
        
        let polyphonic_aftertouch_event = MidiEvent::PolyphonicAftertouch {
            note: 0,
            channel: 0,
            pressure: 20,
        };
        assert!(condition.matches(&polyphonic_aftertouch_event));
        
        let control_change_event_cn4 = MidiEvent::ControlChange {
            channel: 0,
            control_no: 4,
            value: 0,
        };
        assert!(!condition.matches(&control_change_event_cn4));

        let control_change_event_cn5 = MidiEvent::ControlChange {
            channel: 0,
            control_no: 5,
            value: 0,
        };
        assert!(condition.matches(&control_change_event_cn5));

        let control_change_event_cn10 = MidiEvent::ControlChange {
            channel: 0,
            control_no: 10,
            value: 0,
        };
        assert!(condition.matches(&control_change_event_cn10));

        let control_change_event_cn11 = MidiEvent::ControlChange {
            channel: 0,
            control_no: 11,
            value: 0,
        };
        assert!(!condition.matches(&control_change_event_cn11));

        let control_change_event_val15 = MidiEvent::ControlChange {
            channel: 0,
            control_no: 5,
            value: 15,
        };
        assert!(condition.matches(&control_change_event_val15));

        let control_change_event_val16 = MidiEvent::ControlChange {
            channel: 0,
            control_no: 5,
            value: 16,
        };
        assert!(!condition.matches(&control_change_event_val16));
    }
    
    #[test]
    fn test_condition_matches_pattern() {
        let condition = Condition {
            event_pattern: Some(Regex::new("note-[onf]{2,3}").unwrap()),
            ..Default::default()
        };
        
        let note_off_event = MidiEvent::NoteOff {
            note: 0,
            channel: 0,
            velocity: 0,
        };
        assert!(condition.matches(&note_off_event));
        
        let note_on_event = MidiEvent::NoteOn {
            note: 0,
            channel: 0,
            velocity: 0,
        };
        assert!(condition.matches(&note_on_event));
        
        let polyphonic_aftertouch_event = MidiEvent::PolyphonicAftertouch {
            note: 0,
            channel: 0,
            pressure: 0,
        };
        assert!(!condition.matches(&polyphonic_aftertouch_event));
    }

    #[test]
    fn test_routing_table_get_all_output_ports() {
        let create_condition = || {
            Condition {
                ..Default::default()
            }
        };
        let routing_table = RoutingTable {
            rules: vec![
                Rule {
                    condition: create_condition(),
                    actions: vec![
                        Action::ForwardTo {
                            output_port: "drums".to_string(),
                        },
                        Action::ForwardTo {
                            output_port: "lead".to_string(),
                        }
                    ],
                },
                Rule {
                    condition: create_condition(),
                    actions: vec![
                        Action::ForwardTo {
                            output_port: "lead".to_string(),
                        },
                        Action::ForwardTo {
                            output_port: "pads".to_string(),
                        }
                    ],
                },
                Rule {
                    condition: create_condition(),
                    actions: Vec::new(),
                },
                Rule {
                    condition: create_condition(),
                    actions: vec![
                        Action::ForwardTo {
                            output_port: "pads".to_string()
                        }
                    ],
                },
            ],
        };
        let output_ports = routing_table.get_all_output_ports();

        let expected: Vec<_> = vec!["drums", "lead", "pads"].into_iter()
            .map(String::from)
            .collect();
        assert_eq!(output_ports, expected.iter().collect());
    }
    
    #[test]
    fn test_routing_table_get_output_ports() {
        let create_rule = |pattern: &str, output_ports: Vec<&str>| {
            Rule {
                condition: Condition {
                    event_pattern: Some(Regex::new(pattern).unwrap()),
                    channel_pattern: None,
                    value_pattern: None,
                    velocity_pattern: None,
                    controller_pattern: None,
                },
                actions: output_ports.iter()
                    .map(|p| Action::ForwardTo { output_port: p.to_string() })
                    .collect(),
            }
        };
        
        
        let routing_table = RoutingTable {
            rules: vec![
                create_rule("note-off", vec!["x", "xx", "xxx"]),
                create_rule("note-on", vec!["a", "b", "c"]),
                create_rule("note-*", vec!["x", "y", "z"]),
            ],
        };
        let output_ports = routing_table.get_output_ports(MidiEvent::NoteOff {
            channel: 0,
            note: 0, 
            velocity: 0, 
        });
        
        let expected: Vec<_> = vec!["x", "xx", "xxx", "x", "y", "z"];
        assert_eq!(output_ports, expected);
    }
}
