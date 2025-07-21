//! The manager of all the fancy Tattoy eye-candy code

use std::sync::Arc;

use color_eyre::eyre::Result;

use crate::{run::FrameUpdate, tattoys::gpu::shaderer::Shaderer as _};

/// Start all the enabled tattoys.
pub(crate) async fn start_tattoys(
    enabled_tattoys: Vec<String>,
    output: tokio::sync::mpsc::Sender<FrameUpdate>,
    state: Arc<crate::shared_state::SharedState>,
) -> std::thread::JoinHandle<Result<(), color_eyre::eyre::Error>> {
    convert_cli_enabled_args(&enabled_tattoys, &state).await;
    let handle = spawn(enabled_tattoys.clone(), output, Arc::clone(&state));
    wait_for_enabled_tattoys_to_start(enabled_tattoys, &state).await;
    handle
}

/// Centralise all "enabled" settings into the state config. Saves us having to check both the CLI
/// and state everytime.
async fn convert_cli_enabled_args(
    enabled_tattoys: &Vec<String>,
    state: &Arc<crate::shared_state::SharedState>,
) {
    for tattoy in enabled_tattoys {
        match tattoy.as_ref() {
            "startup_logo" => state.config.write().await.show_startup_logo = true,
            "notifications" => state.config.write().await.notifications.enabled = true,
            "minimap" => state.config.write().await.minimap.enabled = true,
            "shaders" => state.config.write().await.shader.enabled = true,
            "animated_cursor" => state.config.write().await.animated_cursor.enabled = true,
            "bg_command" => state.config.write().await.bg_command.enabled = true,
            _ => (),
        }
    }
}

/// Start the main loader thread
#[expect(clippy::too_many_lines, reason = "It's mostly repetitive")]
pub(crate) fn spawn(
    enabled_tattoys: Vec<String>,
    output: tokio::sync::mpsc::Sender<FrameUpdate>,
    state: Arc<crate::shared_state::SharedState>,
) -> std::thread::JoinHandle<Result<(), color_eyre::eyre::Error>> {
    let tokio_runtime = tokio::runtime::Handle::current();
    std::thread::spawn(move || -> Result<()> {
        tokio_runtime.block_on(async {
            crate::run::wait_for_system(&state, "renderer").await;

            let palette = crate::config::main::Config::load_palette(Arc::clone(&state)).await?;
            let mut tattoy_futures = tokio::task::JoinSet::new();

            if state.config.read().await.show_startup_logo {
                tracing::info!("Starting 'startup_logo' tattoy...");
                tattoy_futures.spawn(crate::tattoys::startup_logo::StartupLogo::start(
                    output.clone(),
                    Arc::clone(&state),
                    palette.clone(),
                ));
            }

            if state.config.read().await.notifications.enabled {
                tracing::info!("Starting 'notifications' tattoy...");
                tattoy_futures.spawn(crate::tattoys::notifications::main::Notifications::start(
                    output.clone(),
                    Arc::clone(&state),
                    palette.clone(),
                ));
                crate::run::wait_for_system(&state, "notifications").await;
            }

            tracing::info!("Starting 'scrollbar' tattoy...");
            tattoy_futures.spawn(crate::tattoys::scrollbar::Scrollbar::start(
                output.clone(),
                Arc::clone(&state),
            ));

            if enabled_tattoys.contains(&"random_walker".to_owned()) {
                tracing::info!("Starting 'random_walker' tattoy...");
                tattoy_futures.spawn(crate::tattoys::random_walker::RandomWalker::start(
                    output.clone(),
                    Arc::clone(&state),
                ));
            }

            if state.config.read().await.minimap.enabled {
                tracing::info!("Starting 'minimap' tattoy...");
                tattoy_futures.spawn(crate::tattoys::minimap::Minimap::start(
                    output.clone(),
                    Arc::clone(&state),
                ));
            }

            if state.config.read().await.shader.enabled {
                tracing::info!("Starting 'shaders' tattoy...");
                tattoy_futures.spawn(crate::tattoys::shader::Shaders::start(
                    output.clone(),
                    Arc::clone(&state),
                ));
            }

            if state.config.read().await.animated_cursor.enabled {
                tracing::info!("Starting 'animated_cursor' tattoy...");
                tattoy_futures.spawn(crate::tattoys::animated_cursor::AnimatedCursor::start(
                    output.clone(),
                    Arc::clone(&state),
                ));
            }

            if state.config.read().await.bg_command.enabled {
                tracing::info!("Starting 'bg_command' tattoy...");
                tattoy_futures.spawn(crate::tattoys::bg_command::BGCommand::start(
                    output.clone(),
                    Arc::clone(&state),
                    palette.clone(),
                ));
            }

            for plugin_config in &state.config.read().await.plugins {
                if let Some(is_enabled) = plugin_config.enabled {
                    if !is_enabled {
                        continue;
                    }
                }

                tattoy_futures.spawn(crate::tattoys::plugins::Plugin::start(
                    plugin_config.clone(),
                    palette.clone(),
                    Arc::clone(&state),
                    output.clone(),
                ));
            }

            while let Some(completes) = tattoy_futures.join_next().await {
                match completes {
                    Ok(result) => match result {
                        Ok(()) => tracing::debug!("A tattoy succesfully exited"),
                        Err(error) => {
                            let title = "Unhandled tattoy error";
                            let message = format!("{title}: {error:?}");
                            tracing::warn!(message);
                            state
                                .send_notification(
                                    title,
                                    crate::tattoys::notifications::message::Level::Error,
                                    Some(error.root_cause().to_string()),
                                    true,
                                )
                                .await;
                        }
                    },
                    Err(error) => tracing::error!("Tattoy task join error: {error:?}"),
                }
            }

            Ok(())
        })
    })
}

/// Wait for tattoys that need to be running before the PTY starts.
async fn wait_for_enabled_tattoys_to_start(
    enabled_tattoys: Vec<String>,
    state: &Arc<crate::shared_state::SharedState>,
) {
    if enabled_tattoys.contains(&"random_walker".to_owned()) {
        crate::run::wait_for_system(state, "random_walker").await;
    }

    if state.config.read().await.shader.enabled {
        crate::run::wait_for_system(state, "shader").await;
    }

    if state.config.read().await.minimap.enabled {
        crate::run::wait_for_system(state, "minimap").await;
    }

    if state.config.read().await.animated_cursor.enabled {
        crate::run::wait_for_system(state, "animated_cursor").await;
    }
}
