//! Animate the cursor using shaders.

use color_eyre::eyre::Result;

use crate::tattoys::tattoyer::Tattoyer;

/// The size of the cursor in units of terminal UTF8 half blocl "pixels".
pub const CURSOR_DIMENSIONS_REAL: (f32, f32) = (1.0, 2.0);

/// All the user config for the shader tattoy.
#[derive(serde::Deserialize, Debug, Clone)]
#[serde(default)]
pub(crate) struct Config {
    /// Enable/disable the shaders on and off
    pub enabled: bool,
    /// The path to a given GLSL shader file.
    pub path: std::path::PathBuf,
    /// The opacity of the rendered shader layer.
    pub opacity: f32,
    /// The layer (or z-index) into which the shaders are rendered.
    pub layer: i16,
    /// Whether to upload a pixel representation of the user's terminal. Useful for shader's that
    /// replace the text of the terminal, as Ghostty shaders do.
    pub upload_tty_as_pixels: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            path: format!(
                "{}/{}",
                crate::config::main::CURSOR_SHADER_DIRECTORY_NAME,
                crate::config::main::DEFAULT_CURSOR_SHADER_FILENAME
            )
            .into(),
            opacity: 0.75,
            layer: -1,
            upload_tty_as_pixels: false,
        }
    }
}

/// `AnimatedCursor`
pub(crate) struct AnimatedCursor {
    /// The base Tattoy struct
    tattoy: Tattoyer,
    /// All the special GPU handling code.
    gpu: super::gpu::pipeline::GPU,
}

impl crate::tattoys::gpu::shaderer::Shaderer for AnimatedCursor {
    fn tattoy(&self) -> &crate::tattoys::tattoyer::Tattoyer {
        &self.tattoy
    }

    fn tattoy_mut(&mut self) -> &mut crate::tattoys::tattoyer::Tattoyer {
        &mut self.tattoy
    }

    fn gpu(&mut self) -> &mut super::gpu::pipeline::GPU {
        &mut self.gpu
    }

    async fn is_upload_tty_as_pixels(&self) -> bool {
        self.tattoy
            .state
            .config
            .read()
            .await
            .animated_cursor
            .upload_tty_as_pixels
    }

    async fn get_layer(&self) -> i16 {
        self.tattoy()
            .state
            .config
            .read()
            .await
            .animated_cursor
            .layer
    }

    async fn get_opacity(&self) -> f32 {
        self.tattoy()
            .state
            .config
            .read()
            .await
            .animated_cursor
            .opacity
    }

    /// Instantiate
    async fn new(
        output_channel: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
        state: std::sync::Arc<crate::shared_state::SharedState>,
    ) -> Result<Self> {
        let config_directory = state.config_path.read().await.clone();
        let shader_path = state.config.read().await.animated_cursor.path.clone();
        let tty_size = *state.tty_size.read().await;
        let gpu = super::gpu::pipeline::GPU::new(
            config_directory.join(shader_path),
            tty_size.width,
            tty_size.height * 2,
            state.protocol_tx.clone(),
        )
        .await?;
        let layer = state.config.read().await.animated_cursor.layer;
        let opacity = state.config.read().await.animated_cursor.opacity;
        let tattoy = Tattoyer::new(
            "animated_cursor".to_owned(),
            state,
            layer,
            opacity,
            output_channel,
        )
        .await;
        Ok(Self { tattoy, gpu })
    }
}
