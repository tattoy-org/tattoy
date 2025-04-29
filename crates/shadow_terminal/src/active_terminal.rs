//! A convenience module wrapping [`ShadowTerminal`] for running an active shadow terminal
//! running in a Tokio task.
//!
//! The underlying [`Wezterm`] terminal cannnot be interacted with directly. Instead input
//! and output must be sent and read over channels. This module is more likely useful for
//! real-world usecases, such as terminal multiplexing for example.

use tracing::Instrument as _;

/// An active terminal is running in a Tokio task, so we don't have direct access to the
/// underlying `wezterm_term::Terminal`. Instead we interact with it and the PTY through Tokio
/// channels.
#[non_exhaustive]
pub struct ActiveTerminal {
    /// The task handle to the actively running [`crate::shadow_tty::ShadowTerminal`]
    pub task_handle: tokio::task::JoinHandle<()>,
    /// A Tokio channel that receives [`termwiz::surface::Surface`] updates of the underlying
    /// terminal.
    pub surface_output_rx: tokio::sync::mpsc::Receiver<crate::output::Output>,
    /// A Tokio channel that forwards bytes to the underlying PTY's STDIN.
    pub pty_input_tx: tokio::sync::mpsc::Sender<crate::pty::BytesFromSTDIN>,
    /// A Tokio broadcast sender to send protocol messages that control the shadow terminal and
    /// PTY. For example; resizing and shutting down.
    pub control_tx: tokio::sync::broadcast::Sender<crate::Protocol>,
}

impl ActiveTerminal {
    /// Start a [`crate::shadow_tty::ShadowTerminal`] running in a Tokio task.
    #[inline]
    #[must_use]
    pub fn start(config: crate::shadow_terminal::Config) -> Self {
        tracing::debug!("Starting shadow terminal...");
        let (pty_input_tx, pty_input_rx) = tokio::sync::mpsc::channel(1);
        let (surface_output_tx, surface_output_rx) = tokio::sync::mpsc::channel(1);

        let mut shadow_terminal =
            crate::shadow_terminal::ShadowTerminal::new(config, surface_output_tx);
        let control_tx = shadow_terminal.channels.control_tx.clone();

        let current_span = tracing::Span::current();
        let task_handle = tokio::spawn(async move {
            shadow_terminal
                .run(pty_input_rx)
                .instrument(current_span)
                .await;
        });
        tracing::debug!("Shadow terminal started.");

        Self {
            task_handle,
            surface_output_rx,
            pty_input_tx,
            control_tx,
        }
    }

    /// Send input directly into the underlying PTY process. This doesn't go through the shadow
    /// terminal's "frontend".
    ///
    /// # Errors
    /// If sending the bytes fails
    #[inline]
    pub async fn send_input(
        &self,
        bytes: crate::pty::BytesFromSTDIN,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<crate::pty::BytesFromSTDIN>> {
        self.pty_input_tx.send(bytes).await
    }

    /// End all loops and send OS kill signals to the underlying PTY.
    ///
    /// # Errors
    /// If sending message over channel fails.
    #[inline]
    pub fn kill(&self) -> Result<usize, tokio::sync::broadcast::error::SendError<crate::Protocol>> {
        tracing::debug!("`kill()` called on `ActiveTerminal`");
        self.control_tx.send(crate::Protocol::End)
    }

    /// Resize the shadow terminal "frontend". The PTY is agnostic about size.
    ///
    /// # Errors
    /// If sending message over channel fails.
    #[inline]
    pub fn resize(
        &self,
        width: u16,
        height: u16,
    ) -> Result<usize, tokio::sync::broadcast::error::SendError<crate::Protocol>> {
        self.control_tx
            .send(crate::Protocol::Resize { width, height })
    }

    /// Scroll the shadow Wezterm terminal up.
    ///
    /// # Errors
    /// If sending message over channel fails.
    #[inline]
    pub fn scroll_up(
        &self,
    ) -> Result<usize, tokio::sync::broadcast::error::SendError<crate::Protocol>> {
        self.control_tx
            .send(crate::Protocol::Scroll(crate::Scroll::Up))
    }

    /// Scroll the shadow Wezterm terminal down.
    ///
    /// # Errors
    /// If sending message over channel fails.
    #[inline]
    pub fn scroll_down(
        &self,
    ) -> Result<usize, tokio::sync::broadcast::error::SendError<crate::Protocol>> {
        self.control_tx
            .send(crate::Protocol::Scroll(crate::Scroll::Down))
    }

    /// Cancel scrolling, and return the scroll to normal.
    ///
    /// # Errors
    /// If sending message over channel fails.
    #[inline]
    pub fn scroll_cancel(
        &self,
    ) -> Result<usize, tokio::sync::broadcast::error::SendError<crate::Protocol>> {
        self.control_tx
            .send(crate::Protocol::Scroll(crate::Scroll::Cancel))
    }
}

impl Drop for ActiveTerminal {
    #[inline]
    fn drop(&mut self) {
        let result = self.kill();
        if let Err(error) = result {
            tracing::error!("{error:?}");
        }
    }
}
