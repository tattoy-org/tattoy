//! Shadertoy-like shaders. You should be able to copy and paste most shaders found on
//! <https://shadertoy.com>.

use color_eyre::eyre::Result;

use crate::tattoys::tattoyer::Tattoyer;

/// All the user config for the shader tattoy.
#[expect(
    clippy::struct_excessive_bools,
    reason = "We need the bools for the config"
)]
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
    /// The shader is still sent and run on the GPU but it's not rendered to a layer on the
    /// terminal. This is most likely useful in conjunction with `render_shader_colours_to_text`,
    /// as "contents" of the shader are rendered via the terminal's text.
    pub render: bool,
    /// Whether to upload a pixel representation of the user's terminal. Useful for shader's that
    /// replace the text of the terminal, as Ghostty shaders do.
    pub upload_tty_as_pixels: bool,
    /// Define the terminal's text colours based on the colour of the shader pixel at the same
    /// position. This would most likely be used in conjunction with auto contrast enabled,
    /// otherwise the text won't actually be readable.
    pub render_shader_colours_to_text: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            path: format!(
                "{}/{}",
                crate::config::main::SHADER_DIRECTORY_NAME,
                crate::config::main::DEFAULT_SHADER_FILENAME
            )
            .into(),
            opacity: 0.75,
            layer: -10,
            render: true,
            upload_tty_as_pixels: true,
            render_shader_colours_to_text: false,
        }
    }
}

/// `Shaders`
pub(crate) struct Shaders {
    /// The base Tattoy struct
    tattoy: Tattoyer,
    /// All the special GPU handling code.
    gpu: super::gpu::pipeline::GPU,
}

impl crate::tattoys::gpu::shaderer::Shaderer for Shaders {
    fn tattoy(&self) -> &crate::tattoys::tattoyer::Tattoyer {
        &self.tattoy
    }

    fn tattoy_mut(&mut self) -> &mut crate::tattoys::tattoyer::Tattoyer {
        &mut self.tattoy
    }

    fn gpu(&self) -> &super::gpu::pipeline::GPU {
        &self.gpu
    }

    fn gpu_mut(&mut self) -> &mut super::gpu::pipeline::GPU {
        &mut self.gpu
    }

    async fn is_upload_tty_as_pixels(&self) -> bool {
        self.tattoy
            .state
            .config
            .read()
            .await
            .shader
            .upload_tty_as_pixels
    }

    fn is_upload_tty_with_characters(&self) -> bool {
        true
    }

    async fn get_layer(&self) -> i16 {
        self.tattoy().state.config.read().await.shader.layer
    }

    async fn get_opacity(&self) -> f32 {
        self.tattoy().state.config.read().await.shader.opacity
    }

    /// Instantiate
    async fn new(
        output_channel: tokio::sync::mpsc::Sender<crate::run::FrameUpdate>,
        state: std::sync::Arc<crate::shared_state::SharedState>,
    ) -> Result<Self> {
        let config_directory = state.config_path.read().await.clone();
        let shader_path = state.config.read().await.shader.path.clone();
        let tty_size = *state.tty_size.read().await;
        let gpu = super::gpu::pipeline::GPU::new(
            config_directory.join(shader_path),
            tty_size.width,
            tty_size.height * 2,
            state.protocol_tx.clone(),
        )
        .await?;
        let layer = state.config.read().await.shader.layer;
        let opacity = state.config.read().await.shader.opacity;
        let tattoy =
            Tattoyer::new("shader".to_owned(), state, layer, opacity, output_channel).await;
        Ok(Self { tattoy, gpu })
    }
}
