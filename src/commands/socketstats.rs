use std::{
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use color_eyre::Result;
use eyre::{eyre, Context, OptionExt};
use log::{debug, info};
use netlink_packet_core::{NetlinkHeader, NetlinkMessage, NetlinkPayload, NLM_F_DUMP, NLM_F_REQUEST};
use netlink_packet_sock_diag::{
    constants::*,
    inet::{
        nlas::{Nla, TcpInfo},
        ExtensionFlags, InetRequest, InetResponse, SocketId, StateFlags,
    },
    SockDiagMessage,
};
use netlink_sys::{protocols::NETLINK_SOCK_DIAG, Socket, SocketAddr};

use crate::{
    cli::opt::SocketstatsArgs,
    phork::namespace::{self, Namespace, NS_NAME_LINK},
};

const SLEEP_DUR: Duration = Duration::from_millis(250);

/// Runs the socket stats collector and exports the received statistics to a .csv file.
pub fn run(socketstats_args: SocketstatsArgs) -> Result<()> {
    // check if network environment is set up
    if !crate::phork::namespace::is_setup()? {
        info!("Network environment is not set up. Setting up...");
        namespace::setup().map_err(|e| eyre::eyre!("Failed to set up network environment: {}", e))?;
    }

    // switch to the link namespace
    info!("Switching to namespace: '{}'", NS_NAME_LINK);
    Namespace::try_load(NS_NAME_LINK)
        .map_err(|e| eyre::eyre!("Failed to load namespace '{}': {}", NS_NAME_LINK, e))?
        .try_switch_calling_pid_to_namespace()
        .map_err(|e| eyre::eyre!("Failed to switch to namespace '{}': {}", NS_NAME_LINK, e))?;

    let socket = connect_socket().unwrap();
    info!("Running socketstats until receiving Ctrl-C...");

    let inet_response = wait_until_socket_exists(&socket)?;
    let client_socket_id = inet_response.header.socket_id.clone();
    info!(
        "Found iperf client socket running {}: {client_socket_id:?}",
        extract_cong_alg(&inet_response)?
    );

    let mut wtr = csv::Writer::from_path(socketstats_args.output_file)?;
    wtr.write_record(FORMAT_TCP_INFO_HEADER)?;

    let tcp_info = extract_tcp_info(inet_response)?;
    wtr.write_record(format_tcp_info_record(&tcp_info)?)?;

    loop {
        let mut received_msgs = fetch_socket_stats(&socket, generate_sock_diag_msg(Some(&client_socket_id)))?;

        match received_msgs.len() {
            0 => {
                info!("Received empty response (socket closed). Terminating.");
                break;
            }
            1 => {
                let inet_response = received_msgs.pop().ok_or_eyre("received_msgs is empty")?;
                let tcp_info = extract_tcp_info(inet_response)?;
                wtr.write_record(format_tcp_info_record(&tcp_info)?)?;
            }
            _ => unreachable!("there should be only one message per socket id"),
        }

        sleep(SLEEP_DUR);
    }
    wtr.flush()?;
    Ok(())
}

/// Creates a new socket and connects to the kernel using the sock_diag netlink subsystem.
fn connect_socket() -> Result<Socket> {
    let mut socket = Socket::new(NETLINK_SOCK_DIAG)?;
    socket.bind_auto()?;
    socket.connect(&SocketAddr::new(0, 0))?;
    Ok(socket)
}

/// Generates a new sock_diag request message.
/// If client_socket_id = None, a list of sockets is requested. Otherwise, create a query about this individual socket (typically the id of the iperf client).
/// For both, request the used congestion control algorithm (INET_DIAG_CONG) as extended information report.
/// When querying an individual socket, also request a tcp_info struct (INET_DIAG_INFO).
fn generate_sock_diag_msg(socket_id: Option<&SocketId>) -> NetlinkMessage<SockDiagMessage> {
    let mut nl_hdr = NetlinkHeader::default();
    nl_hdr.flags = NLM_F_REQUEST | NLM_F_DUMP;

    let mut packet = NetlinkMessage::new(
        nl_hdr,
        SockDiagMessage::InetRequest(InetRequest {
            family: AF_INET,
            protocol: IPPROTO_TCP,
            extensions: ExtensionFlags::INFO | ExtensionFlags::CONG,
            states: StateFlags::ESTABLISHED,
            socket_id: socket_id.map_or_else(SocketId::new_v4, |x| x.to_owned()),
        })
        .into(),
    );

    packet.finalize();
    packet
}

/// Sends a request message to the kernel via the given socket and returns the received response.
fn fetch_socket_stats(socket: &Socket, packet: NetlinkMessage<SockDiagMessage>) -> Result<Vec<InetResponse>> {
    send_packet(socket, packet).inspect_err(|e| info!("Send Error {e}"))?;
    receive_response(socket).inspect_err(|e| info!("Receive Error {e}"))
}

/// Sends the given sock_diag request via the given socket.
fn send_packet(socket: &Socket, packet: NetlinkMessage<SockDiagMessage>) -> Result<usize> {
    let mut buf = vec![0; packet.header.length as usize];
    // Before calling serialize, it is important to check that the buffer in which we're emitting is big enough for the packet, otherwise `serialize()` panics.
    assert_eq!(buf.len(), packet.buffer_len());
    packet.serialize(&mut buf[..]);

    debug!(">>> {packet:?}");
    socket.send(&buf[..], 0).wrap_err("Failed to send the message through the socket")
}

/// Receives from the socket until a DoneMessage is received and returns the received messages.
fn receive_response(socket: &Socket) -> Result<Vec<InetResponse>> {
    let mut receive_buffer = vec![0; 4096];
    let mut offset = 0;
    let mut results = Vec::new();

    while let Ok(size) = socket.recv(&mut &mut receive_buffer[..], 0) {
        loop {
            let bytes = &receive_buffer[offset..];
            let rx_packet = <NetlinkMessage<SockDiagMessage>>::deserialize(bytes)?;
            debug!("<<< {rx_packet:?}");

            match rx_packet.payload {
                NetlinkPayload::Noop => {}
                NetlinkPayload::InnerMessage(SockDiagMessage::InetResponse(response)) => {
                    results.push(*response);
                }
                NetlinkPayload::Done(_) => {
                    return Ok(results);
                }
                NetlinkPayload::Error(msg) => {
                    return Err(msg.to_io().into());
                }
                _ => todo!(),
            }

            offset += rx_packet.header.length as usize;
            if offset == size || rx_packet.header.length == 0 {
                offset = 0;
                break;
            }
        }
    }
    unreachable!()
}

/// Repeatedly polls the kernel for a list of established sockets until the socket of the iperf client is found.
/// The iperf client socket is detected as being the only established socket with a running retransmit timer.
/// If multiple matching sockets are found, this method fails.
fn wait_until_socket_exists(socket: &Socket) -> Result<InetResponse> {
    loop {
        let received_msgs = fetch_socket_stats(socket, generate_sock_diag_msg(None))?;
        let mut received_msgs = received_msgs
            .into_iter()
            .filter(|inet_response| inet_response.header.socket_id.source_port == 54321)
            .collect::<Vec<_>>();

        match received_msgs.len() {
            0 => {} // continue poll & wait
            1 => {
                let inet_response = received_msgs.pop().ok_or_eyre("received_msgs is empty but should have length 1")?;
                return Ok(inet_response);
            }
            _ => {
                return Err(eyre!(
                    "Found multiple matching sockets: {:?}",
                    received_msgs
                        .into_iter()
                        .map(|inet_response| inet_response.header.socket_id)
                        .collect::<Vec<_>>()
                ));
            }
        }
        sleep(SLEEP_DUR);
    }
}

/// Extracts the congestion control algorithm from the given response.
fn extract_cong_alg(inet_response: &InetResponse) -> Result<String> {
    inet_response
        .nlas
        .iter()
        .find_map(|nla| {
            if let Nla::Congestion(cong) = nla {
                Some(cong.to_owned())
            } else {
                None
            }
        })
        .ok_or_eyre("InetResponse does not contain TcpInfo")
}

/// Extracts the tcp_info struct from the given response.
fn extract_tcp_info(inet_response: InetResponse) -> Result<TcpInfo> {
    inet_response
        .nlas
        .into_iter()
        .find_map(|x| if let Nla::TcpInfo(tcp) = x { Some(tcp) } else { None })
        .ok_or_eyre("InetResponse does not contain TcpInfo")
}

/// Formats a TCP CA state as human-readable string.
fn tcp_ca_state_to_string(tcp_ca_state: u8) -> Result<&'static str> {
    match tcp_ca_state {
        TCP_CA_OPEN => Ok("TCP_CA_OPEN"),
        TCP_CA_DISORDER => Ok("TCP_CA_DISORDER"),
        TCP_CA_CWR => Ok("TCP_CA_CWR"),
        TCP_CA_RECOVERY => Ok("TCP_CA_RECOVERY"),
        TCP_CA_LOSS => Ok("TCP_CA_LOSS"),
        _ => Err(eyre!("Unknown tcp ca state: {tcp_ca_state}")),
    }
}

