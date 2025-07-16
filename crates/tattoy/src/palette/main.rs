//! Get the true colour values for the terminal's colour palette.

#![expect(clippy::print_stdout, reason = "We need to give user feedback")]

use color_eyre::Result;
/// A default palette for users that can't parse their own palette.
const DEFAULT_PALETTE: &str = include_str!("../../default_palette.toml");

/// A single palette colour.
pub type PaletteColour = (u8, u8, u8);

/// A hash of palette indexes to true colour values.
pub type PaletteHashMap = std::collections::HashMap<String, PaletteColour>;

/// The key for the foreground colour in the palette hash.
pub const FOREGROUND_COLOUR_KEY: &str = "foreground";

/// The key for the background colour in the palette hash.
pub const BACKGROUND_COLOUR_KEY: &str = "background";

/// Get the terminal's colour palette.
pub(crate) async fn get_palette(
    state: &std::sync::Arc<crate::shared_state::SharedState>,
) -> Result<()> {
    match super::osc::OSC::run(state).await {
        Ok(()) => return Ok(()),
        Err(error) => tracing::warn!("Failed getting palette with OSC query: {error:?}"),
    }

    super::parser::Parser::run(state, None).await?;

    Ok(())
}

/// Canonical path to the palette config file.
pub(crate) async fn palette_config_path(
    state: &std::sync::Arc<crate::shared_state::SharedState>,
) -> std::path::PathBuf {
    crate::config::main::Config::directory(state)
        .await
        .join("palette.toml")
}

/// Does a palette config file exist?
pub(crate) async fn palette_config_exists(
    state: &std::sync::Arc<crate::shared_state::SharedState>,
) -> bool {
    palette_config_path(state).await.exists()
}

/// Save the default palette config to the user's Tattoy config path.
pub(crate) async fn set_default_palette(
    state: &std::sync::Arc<crate::shared_state::SharedState>,
) -> Result<()> {
    let path = palette_config_path(state).await;
    std::fs::write(path.clone(), DEFAULT_PALETTE)?;

    println!("Default palette saved to: {}", path.display());
    Ok(())
}

/// Save the parsed palette true colours as TOML in the Tattoy config directory.
pub(crate) async fn save(
    state: &std::sync::Arc<crate::shared_state::SharedState>,
    palette: &crate::palette::converter::Palette,
) -> Result<()> {
    let path = palette_config_path(state).await;
    let data = toml::to_string(&palette.map)?;
    std::fs::write(path.clone(), data)?;

    println!("Palette saved to: {}", path.display());
    Ok(())
}
