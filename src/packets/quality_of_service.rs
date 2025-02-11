#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QualityOfService {
    QoS0 = 0,
    QoS1 = 1,
    QoS2 = 2,
}
