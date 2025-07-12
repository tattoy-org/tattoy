//! A trait for tattoys that use shaders.

use color_eyre::eyre::{ContextCompat as _, Result};
use futures_util::FutureExt as _;

/// Common logic for tattoys that render shaders.
pub(crate) trait Shaderer: Sized {
    /// Return a immutable reference to the Tattoyer helper.
    fn tattoy(&self) -> &crate::tattoys::tattoyer::Tattoyer;

    /// Return a mutable reference to the Tattoyer helper.
    fn tattoy_mut(&mut self) -> &mut crate::tattoys::tattoyer::Tattoyer;

    /// Returns a mutable reference to the GPU pipeline manager.
    fn gpu(&mut self) -> &mut super::pipeline::GPU;

    /// Is the config for this tattoy set to upload the TTY as pixels?
    async fn is_upload_tty_as_pixels(&self) -> bool;

    /// Get the current configured layer for the tattoy.
    async fn get_layer(&self) -> i16;

    /// Get the current configured opacity for the tattoy.
    async fn get_opacity(&self) -> f32;

    /// Get the current configured cursor scale for the tattoy.
    #[expect(
        clippy::allow_attributes,
        reason = "The lint behaves differently on CI"
    )]
    #[allow(clippy::unused_async, reason = "It's a default implementation")]
    async fn get_cursor_scale(&self) -> f32 {
        1.0
    }

    /// Instantiate
    async fn new(
        output_channel: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
        state: std::sync::Arc<crate::shared_state::SharedState>,
    ) -> Result<Self>;

    /// Our main entrypoint.
    async fn start(
        output: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
        state: std::sync::Arc<crate::shared_state::SharedState>,
    ) -> Result<()> {
        let may_panic = std::panic::AssertUnwindSafe(async {
            let result = Self::main(output, &state).await;

            if let Err(error) = result {
                tracing::error!("GPU pipeline error: {error:?}");
                state
                    .send_notification(
                        "GPU pipeline error",
                        crate::tattoys::notifications::message::Level::Error,
                        Some(error.root_cause().to_string()),
                        true,
                    )
                    .await;
                Err(error)
            } else {
                Ok(())
            }
        });

        if let Err(error) = may_panic.catch_unwind().await {
            let message = if let Some(message) = error.downcast_ref::<String>() {
                message
            } else if let Some(message) = error.downcast_ref::<&str>() {
                message
            } else {
                "Caught a panic with an unknown type."
            };
            tracing::error!("Shader panic: {message:?}");
            state
                .send_notification(
                    "GPU pipeline panic",
                    crate::tattoys::notifications::message::Level::Error,
                    Some(message.into()),
                    true,
                )
                .await;
        }

        Ok(())
    }

    /// Enter the main render loop. We put it in its own function so that we can easily handle any
    /// errors.
    async fn main(
        output: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
        state: &std::sync::Arc<crate::shared_state::SharedState>,
    ) -> Result<()> {
        let mut protocol = state.protocol_tx.subscribe();
        let mut shader = Self::new(output, std::sync::Arc::clone(state)).await?;

        #[expect(
            clippy::integer_division_remainder_used,
            reason = "This is caused by the `tokio::select!`"
        )]
        loop {
            tokio::select! {
                () = shader.tattoy_mut().sleep_until_next_frame_tick() => {
                    shader.render().await?;
                },
                result = protocol.recv() => {
                    if matches!(result, Ok(crate::run::Protocol::End)) {
                        break;
                    }
                    shader.handle_protocol_message(result).await?;
                }
            }
        }

        Ok(())
    }

    /// Handle messages from the main Tattoy app.
    async fn handle_protocol_message(
        &mut self,
        protocol_result: std::result::Result<
            crate::run::Protocol,
            tokio::sync::broadcast::error::RecvError,
        >,
    ) -> Result<()> {
        match protocol_result {
            Ok(message) => {
                if matches!(&message, crate::run::Protocol::Repaint) {
                    self.upload_tty_as_pixels().await?;
                }

                self.gpu().handle_protocol_message(&message).await?;
                self.tattoy_mut().handle_common_protocol_messages(message)?;
            }
            Err(error) => tracing::error!("Receiving protocol message: {error:?}"),
        }

        Ok(())
    }

    /// Upload the TTY content as coloured pixels.
    async fn upload_tty_as_pixels(&mut self) -> Result<()> {
        let is_upload_tty_as_pixels = self.is_upload_tty_as_pixels().await;
        let image = self
            .tattoy_mut()
            .get_tty_image_for_upload(is_upload_tty_as_pixels)?;
        self.gpu().update_ichannel_texture_data(&image);

        Ok(())
    }

    /// Tick the render
    async fn render(&mut self) -> Result<()> {
        let cursor = self.tattoy().screen.surface.cursor_position();
        let cursor_scale = self.get_cursor_scale().await;
        self.gpu()
            .update_cursor_position(cursor.0.try_into()?, cursor.1.try_into()?, cursor_scale);

        self.tattoy_mut().initialise_surface();
        self.tattoy_mut().opacity = self.get_opacity().await;
        self.tattoy_mut().layer = self.get_layer().await;
        let image = self.gpu().render().await?;

        let tty_height_in_pixels = u32::from(self.tattoy().height) * 2;
        for y in 0..tty_height_in_pixels {
            for x in 0..self.tattoy().width {
                let offset_for_reversal = 1;
                let y_reversed = tty_height_in_pixels - y - offset_for_reversal;
                let pixel = image
                    .get_pixel_checked(x.into(), y_reversed)
                    .context(format!("Couldn't get pixel: {x}x{y_reversed}"))?
                    .0;

                self.tattoy_mut()
                    .surface
                    .add_pixel(x.into(), y.try_into()?, pixel.into())?;
            }
        }

        self.tattoy_mut().send_output().await?;

        Ok(())
    }
}
