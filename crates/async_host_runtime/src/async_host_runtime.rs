//! Asynchronous host loading for heavy native runtimes (CEF, Deno, …).
//!
//! Heavy `dlopen` / isolate startup must never run on the GPUI foreground
//! thread. Callers spawn a host session, observe [`HostLifecycle`] on the UI
//! side, and exchange commands / events over channels. Dropping the session
//! closes the command channel so the background loop can exit.

use std::fmt;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use anyhow::{Result, anyhow};
use async_channel::{Receiver, Sender, bounded, unbounded};
use futures::channel::oneshot;
use parking_lot::RwLock;

/// Lifecycle of an asynchronously loaded host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HostLifecycle {
    #[default]
    Loading,
    Ready,
    Failed {
        message: String,
    },
}

impl HostLifecycle {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    pub fn failure_message(&self) -> Option<&str> {
        match self {
            Self::Failed { message } => Some(message.as_str()),
            _ => None,
        }
    }
}

/// Shared, lockable lifecycle cell observed by the UI thread.
#[derive(Clone, Default)]
pub struct HostLifecycleCell {
    inner: Arc<RwLock<HostLifecycle>>,
}

impl HostLifecycleCell {
    pub fn new(state: HostLifecycle) -> Self {
        Self {
            inner: Arc::new(RwLock::new(state)),
        }
    }

    pub fn get(&self) -> HostLifecycle {
        self.inner.read().clone()
    }

    pub fn set(&self, state: HostLifecycle) {
        *self.inner.write() = state;
    }

    pub fn mark_ready(&self) {
        self.set(HostLifecycle::Ready);
    }

    pub fn mark_failed(&self, message: impl Into<String>) {
        self.set(HostLifecycle::Failed {
            message: message.into(),
        });
    }

    pub fn failure_message(&self) -> Option<String> {
        self.get().failure_message().map(str::to_owned)
    }
}

/// A command delivered to the background host loop.
pub enum HostCommand<C> {
    User(C),
    /// Request/response call; the oneshot is completed by the host loop.
    Call {
        request: C,
        reply: oneshot::Sender<Result<()>>,
    },
    Shutdown,
}

impl<C> fmt::Debug for HostCommand<C>
where
    C: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(command) => f.debug_tuple("User").field(command).finish(),
            Self::Call { request, .. } => f.debug_struct("Call").field("request", request).finish(),
            Self::Shutdown => write!(f, "Shutdown"),
        }
    }
}

/// Handle held on the GPUI / caller side.
pub struct HostSession<C, E> {
    command_tx: Sender<HostCommand<C>>,
    event_rx: Receiver<E>,
    lifecycle: HostLifecycleCell,
    join: Option<JoinHandle<()>>,
}

impl<C, E> HostSession<C, E>
where
    C: Send + 'static,
    E: Send + 'static,
{
    /// Spawn a dedicated OS thread that runs `load` then `run`.
    ///
    /// `load` performs heavy initialization (dynamic linking, isolate creation).
    /// On success the lifecycle becomes [`HostLifecycle::Ready`] and `run` owns
    /// the command loop until shutdown or channel close.
    pub fn spawn_thread<L, R, T>(name: impl Into<String>, load: L, run: R) -> Self
    where
        L: FnOnce() -> Result<T> + Send + 'static,
        R: FnOnce(T, Receiver<HostCommand<C>>, Sender<E>, HostLifecycleCell) + Send + 'static,
        T: Send + 'static,
    {
        let (command_tx, command_rx) = unbounded::<HostCommand<C>>();
        let (event_tx, event_rx) = unbounded::<E>();
        let lifecycle = HostLifecycleCell::new(HostLifecycle::Loading);
        let lifecycle_for_thread = lifecycle.clone();
        let thread_name = name.into();

        let join = thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                match load() {
                    Ok(host) => {
                        lifecycle_for_thread.mark_ready();
                        run(host, command_rx, event_tx, lifecycle_for_thread);
                    }
                    Err(error) => {
                        lifecycle_for_thread.mark_failed(format!("{error:#}"));
                        // Drain until shutdown so drop does not race a finished thread
                        // that never saw the channel close semantics the same way.
                        while let Ok(command) = command_rx.recv_blocking() {
                            if matches!(command, HostCommand::Shutdown) {
                                break;
                            }
                            if let HostCommand::Call { reply, .. } = command {
                                let _ = reply.send(Err(anyhow!("host failed to start")));
                            }
                        }
                    }
                }
            })
            .unwrap_or_else(|error| {
                panic!("failed to spawn async host thread `{thread_name}`: {error}")
            });

        Self {
            command_tx,
            event_rx,
            lifecycle,
            join: Some(join),
        }
    }

    pub fn lifecycle(&self) -> &HostLifecycleCell {
        &self.lifecycle
    }

    pub fn event_receiver(&self) -> &Receiver<E> {
        &self.event_rx
    }

    pub fn clone_event_receiver(&self) -> Receiver<E> {
        self.event_rx.clone()
    }

    pub fn try_recv_event(&self) -> Result<E, async_channel::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub async fn send(&self, command: C) -> Result<()> {
        self.command_tx
            .send(HostCommand::User(command))
            .await
            .map_err(|_| anyhow!("async host command channel closed"))
    }

    pub fn send_blocking(&self, command: C) -> Result<()> {
        self.command_tx
            .send_blocking(HostCommand::User(command))
            .map_err(|_| anyhow!("async host command channel closed"))
    }

    /// Send a command and wait for an acknowledgement from the host loop.
    ///
    /// The host's `run` closure must complete the oneshot when handling
    /// [`HostCommand::Call`].
    pub async fn call(&self, request: C) -> Result<()> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.command_tx
            .send(HostCommand::Call {
                request,
                reply: reply_tx,
            })
            .await
            .map_err(|_| anyhow!("async host command channel closed"))?;
        reply_rx
            .await
            .map_err(|_| anyhow!("async host dropped call reply"))?
    }

    pub fn shutdown(&self) {
        let _ = self.command_tx.send_blocking(HostCommand::Shutdown);
    }

    /// Request shutdown and block until the host thread exits.
    ///
    /// Prefer this from background tasks / tests. Do not call from the GPUI
    /// foreground thread — use [`Drop`](Drop) which never blocks indefinitely.
    pub fn shutdown_and_join(mut self) {
        let _ = self.command_tx.send_blocking(HostCommand::Shutdown);
        self.command_tx.close();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl<C, E> Drop for HostSession<C, E> {
    fn drop(&mut self) {
        let _ = self.command_tx.send_blocking(HostCommand::Shutdown);
        self.command_tx.close();
        // Never block the GPUI foreground thread on a slow/hung host. Wait a
        // short bounded interval for a cooperative exit, then detach.
        if let Some(join) = self.join.take() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
            while !join.is_finished() {
                if std::time::Instant::now() >= deadline {
                    return;
                }
                thread::sleep(std::time::Duration::from_millis(1));
            }
            let _ = join.join();
        }
    }
}

