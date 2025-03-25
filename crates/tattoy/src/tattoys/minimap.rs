//! Display a minimap of the scrollback history.

use std::sync::Arc;

use color_eyre::eyre::{ContextCompat as _, Result};
use shadow_terminal::output::SurfaceKind;

use super::tattoyer::Tattoyer;

/// User-configurable settings for the minimap
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct Config {
    /// Enable/disable the minimap
    pub enabled: bool,
    /// The max width of the minimap (in units of terminal columns). The image resizer may choose a
    /// slimmer minimap in order to maintain the original aspect ratio.
    max_width: u16,
    /// The speed of the minimap show/hide animation.
    animation_speed: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            max_width: 15,
            animation_speed: 0.15,
        }
    }
}

/// The various states of the minimap UI.
#[derive(Debug)]
enum AnimationStep {
    /// The minimap is hidden.
    Hidden,
    /// The minimap is in the process of animating out.
    Showing(f32),
    /// The minimap is completely shown.
    Shown,
    /// The minimap is in the process of animating away.
    Hiding(f32),
}

/// `Minimap`
pub struct Minimap {
    /// The base Tattoy struct
    tattoy: Tattoyer,
    /// A cached version of the scrollback minimap.
    scrollback: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
    /// A cached version of the screen minimap.
    screen: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
    /// Shared app state
    state: Arc<crate::shared_state::SharedState>,
    /// If the PTY output has changed.
    output_changed: bool,
    /// The current state of any UI transitions; fading, sliding, etc.
    animation_step: AnimationStep,
}

impl Minimap {
    /// Instantiate
    fn new(
        output_channel: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
        state: Arc<crate::shared_state::SharedState>,
    ) -> Self {
        let tattoy = Tattoyer::new("minimap".to_owned(), 90, output_channel);
        Self {
            tattoy,
            scrollback: image::ImageBuffer::default(),
            screen: image::ImageBuffer::default(),
            state,
            output_changed: true,
            animation_step: AnimationStep::Hidden,
        }
    }

