//! Convert palette indexes to true colour values.

use color_eyre::{eyre::ContextCompat as _, Result};
use shadow_terminal::termwiz;

/// Convenience type for the palette hash.
#[derive(Clone)]
pub(crate) struct Palette {
    /// The palette hash.
    pub map: super::main::PaletteHashMap,
}

impl Palette {
    /// Convert a palette index to a Termwiz-compatible true colour.
    pub fn true_colour_attribute_from_index(&self, index: u8) -> termwiz::color::ColorAttribute {
        let srgba = self.true_colour_tuple_from_index(index);
        termwiz::color::ColorAttribute::TrueColorWithPaletteFallback(srgba, index)
    }

    // TODO: index is no longer `u8` it's just a `String`.
    /// Convert a palette index to a Termwiz-compatible true colour.
    pub fn true_colour_tuple_from_index(&self, index: u8) -> termwiz::color::SrgbaTuple {
        #[expect(
            clippy::expect_used,
            reason = "Unreachable because a palette should only have 256 colours"
        )]
        let true_colour = self
            .map
            .get(&index.to_string())
            .expect("Palette contains less than 256 colours");
        Self::palette_colour_to_srgba(*true_colour)
    }

    /// Convert a palette colour to a Termwiz SRGBA tuple.
    fn palette_colour_to_srgba(colour: super::main::PaletteColour) -> termwiz::color::SrgbaTuple {
        termwiz::color::RgbColor::new_8bpc(colour.0, colour.1, colour.2).into()
    }

    /// The background colour defined by a cell merely not having any other colour set for its
    /// background.
    pub fn background_colour(&self) -> termwiz::color::SrgbaTuple {
        if let Some(colour) = self.map.get(super::main::BACKGROUND_COLOUR_KEY) {
            return Self::palette_colour_to_srgba(*colour);
        }

        // Terminal emulator convention is that the default background colour is the first colour
        // in the terminal's palette.
        //
        // TODO: remove this once the screenshot parser supports background colour parsing.
        self.true_colour_tuple_from_index(0)
    }

    /// The foreground colour defined by a cell merely not having any other colour set for its
    /// foreground.
    pub fn foreground_colour(&self) -> termwiz::color::SrgbaTuple {
        if let Some(colour) = self.map.get(super::main::FOREGROUND_COLOUR_KEY) {
            return Self::palette_colour_to_srgba(*colour);
        }

        // There's a nice history of the default foreground (and background) colours in this comment
        // from the Microsoft Terminal repo:
        // https://github.com/microsoft/terminal/discussions/14142#discussioncomment-3812803
        //
        // TODO: remove this once the screenshot parser supports background colour parsing.
        self.true_colour_tuple_from_index(7)
    }

    /// Print all the true colour versions of the terminal's palette as found in the screenshot.
    #[expect(
        clippy::print_stdout,
        reason = "We're printing the final parsed palette for the user to confirm."
    )]
    pub fn print_true_colour_palette(&self) -> Result<()> {
        println!();
        println!("These colours should match the colours above:");
        crate::palette::parser::Parser::print_generic_palette(|palette_index| -> Result<()> {
            let bg = self
                .map
                .get(&palette_index.to_string())
                .context("Palette colour not found")?;
            let fg = self
                .map
                .get(&(palette_index + crate::palette::parser::PALETTE_ROW_SIZE).to_string())
                .context("Palette colour not found")?;
            crate::palette::parser::Parser::print_2_true_colours_in_1(
                (bg.0, bg.1, bg.2),
                (fg.0, fg.1, fg.2),
            );
            Ok(())
        })
    }

    /// Convert any palette index-defined cells to their true colour values.
    pub fn cell_attributes_to_true_colour(&self, attributes: &mut termwiz::cell::CellAttributes) {
        self.convert_fg_to_true_colour(attributes);
        self.convert_bg_to_true_colour(attributes);
    }

    /// Convert text palette indexes to true colour values.
    fn convert_fg_to_true_colour(&self, attributes: &mut termwiz::cell::CellAttributes) {
        if matches!(
            attributes.foreground(),
            termwiz::color::ColorAttribute::Default
        ) {
            let colour_attribute = termwiz::color::ColorAttribute::TrueColorWithDefaultFallback(
                self.foreground_colour(),
            );
            attributes.set_foreground(colour_attribute);
            return;
        }

        let termwiz::color::ColorAttribute::PaletteIndex(index) = attributes.foreground() else {
            return;
        };

        let colour_attribute = self.true_colour_attribute_from_index(index);
        attributes.set_foreground(colour_attribute);
    }

    /// Convert the background palette index to a true colour. Note that we don't handle the
    /// default colour variant because that's currently used to help with the compositing of render
    /// layers, namely knowing when to let a lower layer's content pass through to higher layers.
    /// But it might turn out to be a better idea to also make transparent cells use true colour,
    /// because they could easily be defined with a `0.0` alpha channel.
    fn convert_bg_to_true_colour(&self, attributes: &mut termwiz::cell::CellAttributes) {
        let termwiz::color::ColorAttribute::PaletteIndex(index) = attributes.background() else {
            return;
        };

        let colour_attribute = self.true_colour_attribute_from_index(index);
        attributes.set_background(colour_attribute);
    }

    /// Convert TTY cell palette indexes into their true colour values.
    pub fn convert_cells_to_true_colour(
        &self,
        output: &mut shadow_terminal::output::native::Output,
    ) {
        match output {
            shadow_terminal::output::native::Output::Diff(surface_diff) => {
                let changes = match surface_diff {
                    shadow_terminal::output::native::SurfaceDiff::Scrollback(diff) => {
                        &mut diff.changes
                    }
                    shadow_terminal::output::native::SurfaceDiff::Screen(diff) => &mut diff.changes,
                    _ => {
                        tracing::error!(
                            "Unrecognised surface diff when converting cells to true colour"
                        );
                        &mut Vec::new()
                    }
                };

                for change in changes {
                    if let termwiz::surface::change::Change::AllAttributes(attributes) = change {
                        self.cell_attributes_to_true_colour(attributes);
                    }
                }
            }
            shadow_terminal::output::native::Output::Complete(complete_surface) => {
                let cells = match complete_surface {
                    shadow_terminal::output::native::CompleteSurface::Scrollback(scrollback) => {
                        scrollback.surface.screen_cells()
                    }
                    shadow_terminal::output::native::CompleteSurface::Screen(screen) => {
                        screen.surface.screen_cells()
                    }
                    _ => {
                        tracing::error!("Unhandled surface from Shadow Terminal");
                        Vec::new()
                    }
                };
                for line in cells {
                    for cell in line {
                        self.cell_attributes_to_true_colour(cell.attrs_mut());
                    }
                }
            }
            _ => (),
        }
    }
}
