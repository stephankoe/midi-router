/*
 * Decode MIDI events
 */

use std::error::Error;
use jack::RawMidi;
use strum_macros::IntoStaticStr;

/*
 * MIDI events designed according to https://midi.org/expanded-midi-1-0-messages-list
 */

const MIN_PITCHWHEEL: i16 = -8192;

#[derive(Debug, IntoStaticStr, PartialEq)]
pub enum MidiEvent {
    #[strum(serialize = "note-off")]
    NoteOff {
        channel: u8,
        note: u8,
        velocity: u8,
    },   // Indicates a note being released
    #[strum(serialize = "note-on")]
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
    }, // Indicates a note being played
    #[strum(serialize = "polyphonic-aftertouch")]
    PolyphonicAftertouch {
        channel: u8,
        note: u8,
        pressure: u8,
    }, // Pressure applied to a key
    #[strum(serialize = "control-change")]
    ControlChange {
        channel: u8,
        control_no: u8,
        value: u8,
    }, // Change in a control parameter
    #[strum(serialize = "program-change")]
    ProgramChange {
        channel: u8,
        program: u8,
    }, // Change to a different instrument sound
    #[strum(serialize = "channel-aftertouch")]
    ChannelAftertouch {
        channel: u8,
        pressure: u8,
    }, // Pressure applied to the entire channel
    #[strum(serialize = "pitch-bend-change")]
    PitchBendChange {
        channel: u8,
        value: i16,
    }, // Pitch bend event
    #[strum(serialize = "system-exclusive")]
    SystemExclusive {},
    #[strum(serialize = "midi-time-code-qtr-frame")]
    MidiTimeCodeQtrFrame {},
    #[strum(serialize = "song-position-pointer")]
    SongPositionPointer {},
    #[strum(serialize = "song-select")]
    SongSelect {
        song_num: u8,
    },
    #[strum(serialize = "tone-request")]
    TuneRequest {},
    #[strum(serialize = "end-of-sys-ex")]
    EndOfSysEx {},
    #[strum(serialize = "timing-clock")]
    TimingClock {},
    #[strum(serialize = "start")]
    Start {},
    #[strum(serialize = "continue")]
    Continue {},
    #[strum(serialize = "stop")]
    Stop {},
    #[strum(serialize = "active-sensing")]
    ActiveSensing {},
    #[strum(serialize = "system-reset")]
    SystemReset {},
    #[strum(serialize = "undefined")]
    Undefined {},
}

pub fn decode_raw_midi(raw_midi: RawMidi) -> Result<MidiEvent, Box<dyn Error>> {
    let event_type = raw_midi.bytes[0] >> 4;
    let channel = (raw_midi.bytes[0] & 0x0f) + 1;  // channel number is 1-based in standard
    let event = match event_type {
        0x8 => MidiEvent::NoteOff {
            channel,
            note: raw_midi.bytes[1],
            velocity: raw_midi.bytes[2],
        },
        0x9 => MidiEvent::NoteOn {
            channel,
            note: raw_midi.bytes[1],
            velocity: raw_midi.bytes[2],
        },
        0xa => MidiEvent::PolyphonicAftertouch {
            channel,
            note: raw_midi.bytes[1],
            pressure: raw_midi.bytes[2],
        },
        0xb => MidiEvent::ControlChange {
            channel,
            control_no: raw_midi.bytes[1],
            value: raw_midi.bytes[2],
        },
        0xc => MidiEvent::ProgramChange {
            channel,
            program: raw_midi.bytes[1],
        },
        0xd => MidiEvent::ChannelAftertouch {
            channel,
            pressure: raw_midi.bytes[1],
        },
        0xe => {
            MidiEvent::PitchBendChange {
                channel,
                value: ((raw_midi.bytes[2] as i16) << 7) + (raw_midi.bytes[1] as i16) + MIN_PITCHWHEEL,
            }
        },
        0xf => match channel {
            0x0 => MidiEvent::SystemExclusive {},
            0x1 => MidiEvent::MidiTimeCodeQtrFrame {},
            0x2 => MidiEvent::SongPositionPointer {},
            0x3 => MidiEvent::SongSelect {
                song_num: raw_midi.bytes[1],
            },
            0x6 => MidiEvent::TuneRequest {},
            0x7 => MidiEvent::EndOfSysEx {},
            0x8 => MidiEvent::TimingClock {},
            0xa => MidiEvent::Start {},
            0xb => MidiEvent::Continue {},
            0xc => MidiEvent::Stop {},
            0xe => MidiEvent::ActiveSensing {},
            0xf => MidiEvent::SystemReset {},
            _ => MidiEvent::Undefined {},
        },
        _ => Err(format!("Unknown MIDI event ID: {}", event_type))?
    };
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_raw_midi_note_off() {
        let bytes = vec![133, 123, 25];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::NoteOff {
            channel: 6,
            note: 123,
            velocity: 25,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_note_on() {
        let bytes = vec![144, 0, 0];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::NoteOn {
            channel: 1,
            note: 0,
            velocity: 0,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_polyphonic_aftertouch() {
        let bytes = vec![175, 127, 127];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::PolyphonicAftertouch {
            channel: 16,
            note: 127,
            pressure: 127,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_control_change() {
        let bytes = vec![190, 5, 5];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::ControlChange {
            channel: 15,
            control_no: 5,
            value: 5,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_pitch_bend_change_positive() {
        let bytes = vec![230, 66, 123];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::PitchBendChange {
            channel: 7,
            value: 7618,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_pitch_bend_change_negative() {
        let bytes = vec![230, 66, 28];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::PitchBendChange {
            channel: 7,
            value: -4542,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_pitch_bend_change_max() {
        let bytes = vec![230, 127, 127];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::PitchBendChange {
            channel: 7,
            value: 8191,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_pitch_bend_change_min() {
        let bytes = vec![230, 0, 0];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::PitchBendChange {
            channel: 7,
            value: -8192,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }

    #[test]
    fn test_decode_raw_midi_pitch_bend_change_zero() {
        let bytes = vec![230, 0, 64];
        let raw_midi = RawMidi { time: 0, bytes: &bytes};

        let result = decode_raw_midi(raw_midi);

        let expected = MidiEvent::PitchBendChange {
            channel: 7,
            value: 0,
        };
        assert!(result.is_ok());
        assert_eq!(expected, result.unwrap());
    }
}
