//! Shared state and behaviour useful to all tattoys.#

use color_eyre::eyre::{ContextCompat as _, Result};

/// Shared state and behaviour useful to all tattoys.
pub(crate) struct Tattoyer {
    /// A unique identifier.
    pub id: String,
    /// The compositing layer that the tattoy is rendered to. 0 is the PTY screen itself.
    pub layer: i16,
    /// The transparency of layer.
    pub opacity: f32,
    /// The application shared state
    pub state: std::sync::Arc<crate::shared_state::SharedState>,
    /// A channel to send final rendered output.
    pub output_channel: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
    /// The surface on which to construct this tattoy's frame.
    pub surface: crate::surface::Surface,
    /// TTY width
    pub width: u16,
    /// TTY height
    pub height: u16,
    /// Our own copy of the scrollback. Saves taking costly read locks.
    pub scrollback: shadow_terminal::output::native::CompleteScrollback,
    /// Our own copy of the screen. Saves taking costly read locks.
    pub screen: shadow_terminal::output::native::CompleteScreen,
    /// The target frame rate.
    pub target_frame_rate: u32,
    /// The time at which the previous frame was rendererd.
    pub last_frame_tick: tokio::time::Instant,
    /// The last known position of an active scroll.
    pub last_scroll_position: usize,
}

impl Tattoyer {
    /// Instantiate
    pub(crate) async fn new(
        id: String,
        state: std::sync::Arc<crate::shared_state::SharedState>,
        layer: i16,
        opacity: f32,
        output_channel: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
    ) -> Self {
        let tty_size = state.get_tty_size().await;
        let target_frame_rate = state.config.read().await.frame_rate;
        Self {
            id: id.clone(),
            layer,
            opacity,
            state,
            output_channel,
            surface: crate::surface::Surface::new(id, 0, 0, layer, opacity),
            width: tty_size.width,
            height: tty_size.height,
            scrollback: shadow_terminal::output::native::CompleteScrollback::default(),
            screen: shadow_terminal::output::native::CompleteScreen::default(),
            target_frame_rate,
            last_frame_tick: tokio::time::Instant::now(),
            last_scroll_position: 0,
        }
    }

    /// Create an empty surface ready for building a new frame.
    pub fn initialise_surface(&mut self) {
        self.surface = crate::surface::Surface::new(
            self.id.clone(),
            self.width.into(),
            self.height.into(),
            self.layer,
            self.opacity,
        );
    }

    /// Keep track of the size of the underlying terminal.
    pub const fn set_tty_size(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
    }

