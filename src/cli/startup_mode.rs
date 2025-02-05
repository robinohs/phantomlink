use clap::{builder::PossibleValue, ValueEnum};
use etherparse::PacketHeaders;
use eyre::Result;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum StartupMode {
    AppStart,
    FirstPacket,
    FirstTransportPacket,
    FirstLargeTransportPacket,
}

// Can also be derived with feature flag `derive`
impl ValueEnum for StartupMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            StartupMode::AppStart,
            StartupMode::FirstPacket,
            StartupMode::FirstTransportPacket,
            StartupMode::FirstLargeTransportPacket,
        ]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        Some(match self {
            StartupMode::AppStart => PossibleValue::new("app-start").help("phantomlink immediately starts following input."),
            StartupMode::FirstPacket => {
                PossibleValue::new("first-packet").help("phantomlink starts following input after arrival of first packet")
            }
            StartupMode::FirstTransportPacket => {
                PossibleValue::new("first-transport-packet").help("phantomlink starts following input after first TCP/UDP packet.")
            }
            StartupMode::FirstLargeTransportPacket => PossibleValue::new("first-large-transport-packet")
                .help("phantomlink starts following input after first TCP/UDP packet with >1500 bytes."),
        })
    }
}

impl std::fmt::Display for StartupMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value().expect("no values are skipped").get_name().fmt(f)
    }
}

impl std::str::FromStr for StartupMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for variant in Self::value_variants() {
            if variant.to_possible_value().unwrap().matches(s, false) {
                return Ok(*variant);
            }
        }
        Err(format!("invalid variant: {s}"))
    }
}

impl StartupMode {
    pub fn check_packet_satisfies_constraint(self, data: &[u8]) -> bool {
        // if first packet is trigger, immediately return true
        if self == StartupMode::FirstPacket || self == StartupMode::AppStart {
            return true;
        }

        // check if minimal size is satisfied
        if data.len() < self.get_min_size() {
            return false;
        }

        // get headers
        let headers = match PacketHeaders::from_ethernet_slice(data) {
            Ok(header) => header,
            Err(_) => return false,
        };

        // get transport_headers
        let transport_headers = match headers.transport {
            Some(transport_headers) => transport_headers,
            None => return false,
        };

        // check if packet is TCP or UDP
        match transport_headers {
            etherparse::TransportHeader::Udp(_) | etherparse::TransportHeader::Tcp(_) => true,
            etherparse::TransportHeader::Icmpv4(_) | etherparse::TransportHeader::Icmpv6(_) => false,
        }
    }

    fn get_min_size(&self) -> usize {
        match self {
            StartupMode::FirstTransportPacket => 0,
            StartupMode::FirstLargeTransportPacket => 1500,
            StartupMode::AppStart | StartupMode::FirstPacket => unreachable!(),
        }
    }
}

#[cfg(test)]
pub mod test {
    use etherparse::PacketBuilder;

    use super::*;

    #[test]
    fn test_startup_app_startup() {
        let small_data: &[u8] = &[0u8; 200];
        let large_data: &[u8] = &[0u8; 1500];

        assert!(StartupMode::AppStart.check_packet_satisfies_constraint(small_data));
        assert!(StartupMode::AppStart.check_packet_satisfies_constraint(large_data));
    }

    #[test]
    fn test_startup_first_pkt() {
        let small_data: &[u8] = &[0u8; 200];
        let large_data: &[u8] = &[0u8; 1500];

        assert!(StartupMode::FirstPacket.check_packet_satisfies_constraint(small_data));
        assert!(StartupMode::FirstPacket.check_packet_satisfies_constraint(large_data));
    }

    #[test]
    fn test_startup_first_transport_pkt() {
        let tcp_small_data: Vec<u8> = create_tcp_packet(500);
        let tcp_large_data: Vec<u8> = create_tcp_packet(1500);
        let icmp_small_data: Vec<u8> = create_icmp_packet(500);
        let icmp_large_data: Vec<u8> = create_icmp_packet(1500);

        assert!(StartupMode::FirstTransportPacket.check_packet_satisfies_constraint(&tcp_small_data));
        assert!(StartupMode::FirstTransportPacket.check_packet_satisfies_constraint(&tcp_large_data));
        assert!(!StartupMode::FirstTransportPacket.check_packet_satisfies_constraint(&icmp_small_data));
        assert!(!StartupMode::FirstTransportPacket.check_packet_satisfies_constraint(&icmp_large_data));
    }

    #[test]
    fn test_startup_first_large_transport_pkt() {
        let tcp_small_data: Vec<u8> = create_tcp_packet(500);
        let tcp_large_data: Vec<u8> = create_tcp_packet(1500);
        let icmp_small_data: Vec<u8> = create_icmp_packet(500);
        let icmp_large_data: Vec<u8> = create_icmp_packet(1500);

        assert!(!StartupMode::FirstLargeTransportPacket.check_packet_satisfies_constraint(&tcp_small_data));
        assert!(StartupMode::FirstLargeTransportPacket.check_packet_satisfies_constraint(&tcp_large_data));
        assert!(!StartupMode::FirstLargeTransportPacket.check_packet_satisfies_constraint(&icmp_small_data));
        assert!(!StartupMode::FirstLargeTransportPacket.check_packet_satisfies_constraint(&icmp_large_data));
    }

    /// taken from https://github.com/JulianSchmid/etherparse/blob/master/etherparse/examples/write_tcp.rs
    fn create_tcp_packet(payload_length: usize) -> Vec<u8> {
        let builder = PacketBuilder::ethernet2(
            [1, 2, 3, 4, 5, 6],    //source mac
            [7, 8, 9, 10, 11, 12], //destination mac
        )
        .ipv4(
            [192, 168, 1, 1], //source ip
            [192, 168, 1, 2], //destination ip
            20,               //time to life
        )
        .tcp(
            21,    //source port
            1234,  //desitnation port
            1,     //sequence number
            26180, //window size
        );
        let payload: Vec<u8> = (0..payload_length).map(|_| 1u8).collect();
        let mut result = Vec::<u8>::with_capacity(builder.size(payload.len()));
        builder.write(&mut result, &payload).unwrap();
        result
    }

    fn create_icmp_packet(payload_length: usize) -> Vec<u8> {
        let builder = PacketBuilder::ethernet2(
            [1, 2, 3, 4, 5, 6],    //source mac
            [7, 8, 9, 10, 11, 12], //destination mac
        )
        .ipv4(
            [192, 168, 1, 1], //source ip
            [192, 168, 1, 2], //destination ip
            20,               //time to life
        )
        .icmpv4(etherparse::Icmpv4Type::Unknown {
            type_u8: 1,
            code_u8: 2,
            bytes5to8: [1u8, 2u8, 3u8, 1u8],
        });
        let payload: Vec<u8> = (0..payload_length).map(|_| 1u8).collect();
        let mut result = Vec::<u8>::with_capacity(builder.size(payload.len()));
        builder.write(&mut result, &payload).unwrap();
        result
    }
}
