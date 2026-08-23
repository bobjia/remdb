// 跨平台UDP套接字封装

use super::PubSubError;
use super::Result;
use super::UdpMode;

// 跨平台UDP套接字 trait
pub trait UdpSocketImpl: Send + Sync {
    // 初始化套接字
    fn init(&mut self) -> Result<()>;

    // 发送数据
    fn send(&self, data: &[u8]) -> Result<usize>;

    // 接收数据
    fn recv(&self, buf: &mut [u8]) -> Result<usize>;

    // 关闭套接字
    fn close(&mut self) -> Result<()>;

    // 获取实际绑定的端口
    fn get_port(&self) -> Result<u16>;

    // 克隆自身，返回Box<dyn UdpSocketImpl>
    fn clone_box(&self) -> Box<dyn UdpSocketImpl>;
}

// 为Box<dyn UdpSocketImpl>实现Clone
impl Clone for Box<dyn UdpSocketImpl> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// 为UdpSocket实现Clone
impl Clone for UdpSocket {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

// UDP套接字封装结构体
pub struct UdpSocket {
    // 内部实现（平台特定）
    inner: Box<dyn UdpSocketImpl>,
}

impl UdpSocket {
    /// 创建新的UDP套接字
    pub fn new(
        mode: UdpMode,
        multicast_addr: Option<std::net::IpAddr>,
        port: u16,
        buffer_size: usize,
    ) -> Result<Self> {
        // 使用标准库的UDP套接字实现，适用于所有平台
        let inner = Box::new(posix::PosixUdpSocket::new(
            mode,
            multicast_addr,
            port,
            buffer_size,
        )?);
        Ok(Self { inner })
    }

    /// 初始化套接字
    pub fn init(&mut self) -> Result<()> {
        self.inner.init()
    }

    /// 发送数据
    pub fn send(&self, data: &[u8]) -> Result<usize> {
        self.inner.send(data)
    }

    /// 接收数据
    pub fn recv(&self, buf: &mut [u8]) -> Result<usize> {
        self.inner.recv(buf)
    }

    /// 关闭套接字
    pub fn close(&mut self) -> Result<()> {
        self.inner.close()
    }

    /// 获取实际绑定的端口
    pub fn get_port(&self) -> Result<u16> {
        self.inner.get_port()
    }
}

// 标准库实现（适用于所有平台）
mod posix {
    use super::*;
    use socket2;
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
    use std::str::FromStr;

    // POSIX平台UDP套接字实现
    pub struct PosixUdpSocket {
        // 标准库UDP套接字
        socket: Option<StdUdpSocket>,
        // 目标地址（用于单播）
        dest_addr: Option<SocketAddr>,
        // 缓冲区大小
        buffer_size: usize,
    }

