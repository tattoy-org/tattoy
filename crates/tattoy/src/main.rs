//! Just `main()`. Keep as small as possible.

// TODO: Consider using `mod.rs`. As pointed out by @Justus_Fluegel, the disadvantage of
// this approach is that when moving files/modules, you _also_ have to move these module
// definitions.

pub mod cli_args;
/// All the user-configurable settings.
pub mod config {
    pub mod input;
    pub mod main;
}
pub mod blender;
pub mod compositor;
pub mod loader;
pub mod raw_input;
/// The palette code is for helping convert a terminal's palette to true colour.
pub mod palette {
    pub mod converter;
    pub mod main;
    pub mod osc;
    pub mod parser;
    pub mod state_machine;
}
pub mod renderer;
pub mod run;
pub mod shared_state;
pub mod surface;
/// A layer between Tattoy and the Shadow Terminal
pub mod terminal_proxy {
    pub mod input_handler;
    pub mod proxy;
}
pub mod utils;

/// This is where all the various tattoys are kept
pub mod tattoys {
    pub mod animated_cursor;
    pub mod bg_command;
    pub mod minimap;
    pub mod startup_logo;

    /// Notifications in the terminal UI
    pub mod notifications {
        pub mod main;
        pub mod message;
    }

    pub mod plugins;
    pub mod random_walker;
    pub mod scrollbar;
    pub mod shader;

    /// GPU management code
    pub mod gpu {
        pub mod handle_messages;
        pub mod ichannel;
        pub mod pipeline;
        pub mod shaderer;
    }

    pub mod tattoyer;
}

use color_eyre::eyre::Result;

#[expect(clippy::non_ascii_literal, reason = "It's just for debugging")]
#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "It's our central place for communicating with the user on CLI"
)]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    run::check_for_tattoy_in_tattoy();
    let (protocol_tx, _) = tokio::sync::broadcast::channel(1024);
    let state_arc = shared_state::SharedState::init_with_users_tty_size(protocol_tx).await?;
    let result = run::run(&std::sync::Arc::clone(&state_arc)).await;
    println!("{}", utils::RESET_SCREEN);

    let logpath = state_arc.config.read().await.log_path.clone();
    let is_logging = *state_arc.is_logging.read().await;
    tracing::debug!("Tattoy is exiting 🙇");

    match result {
        Ok(()) => {
            if is_logging {
                println!("Logs saved to {}", logpath.display());
            }
        }
        Err(error) => {
            tracing::error!("{error:?}");
            eprintln!("Error: {error}");
            if is_logging {
                eprintln!("See {} for more details", logpath.display());
            }
        }
    }

    Ok(())
}
