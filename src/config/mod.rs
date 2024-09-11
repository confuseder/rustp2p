use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use crate::pipe::{NodeAddress, PeerNodeAddress};
use crate::protocol::node_id::NodeID;
use async_trait::async_trait;
use bytes::{Buf, BytesMut};
use rust_p2p_core::pipe::tcp_pipe::{Decoder, Encoder, InitCodec};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

pub(crate) mod punch_info;
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum Model {
    High,
    #[default]
    Low,
}

#[derive(Clone, Debug, Default)]
pub struct LocalInterface {
    pub index: u32,
    #[cfg(unix)]
    pub name: Option<String>,
}

impl LocalInterface {
    pub fn new(index: u32, #[cfg(unix)] name: Option<String>) -> Self {
        Self {
            index,
            #[cfg(unix)]
            name,
        }
    }
}

pub(crate) const ROUTE_IDLE_TIME: Duration = Duration::from_secs(10);

pub struct PipeConfig {
    pub first_latency: bool,
    pub multi_pipeline: usize,
    pub route_idle_time: Duration,
    pub udp_pipe_config: Option<UdpPipeConfig>,
    pub tcp_pipe_config: Option<TcpPipeConfig>,
    pub enable_extend: bool,
    pub self_id: Option<NodeID>,
    pub direct_addrs: Option<Vec<PeerNodeAddress>>,
    pub send_buffer_size: usize,
    pub query_id_interval: Duration,
    pub query_id_max_num: usize,
    pub heartbeat_interval: Duration,
    pub tcp_stun_servers: Option<Vec<String>>,
    pub udp_stun_servers: Option<Vec<String>>,
    pub mapping_addrs: Option<Vec<NodeAddress>>,
    pub dns: Option<Vec<String>>,
}

impl Default for PipeConfig {
    fn default() -> Self {
        Self {
            first_latency: false,
            multi_pipeline: MULTI_PIPELINE,
            enable_extend: false,
            udp_pipe_config: Some(Default::default()),
            tcp_pipe_config: Some(Default::default()),
            route_idle_time: ROUTE_IDLE_TIME,
            self_id: None,
            direct_addrs: None,
            send_buffer_size: 2048,
            query_id_interval: Duration::from_secs(12),
            query_id_max_num: 5,
            heartbeat_interval: Duration::from_secs(5),
            tcp_stun_servers: Some(vec![
                "stun.flashdance.cx".to_string(),
                "stun.sipnet.net".to_string(),
                "stun.nextcloud.com:443".to_string(),
            ]),
            udp_stun_servers: Some(vec![
                "stun.miwifi.com".to_string(),
                "stun.chat.bilibili.com".to_string(),
                "stun.hitv.com".to_string(),
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
                "stun2.l.google.com:19302".to_string(),
            ]),
            mapping_addrs: None,
            dns: None,
        }
    }
}

pub(crate) const MULTI_PIPELINE: usize = 2;
pub(crate) const UDP_SUB_PIPELINE_NUM: usize = 82;

impl PipeConfig {
    pub fn none_tcp(self) -> Self {
        self
    }
}

impl PipeConfig {
    pub fn empty() -> Self {
        Self::default()
    }
    pub fn set_first_latency(mut self, first_latency: bool) -> Self {
        self.first_latency = first_latency;
        self
    }
    pub fn set_main_pipeline_num(mut self, main_pipeline_num: usize) -> Self {
        self.multi_pipeline = main_pipeline_num;
        self
    }
    pub fn set_enable_extend(mut self, enable_extend: bool) -> Self {
        self.enable_extend = enable_extend;
        self
    }
    pub fn set_udp_pipe_config(mut self, udp_pipe_config: UdpPipeConfig) -> Self {
        self.udp_pipe_config.replace(udp_pipe_config);
        self
    }
    pub fn set_tcp_pipe_config(mut self, tcp_pipe_config: TcpPipeConfig) -> Self {
        self.tcp_pipe_config.replace(tcp_pipe_config);
        self
    }
    pub fn set_node_id(mut self, self_id: NodeID) -> Self {
        self.self_id.replace(self_id);
        self
    }
    pub fn set_direct_addrs(mut self, direct_addrs: Vec<PeerNodeAddress>) -> Self {
        self.direct_addrs.replace(direct_addrs);
        self
    }
    pub fn set_send_buffer_size(mut self, send_buffer_size: usize) -> Self {
        self.send_buffer_size = send_buffer_size;
        self
    }
    pub fn set_query_id_interval(mut self, query_id_interval: Duration) -> Self {
        self.query_id_interval = query_id_interval;
        self
    }
    pub fn set_query_id_max_num(mut self, query_id_max_num: usize) -> Self {
        self.query_id_max_num = query_id_max_num;
        self
    }
    pub fn set_heartbeat_interval(mut self, heartbeat_interval: Duration) -> Self {
        self.heartbeat_interval = heartbeat_interval;
        self
    }
    pub fn set_tcp_stun_servers(mut self, tcp_stun_servers: Vec<String>) -> Self {
        self.tcp_stun_servers.replace(tcp_stun_servers);
        self
    }
    pub fn set_udp_stun_servers(mut self, udp_stun_servers: Vec<String>) -> Self {
        self.udp_stun_servers.replace(udp_stun_servers);
        self
    }
    pub fn set_mapping_addrs(mut self, mapping_addrs: Vec<NodeAddress>) -> Self {
        self.mapping_addrs.replace(mapping_addrs);
        self
    }
    pub fn set_dns(mut self, dns: Vec<String>) -> Self {
        self.dns.replace(dns);
        self
    }
}

