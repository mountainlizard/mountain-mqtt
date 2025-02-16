use super::packet::{Packet, PacketRead, PacketWrite};
use crate::data::{
    packet_identifier::PacketIdentifier,
    packet_type::PacketType,
    property::SubscribeProperty,
    quality_of_service::QualityOfService,
    subscription_options::{RetainHandling, SubscriptionOptions},
};
use crate::{
    codec::{
        mqtt_reader::{self, MqttReader},
        mqtt_writer::{self, MqttWriter},
        read::Read,
    },
    error::PacketReadError,
};

use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct SubscriptionRequest<'a> {
    pub topic_name: &'a str,
    pub options: SubscriptionOptions,
}

impl<'a> SubscriptionRequest<'a> {
    pub fn new(topic_name: &'a str, maximum_qos: QualityOfService) -> SubscriptionRequest<'a> {
        SubscriptionRequest {
            topic_name,
            options: SubscriptionOptions {
                maximum_qos,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendOnSubscribe,
            },
        }
    }
}

impl<'a> Read<'a> for SubscriptionRequest<'a> {
    fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        reader.get_subscription_request()
    }
}

#[derive(Debug, PartialEq)]
pub struct Subscribe<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> {
    packet_identifier: PacketIdentifier,
    primary_request: SubscriptionRequest<'a>,
    additional_requests: Vec<SubscriptionRequest<'a>, REQUEST_N>,
    properties: Vec<SubscribeProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> Subscribe<'a, PROPERTIES_N, REQUEST_N> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        primary_request: SubscriptionRequest<'a>,
        additional_requests: Vec<SubscriptionRequest<'a>, REQUEST_N>,
        properties: Vec<SubscribeProperty<'a>, PROPERTIES_N>,
    ) -> Self {
        Self {
            packet_identifier,
            primary_request,
            additional_requests,
            properties,
        }
    }
}

impl<const PROPERTIES_N: usize, const REQUEST_N: usize> Packet
    for Subscribe<'_, PROPERTIES_N, REQUEST_N>
{
    fn packet_type(&self) -> PacketType {
        PacketType::Subscribe
    }
}