/// Helper to process one command in a host loop.
///
/// Returns `false` when the loop should exit.
pub fn handle_command_result<C>(
    command: Result<HostCommand<C>, async_channel::RecvError>,
    mut on_user: impl FnMut(C) -> Result<()>,
) -> bool {
    match command {
        Ok(HostCommand::Shutdown) | Err(_) => false,
        Ok(HostCommand::User(command)) => {
            if let Err(error) = on_user(command) {
                log_host_error(&error);
            }
            true
        }
        Ok(HostCommand::Call { request, reply }) => {
            let result = on_user(request);
            let _ = reply.send(result);
            true
        }
    }
}

fn log_host_error(error: &anyhow::Error) {
    // Keep this crate free of the `log` facade so consumers choose logging.
    // Errors surface via lifecycle / call results; stderr is a last resort for
    // unexpected host-loop failures during early bring-up.
    eprintln!("async_host_runtime: {error:#}");
}

/// Bounded event buffer helper for high-frequency producers (e.g. OSR frames).
///
/// When full, the oldest event is dropped so the UI always sees a recent frame.
pub struct LatestEventBuffer<E> {
    tx: Sender<E>,
    rx: Receiver<E>,
    capacity: usize,
}

impl<E> LatestEventBuffer<E>
where
    E: Send + 'static,
{
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let (tx, rx) = bounded(capacity);
        Self { tx, rx, capacity }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn receiver(&self) -> &Receiver<E> {
        &self.rx
    }

    pub fn sender(&self) -> &Sender<E> {
        &self.tx
    }

    /// Push an event, dropping the oldest if the buffer is full.
    pub fn push_latest(&self, event: E) {
        while self.tx.is_full() {
            let _ = self.rx.try_recv();
        }
        let _ = self.tx.try_send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn load_failure_marks_failed_without_blocking_caller() {
        let session: HostSession<(), ()> = HostSession::spawn_thread(
            "test-fail-load",
            || -> Result<()> { Err(anyhow!("missing native library")) },
            |_host: (), _rx, _tx, _life| unreachable!("run must not be called"),
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !session.lifecycle().get().is_failed() {
            assert!(
                std::time::Instant::now() < deadline,
                "lifecycle never became Failed"
            );
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            session.lifecycle().failure_message().as_deref(),
            Some("missing native library")
        );
    }

    #[test]
    fn successful_load_marks_ready_and_handles_commands() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_run = counter.clone();

        let session: HostSession<usize, ()> = HostSession::spawn_thread(
            "test-ready",
            || Ok(42_u32),
            move |_host, command_rx, _event_tx, _lifecycle| {
                while handle_command_result(command_rx.recv_blocking(), |n: usize| {
                    counter_for_run.fetch_add(n, Ordering::SeqCst);
                    Ok(())
                }) {}
            },
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !session.lifecycle().get().is_ready() {
            assert!(
                std::time::Instant::now() < deadline,
                "lifecycle never became Ready"
            );
            thread::sleep(Duration::from_millis(5));
        }

        smol::block_on(async {
            session.send(3).await.expect("send");
            session.call(4).await.expect("call");
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while counter.load(Ordering::SeqCst) != 7 {
            assert!(
                std::time::Instant::now() < deadline,
                "commands were not processed"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn drop_cancels_in_flight_host_loop() {
        let started = Arc::new(AtomicUsize::new(0));
        let started_for_run = started.clone();
        let session = HostSession::<(), ()>::spawn_thread(
            "test-drop",
            || Ok(()),
            move |_host, command_rx, _event_tx, _lifecycle| {
                started_for_run.fetch_add(1, Ordering::SeqCst);
                while handle_command_result(command_rx.recv_blocking(), |_| Ok(())) {}
            },
        );

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while !session.lifecycle().get().is_ready() {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }

        session.shutdown_and_join();

        assert_eq!(started.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn latest_event_buffer_drops_oldest() {
        let buffer = LatestEventBuffer::new(2);
        buffer.push_latest(1);
        buffer.push_latest(2);
        buffer.push_latest(3);

        assert_eq!(buffer.receiver().try_recv().ok(), Some(2));
        assert_eq!(buffer.receiver().try_recv().ok(), Some(3));
        assert!(buffer.receiver().try_recv().is_err());
    }
}
