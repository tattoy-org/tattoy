//! A trait for tattoys that use shaders.

use color_eyre::eyre::{ContextCompat as _, Result};
use futures_util::FutureExt as _;

/// Common logic for tattoys that render shaders.
pub(crate) trait Shaderer: Sized {
    /// Return an immutable reference to the Tattoyer helper.
    fn tattoy(&self) -> &crate::tattoys::tattoyer::Tattoyer;

    /// Return a mutable reference to the Tattoyer helper.
    fn tattoy_mut(&mut self) -> &mut crate::tattoys::tattoyer::Tattoyer;

    /// Returns an immutable reference to the GPU pipeline manager.
    fn gpu(&self) -> &super::pipeline::GPU;

    /// Returns a mutable reference to the GPU pipeline manager.
    fn gpu_mut(&mut self) -> &mut super::pipeline::GPU;

    /// Is the config for this tattoy set to upload the TTY as pixels?
    async fn is_upload_tty_as_pixels(&self) -> bool;

    /// Should the character colours be uploaded as part of the TTY pixels?
    fn is_upload_tty_with_characters(&self) -> bool;

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

                self.gpu_mut().handle_protocol_message(&message).await?;
                self.tattoy_mut().handle_common_protocol_messages(message)?;
            }
            Err(error) => tracing::error!("Receiving protocol message: {error:?}"),
        }

        Ok(())
    }

    /// Upload the TTY content as coloured pixels.
    async fn upload_tty_as_pixels(&mut self) -> Result<()> {
        let is_upload_tty_as_pixels = self.is_upload_tty_as_pixels().await;
        let is_upload_tty_with_characters = self.is_upload_tty_with_characters();
        self.gpu_mut().tty_pixels = self
            .tattoy_mut()
            .get_tty_image_for_upload(is_upload_tty_as_pixels, is_upload_tty_with_characters)
            .await?;
        self.gpu_mut().update_ichannel_texture_data();

        Ok(())
    }

    /// Tick the render
    async fn render(&mut self) -> Result<()> {
        let rendered_pixels = self.gpu_mut().render().await?;

        if self.is_upload_tty_as_pixels().await {
            if self.gpu().tty_pixels.dimensions().1 == 0 {
                tracing::trace!("Not building shader layer because TTY pixels aren't ready");
                return Ok(());
            }

            if self.gpu().tty_pixels.dimensions() != rendered_pixels.dimensions() {
                tracing::trace!(
                    "Not building shader layer because TTY pixels aren't the same dimensions \
                    as the GPU-rendered pixels."
                );
                return Ok(());
            }
        }

        let cursor_position = self.tattoy().screen.surface.cursor_position();
        let cursor_colour = self.get_cursor_colour(cursor_position.0, cursor_position.1)?;

        let cursor_scale = self.get_cursor_scale().await;
        self.gpu_mut().update_cursor(
            cursor_position.0.try_into()?,
            cursor_position.1.try_into()?,
            cursor_colour,
            cursor_scale,
        );

        self.tattoy_mut().initialise_surface();
        self.tattoy_mut().opacity = self.get_opacity().await;
        self.tattoy_mut().layer = self.get_layer().await;

        let tty_height_in_pixels = u32::from(self.tattoy().height) * 2;
        for y in 0..tty_height_in_pixels {
            for x in 0..self.tattoy().width {
                let offset_for_reversal = 1;
                let y_reversed = tty_height_in_pixels - y - offset_for_reversal;

                let pixel_u8 = rendered_pixels
                    .get_pixel_checked(x.into(), y_reversed)
                    .context(format!("Couldn't get new pixel: {x}x{y_reversed}"))?
                    .0;
                let pixel = [
                    f32::from(pixel_u8[0]) / 255.0,
                    f32::from(pixel_u8[1]) / 255.0,
                    f32::from(pixel_u8[2]) / 255.0,
                    f32::from(pixel_u8[3]) / 255.0,
                ];

                if self.is_upload_tty_as_pixels().await {
                    if self.are_pixels_different(x.into(), y_reversed, pixel_u8)? {
                        self.tattoy_mut().surface.add_pixel(
                            x.into(),
                            y.try_into()?,
                            pixel.into(),
                        )?;
                    }
                } else {
                    self.tattoy_mut()
                        .surface
                        .add_pixel(x.into(), y.try_into()?, pixel.into())?;
                }
            }
        }

        self.tattoy_mut().send_output().await?;

        Ok(())
    }

    /// Compare the pixel before and after rendering.
    fn are_pixels_different(&self, x: u32, y_reversed: u32, new_pixel: [u8; 4]) -> Result<bool> {
        let old_pixel = self
            .gpu()
            .tty_pixels
            .get_pixel_checked(x, y_reversed)
            .context(format!("Couldn't get old pixel: {x}x{y_reversed}"))?
            .0;
        Ok(new_pixel != old_pixel)
    }

    /// Get the current colour of the cursor.
    fn get_cursor_colour(&mut self, x: usize, y: usize) -> Result<[f32; 4]> {
        let cells = self.tattoy().screen.surface.get_screen_cells();
        if cells.is_empty() {
            return Ok([0.0, 0.0, 0.0, 0.0]);
        }

        let cursor_colour_attribute = cells
            .get(y)
            .context(format!(
                "Couldn't get y coordinate ({y}) for cursor cell. Line count: {}",
                cells.len()
            ))?
            .get(x)
            .context("Couldn't get x coordinate for cursor cell")?
            .attrs()
            .foreground();

        let colour: [f32; 4] = crate::blender::Blender::extract_colour(cursor_colour_attribute)
            .context("Couldn't get colour of cursor cell")?
            .to_tuple_rgba()
            .into();

        Ok(colour)
    }
}
