use heapless::Vec;

use super::property::ConnectionProperty;

pub enum Packet<'a, const PROPERTIES_N: usize> {
    Connect {
        keep_alive: u16,
        properties: Vec<ConnectionProperty<'a>, PROPERTIES_N>,
        username: Option<&'a str>,
        password: Option<&'a str>,
        client_id: &'a str,
    },
}

// impl<const PROPERTIES_N: usize> Write for Packet<'_, PROPERTIES_N> {
//     fn write<'w, W: MqttWriter<'w>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//         writer.
//     }
// }