    impl PosixUdpSocket {
        /// 创建新的POSIX UDP套接字
        pub fn new(
            mode: UdpMode,
            multicast_addr: Option<std::net::IpAddr>,
            port: u16,
            buffer_size: usize,
        ) -> Result<Self> {
            // 创建UDP套接字
            let socket = match mode {
                UdpMode::Unicast => {
                    // 单播模式：创建套接字，设置选项，然后绑定
                    let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port))
                        .expect("failed to parse socket address");
                    // 使用 socket2 库创建套接字，设置选项，然后绑定
                    let socket2 = socket2::Socket::new(
                        socket2::Domain::IPV4,
                        socket2::Type::DGRAM,
                        Some(socket2::Protocol::UDP),
                    )
                    .map_err(|_| PubSubError::NetworkError)?;
                    // 设置SO_REUSEADDR选项，允许多个进程绑定到同一个端口
                    socket2
                        .set_reuse_address(true)
                        .map_err(|_| PubSubError::NetworkError)?;
                    // 绑定到地址
                    socket2
                        .bind(&addr.into())
                        .map_err(|_| PubSubError::NetworkError)?;
                    // 转换为标准库套接字
                    let socket = socket2.into();
                    socket
                }
                UdpMode::Broadcast => {
                    // 广播模式：创建套接字，设置选项，然后绑定
                    let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port))
                        .expect("failed to parse socket address");
                    // 使用 socket2 库创建套接字，设置选项，然后绑定
                    let socket2 = socket2::Socket::new(
                        socket2::Domain::IPV4,
                        socket2::Type::DGRAM,
                        Some(socket2::Protocol::UDP),
                    )
                    .map_err(|_| PubSubError::NetworkError)?;
                    // 设置SO_REUSEADDR选项，允许多个进程绑定到同一个端口
                    socket2
                        .set_reuse_address(true)
                        .map_err(|_| PubSubError::NetworkError)?;
                    // 设置广播选项
                    socket2
                        .set_broadcast(true)
                        .map_err(|_| PubSubError::NetworkError)?;
                    // 绑定到地址
                    socket2
                        .bind(&addr.into())
                        .map_err(|_| PubSubError::NetworkError)?;
                    // 转换为标准库套接字
                    let socket = socket2.into();
                    socket
                }
                UdpMode::Multicast => {
                    // 组播模式：创建套接字，设置选项，加入组播组
                    let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", port))
                        .expect("failed to parse socket address");
                    // 使用 socket2 库创建套接字，设置选项，然后绑定
                    let socket2 = socket2::Socket::new(
                        socket2::Domain::IPV4,
                        socket2::Type::DGRAM,
                        Some(socket2::Protocol::UDP),
                    )
                    .map_err(|_| PubSubError::NetworkError)?;
                    // 设置SO_REUSEADDR选项，允许多个进程绑定到同一个端口
                    socket2
                        .set_reuse_address(true)
                        .map_err(|_| PubSubError::NetworkError)?;
                    // 绑定到地址
                    socket2
                        .bind(&addr.into())
                        .map_err(|_| PubSubError::NetworkError)?;

                    if let Some(multicast_addr) = multicast_addr {
                        match multicast_addr {
                            std::net::IpAddr::V4(ipv4) => {
                                socket2
                                    .join_multicast_v4(&ipv4, &Ipv4Addr::new(0, 0, 0, 0))
                                    .map_err(|_| PubSubError::NetworkError)?;
                            }
                            std::net::IpAddr::V6(ipv6) => {
                                socket2
                                    .join_multicast_v6(&ipv6, 0)
                                    .map_err(|_| PubSubError::NetworkError)?;
                            }
                        }
                    }
                    // 转换为标准库套接字
                    let socket = socket2.into();
                    socket
                }
            };

            // 确定目标地址
            let dest_addr = match mode {
                UdpMode::Unicast => {
                    // 单播模式：需要明确的目标地址
                    // 注意：这里暂时设置为None，实际使用时需要通过其他方式设置
                    None
                }
                UdpMode::Broadcast => {
                    // 广播模式：使用广播地址
                    Some(
                        SocketAddr::from_str(&format!("255.255.255.255:{}", port))
                            .expect("failed to parse socket address"),
                    )
                }
                UdpMode::Multicast => {
                    // 组播模式：使用组播地址
                    if let Some(multicast_addr) = multicast_addr {
                        Some(SocketAddr::new(multicast_addr, port))
                    } else {
                        return Err(PubSubError::InvalidParameter);
                    }
                }
            };

            Ok(Self {
                socket: Some(socket),
                dest_addr,
                buffer_size,
            })
        }
    }

    impl UdpSocketImpl for PosixUdpSocket {
        fn init(&mut self) -> Result<()> {
            // POSIX套接字在创建时已经初始化
            Ok(())
        }

        fn send(&self, data: &[u8]) -> Result<usize> {
            match &self.socket {
                Some(socket) => {
                    match &self.dest_addr {
                        Some(addr) => {
                            // 单播/广播/组播模式：发送到指定地址
                            socket
                                .send_to(data, addr)
                                .map_err(|_| PubSubError::NetworkError)
                        }
                        None => {
                            // 单播模式但未设置目标地址：错误
                            Err(PubSubError::InvalidParameter)
                        }
                    }
                }
                None => Err(PubSubError::NetworkError),
            }
        }

        fn recv(&self, buf: &mut [u8]) -> Result<usize> {
            match &self.socket {
                Some(socket) => {
                    // 使用recv_from接收所有消息，包括广播消息，忽略源地址
                    socket
                        .recv_from(buf)
                        .map(|(len, _)| len)
                        .map_err(|_| PubSubError::NetworkError)
                }
                None => Err(PubSubError::NetworkError),
            }
        }

        fn close(&mut self) -> Result<()> {
            // POSIX套接字会在drop时自动关闭
            self.socket = None;
            Ok(())
        }

        fn get_port(&self) -> Result<u16> {
            match &self.socket {
                Some(socket) => socket
                    .local_addr()
                    .map(|addr| addr.port())
                    .map_err(|_| PubSubError::NetworkError),
                None => Err(PubSubError::NetworkError),
            }
        }

        fn clone_box(&self) -> Box<dyn UdpSocketImpl> {
            // 注意：这里创建了一个新的套接字，而不是克隆现有套接字
            // 因为UDP套接字是面向无连接的，每个实例需要自己的套接字
            // 在实际使用中，可能需要根据具体需求调整此实现
            Box::new(PosixUdpSocket {
                socket: None, // 新套接字需要重新初始化
                dest_addr: self.dest_addr,
                buffer_size: self.buffer_size,
            })
        }
    }
}

// Baremetal平台实现（仅提供接口）
#[cfg(feature = "baremetal")]
mod baremetal {
    use super::*;

    // Baremetal平台UDP套接字实现（需要用户实现）
    pub struct BaremetalUdpSocket {
        // 这里需要用户提供baremetal平台的实现
    }

    impl BaremetalUdpSocket {
        /// 创建新的Baremetal UDP套接字
        pub fn new(
            _mode: UdpMode,
            _multicast_addr: Option<std::net::IpAddr>,
            _port: u16,
            _buffer_size: usize,
        ) -> Result<Self> {
            // 这里需要用户提供实现
            Err(PubSubError::UnsupportedOperation)
        }
    }

    impl UdpSocketImpl for BaremetalUdpSocket {
        fn init(&mut self) -> Result<()> {
            Err(PubSubError::UnsupportedOperation)
        }

        fn send(&self, _data: &[u8]) -> Result<usize> {
            Err(PubSubError::UnsupportedOperation)
        }

        fn recv(&self, _buf: &mut [u8]) -> Result<usize> {
            Err(PubSubError::UnsupportedOperation)
        }

        fn close(&mut self) -> Result<()> {
            Err(PubSubError::UnsupportedOperation)
        }

        fn get_port(&self) -> Result<u16> {
            Err(PubSubError::UnsupportedOperation)
        }

        fn clone_box(&self) -> Box<dyn UdpSocketImpl> {
            Box::new(BaremetalUdpSocket {})
        }
    }
}
