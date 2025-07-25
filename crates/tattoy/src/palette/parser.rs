//! Print out the terminal's palette, take a screenshot and try to parse the true colour values for
//! each member of the palette.
//!
//! The best source for how terminal palettes work is this Stack Overflow answer:
//!   <https://stackoverflow.com/a/27165165/575773>
//!
//! The reason we need this is that the default colours of a terminal are only expressed by their
//! palette index, which doesn't actually give us a colour value that we can use to do things like
//! alpha blending, interpolation etc. So what Tattoy can do is learn to associate each palette
//! index referenced by the PTY with a true colour value. That way the terminal retains the exact
//! palette configured by the user, whilst also being able to do colour maths on the palette.

use std::io::Write as _;

use color_eyre::Result;

/// Convenience type for screenshot image.
pub type Screenshot = xcap::image::ImageBuffer<xcap::image::Rgba<u8>, std::vec::Vec<u8>>;

/// The number of palette colours we put in each row of our "QR code".
pub const PALETTE_ROW_SIZE: u8 = 16;

/// A parser for converting default terminal palette colours to true colours.
pub(crate) struct Parser;

#[expect(
    clippy::print_stdout,
    reason = "We need to print the terminal's palette"
)]
impl Parser {
    /// Main entrypoint
    pub async fn run(
        state: &std::sync::Arc<crate::shared_state::SharedState>,
        maybe_user_screenshot: Option<&String>,
    ) -> Result<()> {
        let screenshot = match maybe_user_screenshot {
            Some(path) => {
                Self::print_native_palette()?;
                println!("Parsing screenshot file at: {path}...");

                xcap::image::open(path)?.into_rgba8()
            }
            None => match Self::take_screenshot(state).await? {
                Some(screenshot) => screenshot,
                // We just added the default palette, so we're ready to start Tattoy.
                None => return Ok(()),
            },
        };
        let result = super::state_machine::Machine::parse_screenshot(&screenshot);
        let palette = match result {
            Ok(palette) => palette,
            Err(error) => {
                if maybe_user_screenshot.is_none() {
                    let path = crate::config::main::Config::temporary_file("screenshot.png")?;
                    screenshot.save(path.clone())?;

                    color_eyre::eyre::bail!(
                        "\
                        Couldn't parse palette, screenshot saved to: {path:?}. \
                        You may also make your own screenshot and provide it with \
                        `tattoy --parse-palette screenshot.png`.
                        "
                    );
                } else {
                    color_eyre::eyre::bail!("Palette parsing failed: {error:?}");
                }
            }
        };

        palette.print_true_colour_palette()?;
        if Self::ask_if_palettes_look_the_same()? {
            super::main::save(state, &palette).await?;
        }

        Ok(())
    }