pub struct TcpPipeConfig {
    pub route_idle_time: Duration,
    pub tcp_multiplexing_limit: usize,
    pub default_interface: Option<LocalInterface>,
    pub tcp_port: u16,
    pub use_v6: bool,
}

impl Default for TcpPipeConfig {
    fn default() -> Self {
        Self {
            route_idle_time: ROUTE_IDLE_TIME,
            tcp_multiplexing_limit: MULTI_PIPELINE,
            default_interface: None,
            tcp_port: 0,
            use_v6: true,
        }
    }
}

impl TcpPipeConfig {
    pub fn set_tcp_multiplexing_limit(mut self, tcp_multiplexing_limit: usize) -> Self {
        self.tcp_multiplexing_limit = tcp_multiplexing_limit;
        self
    }
    pub fn set_route_idle_time(mut self, route_idle_time: Duration) -> Self {
        self.route_idle_time = route_idle_time;
        self
    }
    pub fn set_default_interface(mut self, default_interface: LocalInterface) -> Self {
        self.default_interface = Some(default_interface.clone());
        self
    }
    pub fn set_tcp_port(mut self, tcp_port: u16) -> Self {
        self.tcp_port = tcp_port;
        self
    }
    pub fn set_use_v6(mut self, use_v6: bool) -> Self {
        self.use_v6 = use_v6;
        self
    }
}

#[derive(Clone)]
pub struct UdpPipeConfig {
    pub main_pipeline_num: usize,
    pub sub_pipeline_num: usize,
    pub model: Model,
    pub default_interface: Option<LocalInterface>,
    pub udp_ports: Vec<u16>,
    pub use_v6: bool,
}

impl Default for UdpPipeConfig {
    fn default() -> Self {
        Self {
            main_pipeline_num: MULTI_PIPELINE,
            sub_pipeline_num: UDP_SUB_PIPELINE_NUM,
            model: Model::Low,
            default_interface: None,
            udp_ports: vec![0, 0],
            use_v6: true,
        }
    }
}

impl UdpPipeConfig {
    pub fn set_main_pipeline_num(mut self, main_pipeline_num: usize) -> Self {
        self.main_pipeline_num = main_pipeline_num;
        self
    }
    pub fn set_sub_pipeline_num(mut self, sub_pipeline_num: usize) -> Self {
        self.sub_pipeline_num = sub_pipeline_num;
        self
    }
    pub fn set_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }
    pub fn set_default_interface(mut self, default_interface: LocalInterface) -> Self {
        self.default_interface = Some(default_interface.clone());
        self
    }
    pub fn set_udp_ports(mut self, udp_ports: Vec<u16>) -> Self {
        self.udp_ports = udp_ports;
        self
    }
    pub fn set_simple_udp_port(mut self, udp_port: u16) -> Self {
        self.udp_ports = vec![udp_port];
        self
    }
    pub fn set_use_v6(mut self, use_v6: bool) -> Self {
        self.use_v6 = use_v6;
        self
    }
}

impl From<Model> for rust_p2p_core::pipe::udp_pipe::Model {
    fn from(value: Model) -> Self {
        match value {
            Model::High => rust_p2p_core::pipe::udp_pipe::Model::High,
            Model::Low => rust_p2p_core::pipe::udp_pipe::Model::Low,
        }
    }
}

impl From<LocalInterface> for rust_p2p_core::socket::LocalInterface {
    fn from(value: LocalInterface) -> Self {
        #[cfg(unix)]
        return rust_p2p_core::socket::LocalInterface::new(value.index, value.name);
        #[cfg(not(unix))]
        rust_p2p_core::socket::LocalInterface::new(value.index)
    }
}

