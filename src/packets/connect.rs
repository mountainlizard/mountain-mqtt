use super::{
    packet::{PacketWrite, PROTOCOL_NAME, PROTOCOL_VERSION_5},
    packet_type::PacketType,
    property::ConnectProperty,
};
use crate::data::{
    mqtt_writer::{self, MqttLenWriter, MqttWriter},
    write::Write,
};
use heapless::Vec;

pub struct Connect<'a, const PROPERTIES_N: usize> {
    keep_alive: u16,
    username: Option<&'a str>,
    password: Option<&'a [u8]>,
    client_id: &'a str,
    clean_start: bool,
    properties: Vec<ConnectProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize> Connect<'a, PROPERTIES_N> {
    pub fn new(
        keep_alive: u16,
        username: Option<&'a str>,
        password: Option<&'a [u8]>,
        client_id: &'a str,
        clean_start: bool,
    ) -> Self {
        Self {
            keep_alive,
            username,
            password,
            client_id,
            clean_start,
            properties: Vec::new(),
        }
    }

    pub fn push_property(&mut self, p: ConnectProperty<'a>) -> Result<(), ConnectProperty<'a>> {
        self.properties.push(p)
    }

    fn write_properties<'w, W: MqttWriter<'w>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        for p in self.properties.iter() {
            p.write(writer)?;
        }
        Ok(())
    }

    fn connect_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.clean_start {
            flags |= 1 << 1;
        }
        if self.password.is_some() {
            flags |= 1 << 6;
        }
        if self.username.is_some() {
            flags |= 1 << 7;
        }
        // TODO: Will flags, stored in bits 2, 3, 4, 5
        flags
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Connect<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Connect
    }

    fn write_variable_header<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Find length of written properties
        let mut lw = MqttLenWriter::new();
        self.write_properties(&mut lw)?;
        let properties_len = lw.position();

        // Write the fixed parts of the variable header
        writer.put_str(PROTOCOL_NAME)?; // 3.1.2.1 Protocol name
        writer.put_u8(PROTOCOL_VERSION_5)?; // 3.1.2.2 Protocol Version
        writer.put_u8(self.connect_flags())?; // 3.1.2.3 Connect Flags
        writer.put_u16(self.keep_alive)?; // 3.1.2.10 Keep Alive

        // Write the properties array
        writer.put_variable_u32(properties_len as u32)?; // 3.1.2.11.1 Property Length
        self.write_properties(writer)?; // 3.1.2.11.2 onwards, properties

        Ok(())
    }

    fn write_payload<'w, W: MqttWriter<'w>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        // 3.1.3.1 Client Identifier (ClientID)
        writer.put_str(self.client_id)?;

        // TODO: Omitted for now - they are optional and we omit them in connect flags
        // 3.1.3.2 Will Properties
        // 3.1.3.3 Will Topic
        // 3.1.3.4 Will Payload

        // 3.1.3.5 User Name
        if let Some(username) = self.username {
            writer.put_str(username)?;
        }

        // 3.1.3.6 Password
        if let Some(password) = self.password {
            writer.put_binary_data(password)?;
        }

        Ok(())
    }
}
