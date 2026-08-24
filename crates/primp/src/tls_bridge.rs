//! Synchronous rustls `ClientConnection` bridge for tokio async IO.
//!
//! Wraps rustls into a tokio-compatible [`AsyncRead`] + [`AsyncWrite`] stream,
//! performing TLS handshakes and record I/O via non-blocking polling.

use hyper_util::client::legacy::connect::{Connected, Connection};
use rustls::client::ClientConnection;
use rustls::ClientConfig;
use rustls_pki_types::ServerName;
use std::io::{self, Read, Write};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// Trait alias for types usable as hyper IO with TLS.
pub(crate) trait TlsIoHyper:
    hyper::rt::Read + hyper::rt::Write + Unpin + Send + Sync + 'static
{
}
impl<T: hyper::rt::Read + hyper::rt::Write + Unpin + Send + Sync + 'static> TlsIoHyper for T {}

impl Connection for Box<dyn TlsIoHyper> {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

struct SyncReadAdapter<'a, 'b, T> {
    io: &'a mut T,
    cx: &'a mut Context<'b>,
}

struct SyncWriteAdapter<'a, 'b, T> {
    io: &'a mut T,
    cx: &'a mut Context<'b>,
}

impl<T: AsyncWrite + Unpin> Write for SyncWriteAdapter<'_, '_, T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match Pin::new(&mut self.io).poll_write(self.cx, buf) {
            Poll::Ready(result) => result,
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match Pin::new(&mut self.io).poll_flush(self.cx) {
            Poll::Ready(result) => result,
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
}

impl<T: AsyncRead + Unpin> Read for SyncReadAdapter<'_, '_, T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = ReadBuf::new(buf);
        match Pin::new(&mut self.io).poll_read(self.cx, &mut buf) {
            Poll::Ready(Ok(())) => Ok(buf.filled().len()),
            Poll::Ready(Err(err)) => Err(err),
            Poll::Pending => Err(io::ErrorKind::WouldBlock.into()),
        }
    }
}

struct Stream<'a, IO> {
    io: &'a mut IO,
    session: &'a mut ClientConnection,
    eof: bool,
}

impl<'a, IO: AsyncRead + AsyncWrite + Unpin> Stream<'a, IO> {
    fn new(io: &'a mut IO, session: &'a mut ClientConnection) -> Self {
        Stream {
            io,
            session,
            eof: false,
        }
    }

    fn read_io(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let mut reader = SyncReadAdapter { io: self.io, cx };

        let n = match self.session.read_tls(&mut reader) {
            Ok(n) => n,
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => return Poll::Pending,
            Err(err) => return Poll::Ready(Err(err)),
        };

        // Skip process_new_packets on EOF — it can spuriously error if
        // the peer didn't send a clean close_notify.
        if n > 0 {
            if let Err(err) = self.session.process_new_packets() {
                // Best-effort flush of any pending TLS records (e.g.,
                // close_notify alert) before returning the error.
                if let Poll::Ready(Err(write_err)) = self.write_io(cx) {
                    log::trace!("tls: failed to flush after packet error: {}", write_err);
                }
                return Poll::Ready(Err(io::Error::new(io::ErrorKind::InvalidData, err)));
            }
        }

        Poll::Ready(Ok(n))
    }

    fn write_io(&mut self, cx: &mut Context) -> Poll<io::Result<usize>> {
        let mut writer = SyncWriteAdapter { io: self.io, cx };

        match self.session.write_tls(&mut writer) {
            Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => Poll::Pending,
            result => Poll::Ready(result),
        }
    }