impl From<PipeConfig> for rust_p2p_core::pipe::config::PipeConfig {
    fn from(value: PipeConfig) -> Self {
        rust_p2p_core::pipe::config::PipeConfig {
            first_latency: value.first_latency,
            multi_pipeline: value.multi_pipeline,
            route_idle_time: value.route_idle_time,
            udp_pipe_config: value.udp_pipe_config.map(|v| v.into()),
            tcp_pipe_config: value.tcp_pipe_config.map(|v| v.into()),
            enable_extend: value.enable_extend,
        }
    }
}

impl From<UdpPipeConfig> for rust_p2p_core::pipe::config::UdpPipeConfig {
    fn from(value: UdpPipeConfig) -> Self {
        rust_p2p_core::pipe::config::UdpPipeConfig {
            main_pipeline_num: value.main_pipeline_num,
            sub_pipeline_num: value.sub_pipeline_num,
            model: value.model.into(),
            default_interface: value.default_interface.map(|v| v.into()),
            udp_ports: value.udp_ports,
            use_v6: value.use_v6,
        }
    }
}

impl From<TcpPipeConfig> for rust_p2p_core::pipe::config::TcpPipeConfig {
    fn from(value: TcpPipeConfig) -> Self {
        rust_p2p_core::pipe::config::TcpPipeConfig {
            route_idle_time: value.route_idle_time,
            tcp_multiplexing_limit: value.tcp_multiplexing_limit,
            default_interface: value.default_interface.map(|v| v.into()),
            tcp_port: value.tcp_port,
            use_v6: value.use_v6,
            init_codec: Box::new(LengthPrefixedInitCodec),
        }
    }
}

/// Fixed-length prefix encoder/decoder.
pub(crate) struct LengthPrefixedEncoder {}
pub(crate) struct LengthPrefixedDecoder {
    offset: usize,
    buf: BytesMut,
}
impl LengthPrefixedEncoder {
    pub(crate) fn new() -> Self {
        Self {}
    }
}
impl LengthPrefixedDecoder {
    pub(crate) fn new() -> Self {
        Self {
            offset: 0,
            buf: Default::default(),
        }
    }
}

#[async_trait]
impl Decoder for LengthPrefixedDecoder {
    async fn decode(&mut self, read: &mut OwnedReadHalf, src: &mut [u8]) -> io::Result<usize> {
        let len = src.len() + 2;
        if self.buf.len() < len {
            self.buf.reserve(len - self.buf.len());
            unsafe {
                self.buf.set_len(len);
            }
        }
        while self.offset < 2 {
            let len = read.read(&mut self.buf[self.offset..]).await?;
            self.offset += len;
        }
        let data_len = ((self.buf[0] as usize) << 8) | self.buf[1] as usize;
        if data_len > src.len() {
            return Err(io::Error::from(io::ErrorKind::OutOfMemory));
        }
        let len = data_len + 2;
        loop {
            if len == self.offset {
                src[..data_len].copy_from_slice(&self.buf[2..len]);
                self.offset = 0;
                return Ok(data_len);
            }
            if len < self.offset {
                src[..data_len].copy_from_slice(&self.buf[2..len]);
                self.buf.advance(len);
                self.offset -= len;
                return Ok(data_len);
            }
            let len = read.read(&mut self.buf[self.offset..]).await?;
            self.offset += len;
        }
    }
}

#[async_trait]
impl Encoder for LengthPrefixedEncoder {
    async fn encode(&mut self, write: &mut OwnedWriteHalf, data: &[u8]) -> io::Result<usize> {
        if data.len() > u16::MAX as usize {
            return Err(io::Error::from(io::ErrorKind::OutOfMemory));
        }
        let head = (data.len() as u16).to_be_bytes();
        let bufs: &[_] = &[io::IoSlice::new(&head), io::IoSlice::new(data)];
        let len = data.len() + 2;
        let w = write.write_vectored(bufs).await?;
        if w < 2 {
            write.write_all(&head[w..]).await?;
            write.write_all(data).await?;
        } else if w != len {
            write.write_all(&data[w - 2..]).await?
        }
        Ok(data.len())
    }
}

pub(crate) struct LengthPrefixedInitCodec;

impl InitCodec for LengthPrefixedInitCodec {
    fn codec(&self, _addr: SocketAddr) -> io::Result<(Box<dyn Decoder>, Box<dyn Encoder>)> {
        Ok((
            Box::new(LengthPrefixedDecoder::new()),
            Box::new(LengthPrefixedEncoder::new()),
        ))
    }
}
