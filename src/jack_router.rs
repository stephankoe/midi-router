/*
 * JACK interface: creates and manages client and defines process handler
 */

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use jack::{AsyncClient, Client, ClientOptions, ClientStatus, Control, Error as JackError, MidiIn, MidiOut, MidiWriter, Port, ProcessHandler, ProcessScope, RawMidi};
use log::{debug, error, info};
use crate::midi::decode_raw_midi;
use crate::routing::RoutingTable;
use crate::utils::indent;

pub struct JackRouter {
    client: AsyncClient<(), JackRouterProcessHandler>,
}

impl JackRouter {
    pub fn new(routing_table: RoutingTable,
               router_name: &str) -> Result<JackRouter, JackRouterError> {
        let (client, _status) = Self::create_client(router_name)?;
        let midi_input_port = Self::register_midi_input_port(&client)?;
        let midi_output_ports = Self::register_midi_output_ports(&client, &routing_table)?;
        let process_handler = JackRouterProcessHandler {
            midi_input_port,
            midi_output_ports,
            routing_table,
        };
        let async_client = JackRouter::create_active_client(client, process_handler)?;

        Ok(JackRouter {
            client: async_client,
        })
    }

    fn create_client(router_name: &str) -> Result<(Client, ClientStatus), JackRouterError> {
        info!("Creating Jack client {}", router_name);
        Client::new(router_name, ClientOptions::default())
            .map_err(|err| JackRouterError { reasons: vec![err] })
    }

    fn register_midi_input_port(client: &Client) -> Result<Port<MidiIn>, JackRouterError> {
        let port_name = "midi_in";
        info!("Registering midi input port {}", port_name);
        client.register_port(port_name, MidiIn::default())
            .map_err(|err| JackRouterError { reasons: vec![err] })
    }

    fn register_midi_output_ports(client: &Client, routing_table: &RoutingTable) -> Result<HashMap<String, Port<MidiOut>>, JackRouterError> {
        let output_port_names = routing_table.get_all_output_ports();

        let mut midi_output_ports = HashMap::with_capacity(output_port_names.len());
        let mut errors = Vec::new();

        for port_name in output_port_names {
            info!("Registering midi output port {}", port_name);
            match client.register_port(port_name.as_str(), MidiOut::default()) {
                Ok(output_port) => {
                    midi_output_ports.insert(port_name.into(), output_port);
                },
                Err(error) => errors.push(error),
            }
        }

        if !errors.is_empty() {
            Err(JackRouterError {
                reasons: errors,
            })?
        }

        Ok(midi_output_ports)
    }

    fn create_active_client(client: Client, process_handler: JackRouterProcessHandler) -> Result<AsyncClient<(), JackRouterProcessHandler>, JackRouterError> {
        info!("Activating Jack client {}", client.name());
        client.activate_async((), process_handler)
            .map_err(|err| JackRouterError { reasons: vec![err] })
    }

    pub fn stop(self) -> Result<(), Box<dyn Error>> {
        info!("Deactivating Jack client");
        if let Err(err) = self.client.deactivate() {
            Err(JackRouterError { reasons: vec![err] })?
        };
        Ok(())
    }
}

pub struct JackRouterProcessHandler {
    midi_input_port: Port<MidiIn>,
    midi_output_ports: HashMap<String, Port<MidiOut>>,
    routing_table: RoutingTable,
}

impl JackRouterProcessHandler {
    fn send_event_out(raw_event: RawMidi,
                      output_port_names: Vec<&str>,
                      output_port_writers: &mut HashMap<String, MidiWriter>) {
        for port_name in output_port_names {
            if let Some(writer) = output_port_writers.get_mut(port_name) {
                debug!("Send signal {:?} to port {}", raw_event, port_name);
                writer.write(&raw_event).unwrap()
            } else {
                error!("Could not find output port writer: {}. Ignore this rule.", port_name);
            }
        }
    }

    fn create_output_port_writers<'a>(ps: &'a ProcessScope, output_ports: &'a mut HashMap<String, Port<MidiOut>>) -> HashMap<String, MidiWriter<'a>> {
        let mut output_writers = HashMap::with_capacity(output_ports.len());
        for (port_name, port) in output_ports {
            let writer = port.writer(ps);
            output_writers.insert(port_name.into(), writer);
        }
        output_writers
    }
}

impl ProcessHandler for JackRouterProcessHandler {
    fn process(&mut self, _: &Client, ps: &ProcessScope) -> Control {
        let mut output_port_writers = Self::create_output_port_writers(ps, &mut self.midi_output_ports);
        for raw_event in self.midi_input_port.iter(ps) {
            debug!("Received raw event {:?}", raw_event);
            let midi_event = match decode_raw_midi(raw_event) {
                Ok(event) => {
                    debug!("Decoded raw event to {:?}", event);
                    event
                },
                Err(err) => {
                    error!("Error decoding midi event: {}", err);
                    continue;
                },
            };
            let output_port_names = self.routing_table.get_output_ports(midi_event);

            Self::send_event_out(raw_event, output_port_names, &mut output_port_writers);
        }
        Control::Continue
    }
}

////////////////////////////////////////////////////////////////////////////////
//                                   Errors                                   //
////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct JackRouterError {
    pub reasons: Vec<JackError>,
}

impl Display for JackRouterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let msg = self.reasons.iter()
            .map(|err| format!("{}", err))
            .map(|msg| indent(msg, 4))
            .collect::<Vec<String>>()
            .join("\n  - ");
        write!(formatter, "JACK error(s) occurred:\n  - {}", msg)
    }
}

impl Error for JackRouterError {}