    /// Handle commom protocol messages, like resizing and new output from the underlying terminal.
    pub(crate) fn handle_common_protocol_messages(
        &mut self,
        message: crate::run::Protocol,
    ) -> Result<()> {
        tracing::trace!(
            "'{}' tattoy recevied protocol message: {message:?}",
            self.id
        );

        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "We're just handling the common cases here."
        )]
        match message {
            crate::run::Protocol::Resize { width, height } => {
                self.set_tty_size(width, height);
            }
            crate::run::Protocol::Output(output) => self.handle_pty_output(output)?,
            crate::run::Protocol::Config(config) => self.target_frame_rate = config.frame_rate,
            _ => (),
        }

        Ok(())
    }

    /// Whether the user is scolling.
    pub const fn is_scrolling(&self) -> bool {
        self.scrollback.position != 0
    }

    /// Has scolling just ended?
    pub const fn is_scrolling_end(&self) -> bool {
        self.last_scroll_position != 0 && !self.is_scrolling()
    }

    /// Is the underlying terminal in the alternate screen.
    pub const fn is_alternate_screen(&self) -> bool {
        matches!(
            self.screen.mode,
            shadow_terminal::output::native::ScreenMode::Alternate
        )
    }

    /// Handle new output from the underlying PTY.
    pub fn handle_pty_output(
        &mut self,
        output: shadow_terminal::output::native::Output,
    ) -> Result<()> {
        match output {
            shadow_terminal::output::native::Output::Diff(diff) => match diff {
                shadow_terminal::output::native::SurfaceDiff::Scrollback(scrollback_diff) => {
                    self.scrollback
                        .surface
                        .resize(scrollback_diff.size.0, scrollback_diff.height);
                    self.scrollback.surface.add_changes(scrollback_diff.changes);
                    self.scrollback.position = scrollback_diff.position;
                }
                shadow_terminal::output::native::SurfaceDiff::Screen(screen_diff) => {
                    self.screen
                        .surface
                        .resize(screen_diff.size.0, screen_diff.size.1);
                    self.set_tty_size(
                        screen_diff.size.0.try_into()?,
                        screen_diff.size.1.try_into()?,
                    );
                    self.screen.surface.add_changes(screen_diff.changes);
                }
                _ => (),
            },
            shadow_terminal::output::native::Output::Complete(complete) => match complete {
                shadow_terminal::output::native::CompleteSurface::Scrollback(
                    complete_scrollback,
                ) => {
                    self.scrollback = complete_scrollback;
                }
                shadow_terminal::output::native::CompleteSurface::Screen(complete_screen) => {
                    self.screen = complete_screen;
                }
                _ => (),
            },
            _ => (),
        }

        Ok(())
    }

    /// Send the final surface to the main renderer.
    pub(crate) async fn send_output(&mut self) -> Result<()> {
        self.output_channel
            .send(crate::run::FrameUpdate::TattoySurface(self.surface.clone()))
            .await?;

        self.last_scroll_position = self.scrollback.position;

        Ok(())
    }

    /// Send a blank frame to the renderer.
    pub async fn send_blank_output(&mut self) -> Result<()> {
        self.initialise_surface();
        self.surface.width = 0;
        self.surface.height = 0;
        self.send_output().await
    }

    /// Sleep until the next frame render is due.
    pub async fn sleep_until_next_frame_tick(&mut self) {
        let target = crate::renderer::ONE_MICROSECOND.wrapping_div(self.target_frame_rate.into());
        let target_frame_rate_micro = std::time::Duration::from_micros(target);
        if let Some(wait) = target_frame_rate_micro.checked_sub(self.last_frame_tick.elapsed()) {
            tokio::time::sleep(wait).await;
        }
        self.last_frame_tick = tokio::time::Instant::now();
    }

    /// Check if the scrollback output has changed.
    pub const fn is_scrollback_output_changed(message: &crate::run::Protocol) -> bool {
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "We only want to react to messages that cause output changes"
        )]
        match message {
            crate::run::Protocol::Resize { .. } => return true,
            crate::run::Protocol::Output(output) => match output {
                shadow_terminal::output::native::Output::Diff(
                    shadow_terminal::output::native::SurfaceDiff::Scrollback(diff),
                ) => {
                    // There is always one change to indicate the current position of the cursor.
                    if diff.changes.len() > 1 {
                        return true;
                    }
                }
                shadow_terminal::output::native::Output::Complete(
                    shadow_terminal::output::native::CompleteSurface::Scrollback(_),
                ) => {
                    return true;
                }
                _ => (),
            },
            _ => (),
        }

        false
    }

    /// Check if the screen output has changed.
    pub const fn is_screen_output_changed(message: &crate::run::Protocol) -> bool {
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "We only want to react to messages that cause output changes"
        )]
        match message {
            crate::run::Protocol::Resize { .. } => return true,
            crate::run::Protocol::Output(output) => match output {
                shadow_terminal::output::native::Output::Diff(
                    shadow_terminal::output::native::SurfaceDiff::Screen(diff),
                ) => {
                    // There is always one change to indicate the current position of the cursor.
                    if diff.changes.len() > 1 {
                        return true;
                    }
                }
                shadow_terminal::output::native::Output::Complete(
                    shadow_terminal::output::native::CompleteSurface::Screen(_),
                ) => {
                    return true;
                }
                _ => (),
            },
            _ => (),
        }

        false
    }

    /// Has the contents of the PTY changed?
    pub const fn is_pty_changed(
        message: &crate::run::Protocol,
    ) -> Option<shadow_terminal::output::native::SurfaceKind> {
        if Self::is_scrollback_output_changed(message) {
            return Some(shadow_terminal::output::native::SurfaceKind::Scrollback);
        }
        if Self::is_screen_output_changed(message) {
            return Some(shadow_terminal::output::native::SurfaceKind::Screen);
        }

        None
    }

    /// Convert the PTY's contents to a pixel image representation.
    pub async fn convert_pty_to_pixel_image(
        &mut self,
        kind: &shadow_terminal::output::native::SurfaceKind,
        is_convert_characters: bool,
    ) -> Result<image::DynamicImage> {
        let pixels_per_line = 2;
        let default_background_colour = *self.state.default_background.read().await;

        let surface = match kind {
            shadow_terminal::output::native::SurfaceKind::Scrollback => {
                &mut self.scrollback.surface
            }
            shadow_terminal::output::native::SurfaceKind::Screen => &mut self.screen.surface,
            _ => {
                color_eyre::eyre::bail!("Unkown surface kind: {kind:?}");
            }
        };
        let surface_width = surface.dimensions().0;
        let surface_height = surface.dimensions().1;

        tracing::trace!(
            "Converting PTY of {kind:?} to a pixels, size: {}x{}. Sample content:\n{:.200}\n...",
            surface_width,
            surface_height,
            surface.screen_chars_to_string()
        );

        let mut image = image::DynamicImage::new_rgba8(
            surface_width.try_into()?,
            (surface_height * pixels_per_line).try_into()?,
        );
        let image_buffer = image
            .as_mut_rgba8()
            .context("Couldn't get mutable reference to scrollback image")?;

        let cells = &surface.get_screen_cells();
        for (x, y, pixel) in image_buffer.enumerate_pixels_mut() {
            let line = cells
                .get(usize::try_from(y)?.div_euclid(pixels_per_line))
                .context("Couldn't get surface line")?;

            let cell = &line
                .get(usize::try_from(x)?)
                .context("Couldn't get surface cell from line")?;

            let cell_colour = if cell.str() == " " {
                crate::blender::Blender::extract_colour(cell.attrs().background())
                    .unwrap_or(default_background_colour)
            } else if is_convert_characters {
                crate::blender::Blender::extract_colour(cell.attrs().foreground()).unwrap_or(
                    // TODO: use the actual default foreground colour from the palette.
                    shadow_terminal::termwiz::color::SrgbaTuple(1.0, 1.0, 1.0, 1.0),
                )
            } else {
                crate::blender::Blender::extract_colour(cell.attrs().background())
                    .unwrap_or(default_background_colour)
            };

            *pixel = image::Rgba(cell_colour.to_srgb_u8().into());
        }

        Ok(image)
    }

    /// Depending on whether the `upload_tty_as_pixels` config is set by the user, decide what to
    /// send the GPU in order to represent the terminal contents.
    pub async fn get_tty_image_for_upload(
        &mut self,
        is_upload_tty_as_pixels: bool,
        is_upload_characters: bool,
    ) -> Result<image::RgbaImage> {
        let image = if is_upload_tty_as_pixels {
            self.convert_pty_to_pixel_image(
                &shadow_terminal::output::native::SurfaceKind::Screen,
                is_upload_characters,
            )
            .await?
            .flipv()
            .into()
        } else {
            self.pure_black_image()
        };

        Ok(image)
    }

    /// A "blank" image for when the user doesn't want to upload the TTY but also wants to support
    /// shaders that use `iChannel0`.
    fn pure_black_image(&self) -> image::RgbaImage {
        image::ImageBuffer::from_fn(self.width.into(), u32::from(self.height) * 2, |_, _| {
            // TODO: Does this need to use the default background colour from the palette?
            [0, 0, 0, 255].into()
        })
    }
}
