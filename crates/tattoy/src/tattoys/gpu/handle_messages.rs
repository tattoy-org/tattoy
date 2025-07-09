//! Handle messages from Tattoy protocol. This is shared by both the shaders tattoy and the
//! animated cursor tattoy.

use color_eyre::Result;

use shadow_terminal::termwiz;

impl super::pipeline::GPU {
    /// Handle messages from the main Tattoy app.
    pub async fn handle_protocol_message(&mut self, message: &crate::run::Protocol) -> Result<()> {
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "It's internal so we'll know when there's new arms"
        )]
        match message {
            crate::run::Protocol::Output(_) | crate::run::Protocol::Config(_) => {
                self.protocol.send(crate::run::Protocol::Repaint)?;
            }
            crate::run::Protocol::Resize { width, height } => {
                self.update_resolution(*width, height * 2)?;
            }
            crate::run::Protocol::Input(input) => {
                if let termwiz::input::InputEvent::Mouse(mouse) = &input.event {
                    self.update_mouse_position(mouse.x, mouse.y);
                }
            }
            crate::run::Protocol::KeybindEvent(event) => {
                if matches!(event, crate::config::input::KeybindingAction::ShaderPrev) {
                    self.cycle_shader(false).await?;
                }
                if matches!(event, crate::config::input::KeybindingAction::ShaderNext) {
                    self.cycle_shader(true).await?;
                }
            }
            _ => (),
        }

        Ok(())
    }

    /// Cycle through the shaders in the user's shader directory.
    async fn cycle_shader(&mut self, direction: bool) -> Result<()> {
        let Some(shader_directory) = self.shader_path.parent() else {
            color_eyre::eyre::bail!("Unreachable: current shader doesn't have a parent path.");
        };
        let Some(current_filename) = self.shader_path.file_name() else {
            color_eyre::eyre::bail!("Unreachable: couldn't get current shader's filename.");
        };

        let mut all_shaders = std::fs::read_dir(shader_directory)?
            .map(|result| result.map_err(Into::into))
            .collect::<Result<Vec<std::fs::DirEntry>>>()?
            .into_iter()
            .filter_map(|entry| entry.path().is_file().then(|| entry.file_name()))
            .collect::<Vec<std::ffi::OsString>>();
        all_shaders.sort();

        if !direction {
            all_shaders.reverse();
        }

        let Some(new_shader_raw) = all_shaders.first() else {
            color_eyre::eyre::bail!(
                "Unreachable: current shader's directory doesn't have a shader in it."
            );
        };
        let mut new_shader = new_shader_raw.clone();
        let mut is_current_shader_found = false;
        for shader_filename in all_shaders {
            if is_current_shader_found {
                new_shader = shader_filename;
                break;
            }
            tracing::debug!("{:?}=={:?}", shader_filename, current_filename);
            if shader_filename == current_filename {
                is_current_shader_found = true;
            }
        }

        let shader_path = shader_directory.join(new_shader.clone());
        tracing::info!("Changing shader to: {new_shader:?}");

        self.shader_path = shader_path;
        self.build_pipeline().await?;
        self.protocol.send(crate::run::Protocol::Repaint)?;

        Ok(())
    }
}