/// Header fields for the tcp_info struct used in the csv export.
const FORMAT_TCP_INFO_HEADER: [&str; 8] = [
    "timestamp",
    "ca_state",
    "rto[ms]",
    "rtt[ms]",
    "rttvar[ms2]",
    "snd_ssthresh[B]",
    "snd_cwnd[B]",
    "min_rtt[ms]",
];

/// Formats (a subset of) a tcp_info struct as a list of strings for csv export.
fn format_tcp_info_record(tcp_info: &TcpInfo) -> Result<[String; 8]> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

    Ok([
        now.to_string(),
        tcp_ca_state_to_string(tcp_info.ca_state)?.to_string(),
        (tcp_info.rto / 1000).to_string(),                      // [µs] -> [ms]
        (tcp_info.rtt / 1000).to_string(),                      // [µs] -> [ms]
        (tcp_info.rttvar / 1000).to_string(),                   // [µs] -> [ms]
        (tcp_info.snd_ssthresh * tcp_info.snd_mss).to_string(), // snd_ssthresh is given in number of segments
        (tcp_info.snd_cwnd * tcp_info.snd_mss).to_string(),     // snd_cwnd is given in number of segments
        (tcp_info.min_rtt / 1000).to_string(),                  // [µs] -> [ms]
    ])
}
