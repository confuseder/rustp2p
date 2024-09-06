use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Arc;

use clap::Parser;
use env_logger::Env;
use tun_rs::AsyncDevice;

use rustp2p::config::{PipeConfig, TcpPipeConfig, UdpPipeConfig};
use rustp2p::error::*;
use rustp2p::pipe::{
    HandleError, HandleResult, NodeAddress, Pipe, PipeLine, PipeWriter, RecvError,
};
use rustp2p::protocol::node_id::NodeID;
use rustp2p::protocol::protocol_type::ProtocolType;
use rustp2p::protocol::{Builder, NetPacket};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Peer node address.
    /// example: --peer tcp://192.168.10.13:23333 --peer udp://192.168.10.23:23333
    #[arg(short, long)]
    peer: Option<Vec<String>>,
    /// Local node IP and mask.
    /// example: --local 10.26.0.2/24
    #[arg(short, long)]
    local: String,
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let Args { peer, local } = Args::parse();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let mut split = local.split("/");
    let self_id = Ipv4Addr::from_str(split.next().expect("--local error")).expect("--local error");
    let mask = u8::from_str(split.next().expect("--local error")).expect("--local error");
    let mut addrs = Vec::new();
    if let Some(peers) = peer {
        for addr in peers {
            if let Some(tcp_addr) = addr.strip_prefix("tcp://") {
                addrs.push(NodeAddress::Tcp(tcp_addr.parse().expect("--peer error")));
            } else if let Some(tcp_addr) = addr.strip_prefix("udp://") {
                addrs.push(NodeAddress::Udp(tcp_addr.parse().expect("--peer error")));
            } else {
                panic!("--peer error")
            }
        }
    }
    let device = tun_rs::create_as_async(
        tun_rs::Configuration::default()
            .address_with_prefix(self_id, mask)
            .platform_config(|v| {
                #[cfg(windows)]
                v.ring_capacity(2 * 1024 * 1024);
            })
            .up(),
    )
    .unwrap();
    #[cfg(target_os = "macos")]
    device.set_ignore_packet_info(true);
    let device = Arc::new(device);
    let udp_config = UdpPipeConfig::default().set_udp_ports(vec![23333, 23334]);
    let tcp_config = TcpPipeConfig::default().set_tcp_port(23333);
    let config = PipeConfig::empty()
        .set_udp_pipe_config(udp_config)
        .set_tcp_pipe_config(tcp_config)
        .set_direct_addrs(addrs)
        .set_node_id(self_id.into());

    let mut pipe = Pipe::new(config).await?;
    let writer = pipe.writer();
    let device_r = device.clone();
    tokio::spawn(async move {
        tun_recv(writer, device_r).await.unwrap();
    });
    log::info!("listen 23333");
    loop {
        let line = pipe.accept().await?;
        let device = device.clone();
        tokio::spawn(recv(line, device));
    }
}
async fn recv(mut line: PipeLine, device: Arc<AsyncDevice>) {
    let mut buf = [0; 2000];
    loop {
        let rs = match line.recv_from(&mut buf).await {
            Ok(rs) => rs,
            Err(e) => {
                log::warn!("recv_from {e:?}");
                return;
            }
        };
        let handle_rs = match rs {
            Ok(handle_rs) => handle_rs,
            Err(e) => {
                log::warn!("recv_data_handle {e:?}");
                continue;
            }
        };
        match handle_rs {
            HandleResult::Turn(buf, dest_id, route_key) => {
                if let Err(e) = line.send_to(buf.buffer(), &dest_id).await {
                    log::warn!("Turn {e:?},{dest_id:?},{route_key:?}")
                }
            }
            HandleResult::UserData(buf, src_id, route_key) => {
                if let Err(e) = device.send(buf.payload()).await {
                    log::warn!("UserData {e:?},{src_id:?},{route_key:?}")
                }
            }
        }
    }
}
async fn tun_recv(pipe_writer: PipeWriter, device: Arc<AsyncDevice>) -> Result<()> {
    let mut send_packet = pipe_writer.allocate_send_packet()?;
    loop {
        let payload = send_packet.data_mut();
        let payload_len = device.recv(payload).await?;
        if payload[0] >> 4 != 4 {
            continue;
        }
        let dest_ip = Ipv4Addr::new(payload[16], payload[17], payload[18], payload[19]);
        if dest_ip.is_broadcast()
            || dest_ip.is_multicast()
            || dest_ip.is_unspecified()
            || payload[19] == 255
        {
            continue;
        }
        send_packet.set_payload_len(payload_len);
        if let Err(e) = pipe_writer
            .send_to_packet(&mut send_packet, &dest_ip.into())
            .await
        {
            log::warn!("{e:?},{dest_ip:?}")
        }
    }
}