    /// Show the parsed palette and ask the user if it looks like their actual palette.
    fn ask_if_palettes_look_the_same() -> Result<bool> {
        print!("If the palettes look the same press 'y' to save: ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        println!();

        if answer != format!("y{}", crate::utils::NEWLINE) {
            println!("'y' not selected, exiting...");
            return Ok(true);
        }

        Ok(false)
    }

    /// Print all the colours of the terminal to STDOUT.
    fn print_native_palette() -> Result<()> {
        Self::print_rainbow();
        println!(
            "These are all the colors in your terminal's palette \
            (the red and blue columns are for the parser):"
        );
        Self::print_generic_palette(|palette_index| -> Result<()> {
            let background_colour = palette_index;
            let foreground_colour = palette_index + PALETTE_ROW_SIZE;
            print!(
                "\x1b[48;5;{background_colour}m\x1b[38;5;{foreground_colour}m▄{}",
                crate::utils::RESET_COLOUR
            );
            Ok(())
        })?;

        Ok(())
    }

    /// Print a helpful ANSI true colour word so that users can easily tell if their terminal
    /// supports true colour.
    fn print_rainbow() {
        let rainbow = "\
           \x1b[38;2;255;0;0mr\
           \x1b[38;2;255;127;0ma\
           \x1b[38;2;255;255;0mi\
           \x1b[38;2;0;255;0mn\
           \x1b[38;2;0;0;255mb\
           \x1b[38;2;75;0;130mo\
           \x1b[38;2;148;0;211mw\
           \x1b[0m";
        print!(
            "First, check that each letter in the word 'rainbow' is different: {rainbow}\n\
            If not, then your terminal does not support (or hasn't enabled) true colors and \
            Tattoy will not work.\n\n"
        );
    }

    /// Print out all the colours of a terminal palette in a sqaure, that both looks pretty and
    /// conforms to the QR Code-like requirements of parsing.
    pub fn print_generic_palette<F: Fn(u8) -> Result<()>>(callback: F) -> Result<()> {
        let pure_blue = (0, 0, 255);
        println!("╭──────────────────╮");
        for y in 0u8..8 {
            print!("│");

            // Print the pure(ish) red that indicates the start of a valid palette row.
            Self::print_2_true_colours_in_1((255, y * 2, 0), (255, (y * 2) + 1, 0));
            // Print the pure blue that helps us avoid false positives.
            Self::print_2_true_colours_in_1(pure_blue, pure_blue);

            for x in 0..PALETTE_ROW_SIZE {
                let palette_index = (y * PALETTE_ROW_SIZE * 2) + x;
                callback(palette_index)?;
            }
            print!("│");
            println!();
        }
        println!("╰──────────────────╯");
        std::io::stdout().flush()?;

        Ok(())
    }

    /// Use the UTF-8 half block trick to print 2 colours in one cell.
    pub fn print_2_true_colours_in_1(top: (u8, u8, u8), bottom: (u8, u8, u8)) {
        print!(
            "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m▄{}",
            top.0,
            top.1,
            top.2,
            bottom.0,
            bottom.1,
            bottom.2,
            crate::utils::RESET_COLOUR
        );
    }

    /// Take a screenshot of the current monitor.
    async fn take_screenshot(
        state: &std::sync::Arc<crate::shared_state::SharedState>,
    ) -> Result<Option<Screenshot>> {
        println!("{}", crate::utils::RESET_SCREEN);

        if !super::main::palette_config_exists(state).await {
            print!(
                "Automatically querying your terminal's palette failed. This most often \
                happens on older terminals or when running inside a multiplexer like `tmux`. \
                So you may have success trying again with a different setup. Note that once \
                palette parsing has succeeded you can return to your preffered setup as normal. \
                \n\n\
                Also, Tattoy has another method: screenshotting your palette, which you can do \
                right now by either letting Tattoy take the screenshot for you or provide one \
                you've taken yourself with the `--parse-palette` argument. \
                \n\n\
                And if all else fails you can just use the default bundled palette (Tokyo Night).\n\n"
            );
        }

        Self::print_native_palette()?;

        print!(
            "* Press 'y' to take a screenshot and attempt to parse your terminal palette's true \
            color values.\n\
            * Or press 'd' to use the default Tokyo Night palette.\n\n\
            Enter 'y', 'd' to continue or any other key to cancel: "
        );
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        println!();

        let yes = format!("y{}", crate::utils::NEWLINE);
        let default = format!("d{}", crate::utils::NEWLINE);

        if answer != yes && answer != default {
            println!("Nothing selected, exiting...");
            std::process::exit(0);
        }

        if answer == default {
            super::main::set_default_palette(state).await?;
            return Ok(None);
        }

        for window in xcap::Window::all()? {
            if window.is_focused() {
                return Ok(Some(window.capture_image()?));
            }
        }

        tracing::debug!("No windows found, trying to capture monitor instead");

        let monitors = xcap::Monitor::all()?;
        if monitors.is_empty() {
            color_eyre::eyre::bail!("No windows and no monitors found to take screenshot on");
        }

        // This assumes that the first monitor is the current monitor. Could be wrong.
        if let Some(monitor) = monitors.first() {
            return Ok(Some(monitor.capture_image()?));
        }

        color_eyre::eyre::bail!("No windows and monitors found to take screenshot on");
    }
}
