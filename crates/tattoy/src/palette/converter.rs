//! Convert palette indexes to true colour values.

use color_eyre::{eyre::ContextCompat as _, Result};

/// This might be a big assumption, but I think the convention is that text uses this colour from
/// the palette when no other index or true colour is specified.
const DEFAULT_TEXT_PALETTE_INDEX: u8 = 15;

/// A single palette colour.
type PaletteColour = (u8, u8, u8);

/// A hash of palette indexes to true colour values.
pub type PaletteHashMap = std::collections::HashMap<String, PaletteColour>;

/// Convenience type for the palette hash.
pub(crate) struct Palette {
    /// The palette hash.
    pub map: PaletteHashMap,
}

impl Palette {
    /// Convert a palette index to a Termwiz-compatible true colour.
    pub fn true_colour_from_index(&self, index: u8) -> termwiz::color::ColorAttribute {
        #[expect(
            clippy::expect_used,
            reason = "Unreachable because a palette should only have 256 colours"
        )]
        let true_colour = self
            .map
            .get(&index.to_string())
            .expect("Palette contains less than 256 colours");
        let srgba: termwiz::color::SrgbaTuple =
            termwiz::color::RgbColor::new_8bpc(true_colour.0, true_colour.1, true_colour.2).into();
        termwiz::color::ColorAttribute::TrueColorWithPaletteFallback(srgba, index)
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
            let colour_attribute = self.true_colour_from_index(DEFAULT_TEXT_PALETTE_INDEX);
            attributes.set_foreground(colour_attribute);
            return;
        }

        let termwiz::color::ColorAttribute::PaletteIndex(index) = attributes.foreground() else {
            return;
        };

        let colour_attribute = self.true_colour_from_index(index);
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

        let colour_attribute = self.true_colour_from_index(index);
        attributes.set_background(colour_attribute);
    }
}
