//! Use OSC codes to query the terminal emulator about what RGB values it uses for each palette
//! index.

use color_eyre::eyre::{ContextCompat as _, Result};
use shadow_terminal::termwiz;
use termwiz::terminal::Terminal as _;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// The amount of time in seconds to wait for a response from the host terminal emulator.
const WAIT_FOR_RESPONSE_TIMEOUT: u64 = 1;

/// `OSC`
pub(crate) struct OSC;

impl OSC {
    /// Main entry point.
    pub async fn run(state: &std::sync::Arc<crate::shared_state::SharedState>) -> Result<()> {
        let mut termwiz_terminal = crate::renderer::Renderer::get_termwiz_terminal()?;

        termwiz_terminal.set_raw_mode()?;
        let result = Self::query_terminal().await;
        termwiz_terminal.set_cooked_mode()?;

        match result {
            Ok(hashmap) => {
                let palette = super::converter::Palette { map: hashmap };
                super::main::save(state, &palette).await?;
            }
            Err(error) => color_eyre::eyre::bail!(error),
        }

        Ok(())
    }

    /// Send OSC codes to the user's terminal emulator to query the terminal's palette.
    async fn query_terminal() -> Result<super::main::PaletteHashMap> {
        let mut tty = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .await?;

        let mut command = String::new();
        for index in 0..255u8 {
            command.extend(
                format!("{}]4;{index};?{}", crate::utils::ESCAPE, crate::utils::BELL).chars(),
            );
        }
        tty.write_all(command.as_bytes()).await?;
        tty.flush().await?;

        let palette = Self::read_response(tty).await?;
        tracing::debug!("OSC response to palette RGB query: {palette:?}");
        Ok(palette)
    }

    /// Read the response from the controlling terminal after sending an OSC code.
    async fn read_response(tty: tokio::fs::File) -> Result<super::main::PaletteHashMap> {
        let buffer_size = 1024;
        let mut reader = tokio::io::BufReader::new(tty);
        let mut all = Vec::new();
        let mut attempts = 0u16;

        loop {
            let mut buffer = vec![0; buffer_size];
            let result = tokio::time::timeout(
                tokio::time::Duration::from_secs(WAIT_FOR_RESPONSE_TIMEOUT),
                reader.read(&mut buffer),
            )
            .await;
            attempts += 1;
            if result.is_err() || attempts > 300 {
                let message = "Timed out waiting for response from controlling terminal \
                    when querying for palette colour values";
                tracing::warn!(message);
                color_eyre::eyre::bail!(message);
            }

            buffer.retain(|&x| x != 0);
            all.extend(buffer.clone());

            let response = &String::from_utf8_lossy(&all)
                .replace(crate::utils::ESCAPE, "ESC")
                .replace(crate::utils::STRING_TERMINATOR, "ST")
                .replace(crate::utils::BELL, "BELL");

            match Self::parse_colours(response) {
                Ok(colours) => {
                    if colours.len() == 255 {
                        return Ok(colours);
                    }
                }
                Err(error) => tracing::debug!("Potential error parsing OSC codes: {error:?}"),
            }
        }
    }

    /// Parse the OSC response for palette colours.
    fn parse_colours(response: &str) -> Result<super::main::PaletteHashMap> {
        let mut palette = super::main::PaletteHashMap::new();
        for sequence in response.split("ESC]4;") {
            if sequence.is_empty() {
                continue;
            }
            tracing::trace!("Parsing OSC sequence: {sequence}");

            let mut index_and_colour = sequence.split(';');
            let index = index_and_colour
                .next()
                .context(format!("OSC sequence not delimited by colon: {sequence}"))?
                .to_owned();
            let colourish = index_and_colour
                .next()
                .context(format!("Colour not found in OSC sequence: {sequence}"))?;

            let mut channels = colourish.split('/');
            let red = Self::get_last_chars(
                channels
                    .next()
                    .context(format!("Couldn't get red from OSC response: {colourish:?}"))?,
                2,
            );
            let green = Self::get_last_chars(
                channels.next().context(format!(
                    "Couldn't get green from OSC response: {colourish:?}"
                ))?,
                2,
            );
            let blue = Self::get_first_chars(
                channels.next().context(format!(
                    "Couldn't get blue from OSC response: {colourish:?}"
                ))?,
                2,
            );

            let colour: super::main::PaletteColour = (
                u8::from_str_radix(&red, 16)?,
                u8::from_str_radix(&green, 16)?,
                u8::from_str_radix(&blue, 16)?,
            );
            palette.insert(index, colour);
        }

        Ok(palette)
    }

    /// Get the first x characters from a string.
    fn get_first_chars(string: &str, count: usize) -> String {
        string.chars().take(count).collect::<String>()
    }

    /// Get the last x characters from a string.
    fn get_last_chars(string: &str, count: usize) -> String {
        string
            .chars()
            .rev()
            .take(count)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>()
    }
}

#[cfg(test)]
#[expect(clippy::indexing_slicing, reason = "It's just a test")]
mod test {
    use super::*;

    #[test]
    fn parsing_osc_colours() {
        let response = "ESC]4;1;rgb:c0c0/2222/eaeaBELLESC]4;229;rgb:aaaa/ffff/afafBELL";
        let palette = OSC::parse_colours(response).unwrap();
        assert_eq!(palette["1"], (192, 34, 234));
        assert_eq!(palette["229"], (170, 255, 175));
    }
}