    fn handshake(&mut self, cx: &mut Context) -> Poll<io::Result<(usize, usize)>> {
        // Limit iterations to prevent an infinite loop if the TLS state machine
        // spins without producing I/O. A real TLS handshake completes in very
        // few round-trips; 128 is extremely generous.
        const MAX_ITERATIONS: usize = 128;
        let mut iterations = 0usize;

        let mut wrlen = 0;
        let mut rdlen = 0;

        loop {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                return Poll::Ready(Err(io::Error::other(
                    "tls handshake exceeded maximum iterations",
                )));
            }

            let mut write_would_block = false;
            let mut read_would_block = false;

            while self.session.wants_write() {
                match self.write_io(cx) {
                    Poll::Ready(Ok(0)) => return Poll::Ready(Err(io::ErrorKind::WriteZero.into())),
                    Poll::Ready(Ok(n)) => wrlen += n,
                    Poll::Pending => {
                        write_would_block = true;
                        break;
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            // Flush any written TLS records to the underlying transport.
            if wrlen > 0 {
                match Pin::new(&mut self.io).poll_flush(cx) {
                    Poll::Ready(Ok(())) => {}
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => write_would_block = true,
                }
            }

            // If writes blocked with no progress, return Pending. Attempting
            // reads without a write waker would risk the task hanging.
            if write_would_block && wrlen == 0 {
                return Poll::Pending;
            }

            // Always try to read TLS data from the underlying transport,
            // even when wants_read() is false, to detect new data from the
            // peer and EOF on closed connections. Without this, a handshake
            // stalls after sending ClientHello because the session expects
            // us to read data before it sets wants_read().
            if !self.eof && !self.session.wants_read() {
                match self.read_io(cx) {
                    Poll::Ready(Ok(0)) => self.eof = true,
                    Poll::Ready(Ok(n)) => rdlen += n,
                    Poll::Pending => {
                        read_would_block = true;
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            while !self.eof && self.session.wants_read() {
                match self.read_io(cx) {
                    Poll::Ready(Ok(0)) => self.eof = true,
                    Poll::Ready(Ok(n)) => rdlen += n,
                    Poll::Pending => {
                        read_would_block = true;
                        break;
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            return match (self.eof, self.session.is_handshaking()) {
                (true, true) => {
                    let err = io::Error::new(io::ErrorKind::UnexpectedEof, "tls handshake eof");
                    Poll::Ready(Err(err))
                }
                (_, false) => Poll::Ready(Ok((rdlen, wrlen))),
                (_, true) if write_would_block || read_would_block => {
                    // Both flags may be set when progress was made (bytes
                    // read/written) but the handshake needs more I/O. Return
                    // the progress so the outer loop can continue. If NO
                    // progress was made, return Pending to avoid spinning.
                    if rdlen != 0 || wrlen != 0 {
                        Poll::Ready(Ok((rdlen, wrlen)))
                    } else {
                        Poll::Pending
                    }
                }
                // Handshaking and neither read nor write blocked: the
                // state machine made progress but isn't done. Continue
                // the inner loop to drive it further.
                (..) => continue,
            };
        }
    }
}

impl<'a, IO: AsyncRead + AsyncWrite + Unpin> AsyncRead for Stream<'a, IO> {
    /// Reads decrypted bytes into `buf`, refilling the session from the underlying
    /// IO. May return fewer bytes than `buf.capacity()` since rustls' `Reader`
    /// only exposes its current buffer; loop until `Ok(0)` for a full frame.
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Bound the drain: a (fast/malicious) peer flooding no-plaintext
        // ciphertext (NewSessionTicket, KeyUpdate, empty app-data) would
        // otherwise spin this poll forever, starving the worker and defeating
        // read timeouts. On budget exhaustion: self-wake + return Pending so
        // the executor interleaves; the handshake loop above uses the same cap.
        const MAX_READ_IO_CALLS: usize = 128;
        let mut read_io_calls = 0usize;
        let mut io_pending = false;

        // Always try to read TLS data from the underlying transport,
        // even when wants_read() is false, to detect EOF on closed
        // connections (read_io returns Ok(0)).
        if !self.eof {
            if self.session.wants_read() {
                loop {
                    if read_io_calls >= MAX_READ_IO_CALLS {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    read_io_calls += 1;
                    match self.read_io(cx) {
                        Poll::Ready(Ok(0)) => {
                            self.eof = true;
                            break;
                        }
                        Poll::Ready(Ok(_)) => {
                            if !self.session.wants_read() {
                                break;
                            }
                        }
                        Poll::Pending => {
                            io_pending = true;
                            break;
                        }
                        Poll::Ready(Err(err))
                            if err.kind() == io::ErrorKind::Other
                                && err.to_string() == "received plaintext buffer full" =>
                        {
                            // Plaintext buffer full (16 KiB): stop and drain.
                            break;
                        }
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    }
                }
            } else {
                match self.read_io(cx) {
                    Poll::Ready(Ok(0)) => {
                        self.eof = true;
                    }
                    Poll::Ready(Ok(_)) => {}
                    Poll::Pending => {
                        io_pending = true;
                    }
                    Poll::Ready(Err(err))
                        if err.kind() == io::ErrorKind::Other
                            && err.to_string() == "received plaintext buffer full" =>
                    {
                        // Buffer full: drain first.
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }
        }

        match self.session.reader().read(buf.initialize_unfilled()) {
            Ok(n) => {
                buf.advance(n);
                Poll::Ready(Ok(()))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if self.eof {
                    Poll::Ready(Ok(()))
                } else if !io_pending {
                    // No application data is available and the underlying
                    // transport has not registered a waker (every `read_io`
                    // above returned `Ready`). Loop doing `read_io` so the
                    // transport's waker gets registered (or any trailing
                    // ciphertext / EOF is consumed). This guarantees we are
                    // woken when the peer sends data.
                    //
                    // A loop is required: `read_io` reads at most one chunk
                    // (~4KB) per call, so leftover ciphertext can remain in the
                    // socket after the `read_io` above. A follow-up `read_io`
                    // may therefore surface *new* ciphertext (or an I/O error)
                    // even though no application data was readable a moment
                    // ago, in which case we must retry reading application data
                    // (or surface the error) instead of returning `Pending`
                    // with no waker registered, which would stall the task.
                    //
                    // A sustained cadence of such no-plaintext records would
                    // spin this loop forever; MAX_READ_IO_CALLS bounds it (same
                    // budget as the drain above).
                    loop {
                        if read_io_calls >= MAX_READ_IO_CALLS {
                            cx.waker().wake_by_ref();
                            return Poll::Pending;
                        }
                        read_io_calls += 1;
                        match self.read_io(cx) {
                            Poll::Ready(Ok(0)) => {
                                self.eof = true;
                                return Poll::Ready(Ok(()));
                            }
                            Poll::Ready(Ok(_)) => {
                                match self.session.reader().read(buf.initialize_unfilled()) {
                                    Ok(n) => {
                                        buf.advance(n);
                                        return Poll::Ready(Ok(()));
                                    }
                                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                        continue;
                                    }
                                    Err(e) => return Poll::Ready(Err(e)),
                                }
                            }
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(Err(err))
                                if err.kind() == io::ErrorKind::Other
                                    && err.to_string() == "received plaintext buffer full" =>
                            {
                                // Plaintext buffer full - drain it first.
                                match self.session.reader().read(buf.initialize_unfilled()) {
                                    Ok(n) => {
                                        buf.advance(n);
                                        return Poll::Ready(Ok(()));
                                    }
                                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                        cx.waker().wake_by_ref();
                                        return Poll::Pending;
                                    }
                                    Err(e) => return Poll::Ready(Err(e)),
                                }
                            }
                            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        }
                    }
                } else {
                    Poll::Pending
                }
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

impl<'a, IO: AsyncRead + AsyncWrite + Unpin> AsyncWrite for Stream<'a, IO> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut pos = 0;

        while pos != buf.len() {
            let mut would_block = false;

            match self.session.writer().write(&buf[pos..]) {
                Ok(0) => {
                    // Defensive: in rustls this only happens when the
                    // writer's record buffer is saturated, which should
                    // imply `wants_write()` is true. If it is *not*, we
                    // have no buffered TLS to flush, so we cannot make
                    // progress on this poll. Poll the transport to install a
                    // waker (otherwise a `Poll::Pending` returned below would
                    // leave the task without a waker and could hang), then
                    // fall through to return Pending below.
                    if !self.session.wants_write() {
                        match self.write_io(cx) {
                            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                            _ => {
                                would_block = true;
                            }
                        }
                    }
                }
                Ok(n) => pos += n,
                Err(err) => return Poll::Ready(Err(err)),
            };

            while self.session.wants_write() {
                match self.write_io(cx) {
                    Poll::Ready(Ok(0)) => {
                        return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
                    }
                    Poll::Ready(Ok(_)) => (),
                    Poll::Pending => {
                        would_block = true;
                        break;
                    }
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                }
            }

            return match (pos, would_block) {
                (0, true) => Poll::Pending,
                (n, true) => Poll::Ready(Ok(n)),
                (_, false) => continue,
            };
        }

        Poll::Ready(Ok(pos))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.session.writer().flush()?;
        while self.session.wants_write() {
            if ready!(self.write_io(cx))? == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // RFC 8446 §6.1: send close_notify so the peer sees a clean EOF
        // instead of a truncated record. No-op if already sent; skipped
        // while handshaking so aborted mid-connect teardowns stay unchanged.
        if !self.session.is_handshaking() {
            self.session.send_close_notify();
        }
        // Drain any pending TLS data (e.g., the close_notify alert) before
        // shutting down the underlying transport.
        while self.session.wants_write() {
            if ready!(self.write_io(cx))? == 0 {
                return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
            }
        }
        // Flush any buffered TLS records to the transport.
        match Pin::new(&mut self.io).poll_flush(cx) {
            Poll::Ready(Ok(())) => {}
            Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
            Poll::Pending => return Poll::Pending,
        }
        Poll::Ready(match ready!(Pin::new(&mut self.io).poll_shutdown(cx)) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotConnected => Ok(()),
            Err(err) => Err(err),
        })
    }
}

/// Tokio-compatible TLS stream over rustls' synchronous `ClientConnection`,
/// driving TLS record I/O by polling the async transport via [`Stream`].
pub(crate) struct TokioTlsStream<IO> {
    io: IO,
    session: ClientConnection,
}

impl<IO> TokioTlsStream<IO> {
    pub(crate) fn alpn_protocol(&self) -> Option<&[u8]> {
        self.session.alpn_protocol()
    }

    pub(crate) fn peer_certificates(&self) -> Option<&[rustls_pki_types::CertificateDer<'_>]> {
        self.session.peer_certificates()
    }

    pub(crate) fn protocol_version(&self) -> Option<rustls::ProtocolVersion> {
        self.session.protocol_version()
    }

    pub(crate) fn get_ref(&self) -> &IO {
        &self.io
    }

    pub(crate) fn get_io_mut(&mut self) -> &mut IO {
        &mut self.io
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> Connection for TokioTlsStream<IO> {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Send + 'static> TokioTlsStream<IO> {
    pub(crate) async fn connect(
        config: Arc<ClientConfig>,
        domain: ServerName<'static>,
        mut io: IO,
    ) -> io::Result<Self> {
        let mut session = ClientConnection::new(config, domain).map_err(io::Error::other)?;

        // Limit outer iterations to prevent infinite loop if the inner
        // handshake makes no progress (e.g., returns Ok with 0 bytes
        // repeatedly while still handshaking).
        const MAX_OUTER_ITERATIONS: usize = 16;
        for _ in 0..MAX_OUTER_ITERATIONS {
            let mut made_progress = false;
            let result: io::Result<()> = std::future::poll_fn(|cx| {
                let mut stream = Stream::new(&mut io, &mut session);
                match stream.handshake(cx) {
                    Poll::Ready(Ok((rdlen, wrlen))) => {
                        // Track whether the inner handshake made progress.
                        // If still handshaking with no progress, the outer
                        // loop should stop to avoid spinning.
                        made_progress = rdlen != 0 || wrlen != 0;
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
                match Pin::new(&mut stream).poll_flush(cx) {
                    Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
                    Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                    Poll::Pending => Poll::Pending,
                }
            })
            .await;

            result?;

            if !session.is_handshaking() {
                break;
            }

            // If the inner handshake returned Ok but made no progress
            // (both rdlen and wrlen are 0), we'd spin the outer loop
            // fruitfully. Return an error instead.
            if !made_progress {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "tls handshake made no progress",
                ));
            }
        }

        if session.is_handshaking() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "tls handshake did not complete",
            ));
        }

        Ok(TokioTlsStream { io, session })
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> AsyncRead for TokioTlsStream<IO> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let mut stream = Stream::new(&mut this.io, &mut this.session);
        Pin::new(&mut stream).poll_read(cx, buf)
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin> AsyncWrite for TokioTlsStream<IO> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let mut stream = Stream::new(&mut this.io, &mut this.session);
        Pin::new(&mut stream).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let mut stream = Stream::new(&mut this.io, &mut this.session);
        Pin::new(&mut stream).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let mut stream = Stream::new(&mut this.io, &mut this.session);
        Pin::new(&mut stream).poll_shutdown(cx)
    }
}