    /// Our main entrypoint.
    pub(crate) async fn start(
        protocol_tx: tokio::sync::broadcast::Sender<crate::run::Protocol>,
        output: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
        state: Arc<crate::shared_state::SharedState>,
    ) -> Result<()> {
        let mut minimap = Self::new(output, state);
        let mut protocol = protocol_tx.subscribe();

        #[expect(
            clippy::integer_division_remainder_used,
            reason = "This is caused by the `tokio::select!`"
        )]
        loop {
            tokio::select! {
                () = minimap.tattoy.sleep_until_next_frame_tick(), if minimap.needs_rerendering() => {
                    minimap.render().await?;
                },
                result = protocol.recv() => {
                    if matches!(result, Ok(crate::run::Protocol::End)) {
                        break;
                    }
                    minimap.handle_protocol_message(result).await?;
                }
            }
        }

        Ok(())
    }

    /// Handle messages from the main Tattoy app.
    async fn handle_protocol_message(
        &mut self,
        result: std::result::Result<crate::run::Protocol, tokio::sync::broadcast::error::RecvError>,
    ) -> Result<()> {
        match result {
            Ok(message) => {
                self.check_if_mouse_is_over_right_columns(&message);
                self.tattoy
                    .handle_common_protocol_messages(message.clone())?;
                self.check_if_pty_has_changed(&message).await?;
            }
            Err(error) => tracing::error!("Receiving protocol message: {error:?}"),
        }

        Ok(())
    }

    /// Whether the minimap needs re-rendering.
    const fn needs_rerendering(&self) -> bool {
        self.output_changed || !self.is_hidden()
    }

    /// Check if the scrollback output has changed such that we need to trigger a re-render.
    fn check_if_mouse_is_over_right_columns(&mut self, message: &crate::run::Protocol) {
        let crate::run::Protocol::Input(input) = message else {
            return;
        };

        #[expect(
            clippy::single_match,
            clippy::wildcard_enum_match_arm,
            reason = "We're ready to add handlers for other messages"
        )]
        match &input.event {
            termwiz::input::InputEvent::Mouse(mouse) => {
                if self.is_hidden() && mouse.x > self.tattoy.width - 2 {
                    self.show();
                }

                let is_mouse_outside_minimap = u32::from(mouse.x) - 1
                    < u32::from(self.tattoy.width) - self.scrollback.dimensions().0;
                if self.is_shown() && is_mouse_outside_minimap {
                    self.hide();
                }
            }
            _ => (),
        }
    }

    /// Whether thje minimap is completely hidden.
    const fn is_hidden(&self) -> bool {
        matches!(self.animation_step, AnimationStep::Hidden)
    }

    /// Whether thje minimap is completely shown.
    const fn is_shown(&self) -> bool {
        matches!(self.animation_step, AnimationStep::Shown)
    }

    /// Show the minimap.
    fn show(&mut self) {
        if matches!(self.animation_step, AnimationStep::Hidden) {
            self.animation_step = AnimationStep::Showing(0.0);
            tracing::trace!("Minimap set to: {:?}", self.animation_step);
        }
    }

    /// Hide the minimap.
    fn hide(&mut self) {
        if matches!(self.animation_step, AnimationStep::Shown) {
            self.animation_step = AnimationStep::Hiding(1.0);
            tracing::trace!("Minimap set to: {:?}", self.animation_step);
        }
    }

    // TODO:
    //   Currently this builds the minimap even when it's not visible. Perhaps default
    //   to not building unless visible, and provide a config option?
    //
    /// Check if the PTY output has changed such that we need to trigger a re-render.
    async fn check_if_pty_has_changed(&mut self, message: &crate::run::Protocol) -> Result<()> {
        if Tattoyer::is_scrollback_output_changed(message) {
            self.build_minimap(shadow_terminal::output::SurfaceKind::Scrollback)
                .await?;
            self.output_changed = true;
        }

        if Tattoyer::is_screen_output_changed(message) {
            self.build_minimap(shadow_terminal::output::SurfaceKind::Screen)
                .await?;
            self.output_changed = true;
        }
        Ok(())
    }

    /// Tick the render
    async fn render(&mut self) -> Result<()> {
        if !self.tattoy.is_ready() {
            tracing::trace!("Not rendering minimap as Tattoy isn't ready yet.");
            return Ok(());
        }

        let Some(transition_state) = self.get_transition_state().await else {
            return Ok(());
        };

        tracing::trace!("Rendering minimap.");

        self.tattoy.initialise_surface();

        let dimensions = self.scrollback.dimensions();
        let minimap_width = dimensions.0;
        let minimap_height = dimensions.1;

        #[expect(
            clippy::as_conversions,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "`as` is more convenient than adding a whole new crate, or using `unsafe`"
        )]
        let x_offset = { (minimap_width as f32 * (1.0 - transition_state)) as u32 };

        let tty_height_in_pixels = u32::from(self.tattoy.height) * 2;
        let empty_height = tty_height_in_pixels - minimap_height;

        for y in 0..tty_height_in_pixels {
            for x_minimap in 0..(minimap_width - x_offset) {
                let x_surface: usize =
                    (u32::from(self.tattoy.width) - minimap_width + x_minimap).try_into()?;

                let screen_minimap_height = self.screen.dimensions().1;
                let screen_minimap_offset = tty_height_in_pixels - screen_minimap_height;

                // Draw the empty, transparent part of the minimap at the top (if the minimap isn't
                // very big yet).
                if y < empty_height {
                    if y.rem_euclid(2) == 0 {
                        self.tattoy.surface.add_text(
                            x_surface + usize::try_from(x_offset)?,
                            y.div_euclid(2).try_into()?,
                            " ".to_owned(),
                            Some((0.2, 0.2, 0.2, 0.8)),
                            Some((0.0, 0.0, 0.0, 1.0)),
                        );
                    }

                // Draw the actual minimap pixels themselves.
                } else {
                    // Draw the scrollback minimap.
                    let mut pixel =
                        if y < screen_minimap_offset || !self.tattoy.is_alternate_screen() {
                            let y_image = y - empty_height;
                            self.scrollback
                                .get_pixel_checked(x_minimap, y_image)
                                .context(format!("Couldn't get pixel: {x_minimap}x{y_image}"))?
                                .0

                        // Draw the screen minimap.
                        } else {
                            let y_image = y - screen_minimap_offset;
                            self.screen
                                .get_pixel_checked(x_minimap, y_image)
                                .context(format!("Couldn't get pixel: {x_minimap}x{y_image}"))?
                                .0
                        };

                    // TODO: make configurable
                    pixel[3] = 0.95;

                    self.tattoy.surface.add_pixel(
                        x_surface + usize::try_from(x_offset)?,
                        y.try_into()?,
                        pixel.into(),
                    )?;
                }
            }
        }

        self.tattoy.send_output().await?;
        self.output_changed = false;

        Ok(())
    }

    /// Get the transition state of the minimap animation. Therefore whether it's hidden, animating in,
    /// animating out, or just plain showing.
    async fn get_transition_state(&mut self) -> Option<f32> {
        let animation_speed = self.state.config.read().await.minimap.animation_speed;

        let animation_state = match self.animation_step {
            AnimationStep::Hidden => {
                return None;
            }
            AnimationStep::Shown => {
                if !self.output_changed {
                    return None;
                }
                1.0
            }
            AnimationStep::Showing(offset) => {
                let new_offset = offset + animation_speed;
                if new_offset >= 1.0 {
                    self.animation_step = AnimationStep::Shown;
                } else {
                    self.animation_step = AnimationStep::Showing(new_offset);
                }
                tracing::trace!("Minimap set to: {:?}", self.animation_step);
                new_offset
            }
            AnimationStep::Hiding(offset) => {
                let new_offset = offset - animation_speed;
                if new_offset <= 0.0 {
                    self.animation_step = AnimationStep::Hidden;
                } else {
                    self.animation_step = AnimationStep::Hiding(new_offset);
                }
                tracing::trace!("Minimap set to: {:?}", self.animation_step);
                new_offset
            }
        };

        Some(animation_state.clamp(0.0, 1.0))
    }

    /// Build a minimap by converting terminal cells to a raw RGB image and then resizing the
    /// image.
    async fn build_minimap(&mut self, kind: SurfaceKind) -> Result<()> {
        let pixels_per_line = 2;

        let surface = match kind {
            SurfaceKind::Scrollback => &mut self.tattoy.scrollback.surface,
            SurfaceKind::Screen => &mut self.tattoy.screen.surface,
            _ => {
                color_eyre::eyre::bail!("Unkown surface kind: {kind:?}");
            }
        };
        let surface_width = surface.dimensions().0;
        let surface_height = surface.dimensions().1;

        tracing::trace!(
            "Rebuilding minimap for {kind:?}, size: {}x{}. Sample content:\n{:.200}\n...",
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

        let cells = &surface.screen_cells();
        for (x, y, pixel) in image_buffer.enumerate_pixels_mut() {
            let line = cells
                .get(usize::try_from(y)?.div_euclid(pixels_per_line))
                .context("Couldn't get surface line")?;

            let cell = &line
                .get(usize::try_from(x)?)
                .context("Couldn't get surface cell from line")?;

            let cell_colour = if cell.str() == " " {
                crate::opaque_cell::OpaqueCell::extract_colour(cell.attrs().background())
                    .map_or(crate::opaque_cell::DEFAULT_COLOUR, |background_colour| {
                        background_colour
                    })
            } else {
                let maybe_colour =
                    crate::opaque_cell::OpaqueCell::extract_colour(cell.attrs().foreground());

                if let Some(colour) = maybe_colour {
                    colour
                } else {
                    color_eyre::eyre::bail!("Using Minimap without a parsed palette");
                }
            };

            *pixel = image::Rgba(cell_colour.to_srgb_u8().into());
        }

        let max_width = self.state.config.read().await.minimap.max_width;
        let minimap = image
            .resize(
                max_width.into(),
                (self.tattoy.height * 2).into(),
                image::imageops::Lanczos3,
            )
            .to_rgba32f();

        match kind {
            SurfaceKind::Scrollback => self.scrollback = minimap,
            SurfaceKind::Screen => self.screen = minimap,
            _ => {
                color_eyre::eyre::bail!("Unknown surface kind: {kind:?}");
            }
        }

        Ok(())
    }
}
