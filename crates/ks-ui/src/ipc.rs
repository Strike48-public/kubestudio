//! Cross-platform IPC transport for the LiveView server.
//!
//! - Unix: Unix domain sockets
//! - Windows: Named pipes

/// IPC endpoint address.
#[derive(Clone, Debug)]
pub struct IpcAddr {
    #[cfg(unix)]
    pub(crate) inner: std::path::PathBuf,
    #[cfg(windows)]
    pub(crate) inner: String,
}

impl std::fmt::Display for IpcAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[cfg(unix)]
        return write!(f, "unix://{}", self.inner.display());
        #[cfg(windows)]
        return write!(f, "pipe://{}", self.inner);
    }
}

impl IpcAddr {
    /// Generate a PID-based address for standalone connector mode.
    pub fn for_connector(pid: u32) -> Self {
        #[cfg(unix)]
        return Self {
            inner: std::path::PathBuf::from(format!("/tmp/ks-connector-{}.sock", pid)),
        };
        #[cfg(windows)]
        return Self {
            inner: format!(r"\\.\pipe\ks-connector-{}", pid),
        };
    }

    /// Create from a Unix socket path (StrikeHub IPC mode).
    #[cfg(unix)]
    pub fn from_path(path: std::path::PathBuf) -> Self {
        Self { inner: path }
    }

    /// Remove the socket file (Unix) or no-op (Windows named pipes auto-cleanup).
    pub fn cleanup(&self) {
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.inner);
        }
    }
}

// ---------------------------------------------------------------------------
// IPC listener
// ---------------------------------------------------------------------------

pub struct IpcListener {
    #[cfg(unix)]
    inner: tokio::net::UnixListener,
    #[cfg(windows)]
    pipe_name: String,
    #[cfg(windows)]
    current: tokio::net::windows::named_pipe::NamedPipeServer,
}

impl IpcListener {
    pub fn bind(addr: &IpcAddr) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            if addr.inner.exists() {
                let _ = std::fs::remove_file(&addr.inner);
            }
            let inner = tokio::net::UnixListener::bind(&addr.inner)?;
            Ok(Self { inner })
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ServerOptions;
            let current = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&addr.inner)?;
            Ok(Self {
                pipe_name: addr.inner.clone(),
                current,
            })
        }
    }

    /// Accept a new IPC connection, returning a stream usable with `hyper_util::rt::TokioIo`.
    pub async fn accept(&mut self) -> std::io::Result<IpcStream> {
        #[cfg(unix)]
        {
            let (stream, _) = self.inner.accept().await?;
            Ok(IpcStream { inner: stream })
        }
        #[cfg(windows)]
        {
            self.current.connect().await?;
            use tokio::net::windows::named_pipe::ServerOptions;
            let next = ServerOptions::new().create(&self.pipe_name)?;
            let connected = std::mem::replace(&mut self.current, next);
            Ok(IpcStream { inner: connected })
        }
    }
}

// ---------------------------------------------------------------------------
// Client-side IPC stream
// ---------------------------------------------------------------------------

pub struct IpcStream {
    #[cfg(unix)]
    inner: tokio::net::UnixStream,
    #[cfg(windows)]
    inner: tokio::net::windows::named_pipe::NamedPipeServer,
}

/// Client-side connection to an IPC endpoint.
pub struct IpcClientStream {
    #[cfg(unix)]
    inner: tokio::net::UnixStream,
    #[cfg(windows)]
    inner: tokio::net::windows::named_pipe::NamedPipeClient,
}

impl IpcClientStream {
    pub async fn connect(addr: &IpcAddr) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            let inner = tokio::net::UnixStream::connect(&addr.inner).await?;
            Ok(Self { inner })
        }
        #[cfg(windows)]
        {
            use tokio::net::windows::named_pipe::ClientOptions;
            let inner = ClientOptions::new().open(&addr.inner)?;
            Ok(Self { inner })
        }
    }
}

// AsyncRead/AsyncWrite for IpcStream (server-accepted connections)
impl tokio::io::AsyncRead for IpcStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for IpcStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

// AsyncRead/AsyncWrite for IpcClientStream (client connections)
impl tokio::io::AsyncRead for IpcClientStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for IpcClientStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}