impl<const PROPERTIES_N: usize, const REQUEST_N: usize> PacketWrite
    for Subscribe<'_, PROPERTIES_N, REQUEST_N>
{
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:
        writer.put_u16(self.packet_identifier.0)?; // 3.8.2 SUBSCRIBE Variable Header
        writer.put_variable_u32_delimited_vec(&self.properties)?; //3.8.2.1 SUBSCRIBE Properties

        // Payload:
        writer.put_subscription_request(&self.primary_request)?;
        // Note we just put the requests in without a delimiter, they end at the end of the packet
        for r in self.additional_requests.iter() {
            writer.put_subscription_request(r)?;
        }

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> PacketRead<'a>
    for Subscribe<'a, PROPERTIES_N, REQUEST_N>
{
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // The payload is a concatenated list of subscription requests, we need to
        // know the end position to know where to stop
        let payload_end_position = reader.position() + len;

        // Variable header:
        let packet_identifier = PacketIdentifier(reader.get_u16()?);
        let mut properties = Vec::new();
        reader.get_property_list(&mut properties)?;

        // Payload:
        // We must have at least one subscription request, otherwise this is a protocol
        // error [MQTT-3.8.3-2]
        let primary_request = reader
            .get_subscription_request()
            .map_err(|_| PacketReadError::SubscribeWithoutValidSubscriptionRequest)?;

        // Read additional subscription requests until we run out of data
        let mut additional_requests = Vec::new();
        while reader.position() < payload_end_position {
            let additional_request = SubscriptionRequest::read(reader)?;
            additional_requests
                .push(additional_request)
                .map_err(|_e| PacketReadError::TooManyRequests)?;
        }

        let packet = Subscribe::new(
            packet_identifier,
            primary_request,
            additional_requests,
            properties,
        );
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{
        mqtt_reader::MqttBufReader,
        mqtt_writer::{MqttBufWriter, MqttLenWriter},
        read::Read,
        write::Write,
    };

    use super::*;

    pub const SUBSCRIPTION_OPTIONS_CASES: [(SubscriptionOptions, u8); 7] = [
        (
            SubscriptionOptions {
                maximum_qos: QualityOfService::QoS0,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendOnSubscribe,
            },
            0b0000_0000,
        ),
        (
            SubscriptionOptions {
                maximum_qos: QualityOfService::QoS1,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendOnSubscribe,
            },
            0b0000_0001,
        ),
        (
            SubscriptionOptions {
                maximum_qos: QualityOfService::QoS2,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendOnSubscribe,
            },
            0b0000_0010,
        ),
        (
            SubscriptionOptions {
                maximum_qos: QualityOfService::QoS0,
                no_local: true,
                retain_as_published: false,
                retain_handling: RetainHandling::SendOnSubscribe,
            },
            0b0000_0100,
        ),
        (
            SubscriptionOptions {
                maximum_qos: QualityOfService::QoS0,
                no_local: false,
                retain_as_published: true,
                retain_handling: RetainHandling::SendOnSubscribe,
            },
            0b0000_1000,
        ),
        (
            SubscriptionOptions {
                maximum_qos: QualityOfService::QoS0,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::SendOnNewSubscribe,
            },
            0b0001_0000,
        ),
        (
            SubscriptionOptions {
                maximum_qos: QualityOfService::QoS0,
                no_local: false,
                retain_as_published: false,
                retain_handling: RetainHandling::DoNotSend,
            },
            0b0010_0000,
        ),
    ];

    #[test]
    fn mqtt_writers_can_put_subscription_options() -> mqtt_writer::Result<()> {
        for (o, encoded) in SUBSCRIPTION_OPTIONS_CASES.iter() {
            let mut buf = [0xFF];
            {
                let mut r = MqttBufWriter::new(&mut buf);
                let mut rl = MqttLenWriter::new();
                r.put_subscription_options(o)?;
                assert_eq!(1, r.position());
                assert_eq!(0, r.remaining());
                rl.put_subscription_options(o)?;
                assert_eq!(1, rl.position());
            }
            assert_eq!(buf[0], *encoded);
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_subscription_options() -> mqtt_reader::Result<()> {
        for (o, encoded) in SUBSCRIPTION_OPTIONS_CASES.iter() {
            let buf = [*encoded];
            let mut r = MqttBufReader::new(&buf);
            let o_read = r.get_subscription_options()?;
            assert_eq!(1, r.position());
            assert_eq!(0, r.remaining());
            assert_eq!(o, &o_read);
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_invalid_retain_handing_in_subscription_options(
    ) -> mqtt_reader::Result<()> {
        // Bits 4 and 5 set to 1, implies retain handling value 3, the only invalid option
        let buf = [0b0011_0000];
        let mut r = MqttBufReader::new(&buf);
        assert_eq!(
            r.get_subscription_options(),
            Err(PacketReadError::InvalidRetainHandlingValue)
        );

        Ok(())
    }

    fn example_packet<'a>() -> Subscribe<'a, 1, 2> {
        let primary_request = SubscriptionRequest::new("test/topic", QualityOfService::QoS0);
        let mut additional_requests = Vec::new();
        additional_requests
            .push(SubscriptionRequest::new("hehe/#", QualityOfService::QoS1))
            .unwrap();
        let mut properties = Vec::new();
        properties
            .push(SubscribeProperty::SubscriptionIdentifier(2432.into()))
            .unwrap();
        let packet = Subscribe::new(
            PacketIdentifier(5432),
            primary_request,
            additional_requests,
            properties,
        );
        packet
    }

    const EXAMPLE_DATA: [u8; 30] = [
        0x82, 0x1C, 0x15, 0x38, 0x03, 0x0B, 0x80, 0x13,
        // subscription requests
        // first request - len + test/topic
        0x00, 0x0A, 0x74, 0x65, 0x73, 0x74, 0x2f, 0x74, 0x6f, 0x70, 0x69, 0x63,
        // subscription options byte
        0x00, // second request - len + hehe/#
        0x00, 0x06, 0x68, 0x65, 0x68, 0x65, 0x2F, 0x23, // subscription options byte
        0x01,
    ];

    // As for EXAMPLE_DATA, but we have only one request, and we miss out the last byte
    // so we can't read it fully - we want to check this produces a more specific
    // SubscribeWithoutValidSubscriptionRequest error, rather than just InsufficientData
    const EXAMPLE_DATA_TRUNCATED_REQUEST: [u8; 20] = [
        0x82, 0x12, 0x15, 0x38, 0x03, 0x0B, 0x80, 0x13,
        // subscription requests
        // first request - len + test/topic
        0x00, 0x0A, 0x74, 0x65, 0x73, 0x74, 0x2f, 0x74, 0x6f, 0x70, 0x69,
        0x63,
        // subscription options byte missing
    ];

    #[test]
    fn encode_example() {
        let packet = example_packet();

        let mut buf = [0; EXAMPLE_DATA.len()];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(buf[0..len], EXAMPLE_DATA);
    }

    #[test]
    fn decode_example() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA);
        assert_eq!(Subscribe::read(&mut r).unwrap(), example_packet());
    }

    #[test]
    fn decode_should_fail_on_truncated_request() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA_TRUNCATED_REQUEST);
        let result: Result<Subscribe<'_, 16, 16>, PacketReadError> = Subscribe::read(&mut r);
        assert_eq!(
            result,
            Err(PacketReadError::SubscribeWithoutValidSubscriptionRequest)
        );
    }
}
