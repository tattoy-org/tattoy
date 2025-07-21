#![expect(clippy::large_futures, reason = "It's okay in tests")]

use shadow_terminal::steppable_terminal::SteppableTerminal;
use shadow_terminal::termwiz;

pub const ESCAPE: &str = "\x1b";

#[inline]
pub fn workspace_dir() -> std::path::PathBuf {
    let output = std::process::Command::new(env!("CARGO"))
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format=plain")
        .output()
        .unwrap()
        .stdout;
    let cargo_path = std::path::Path::new(std::str::from_utf8(&output).unwrap().trim());
    let workspace_dir = cargo_path.parent().unwrap().to_path_buf();
    tracing::debug!("Using workspace directory: {workspace_dir:?}");
    workspace_dir
}

pub fn tattoy_binary_path() -> String {
    workspace_dir()
        .join("target")
        .join("debug")
        .join("tattoy")
        .display()
        .to_string()
}

pub async fn start_tattoy(
    maybe_config_path: Option<String>,
) -> (SteppableTerminal, tempfile::TempDir) {
    let shell = shadow_terminal::tests::helpers::get_canonical_shell();

    let prompt = "tattoy $ ";

    let temp_dir = tempfile::tempdir().unwrap();

    let config = shadow_terminal::shadow_terminal::Config {
        width: 50,
        height: 10,
        command: shell.clone(),
        ..shadow_terminal::shadow_terminal::Config::default()
    };
    let mut stepper = SteppableTerminal::start(config).await.unwrap();

    let config_path = match maybe_config_path {
        None => temp_dir.path().display().to_string(),
        Some(path) => path,
    };

    std::fs::copy(
        "../tattoy/default_palette.toml",
        std::path::PathBuf::new()
            .join(config_path.clone())
            .join("palette.toml"),
    )
    .unwrap();

    let command = generate_tattoy_command(&shell, prompt, config_path.as_ref());
    stepper.send_command(&command).unwrap();
    stepper.wait_for_string(prompt, None).await.unwrap();
    stepper.wait_for_any_change().await.unwrap();
    (stepper, temp_dir)
}

// We use the minimum possible ENV to support reproducibility of tests.
pub fn generate_tattoy_command(
    shell_as_vec: &[std::ffi::OsString],
    prompt: &str,
    config_dir: &str,
) -> String {
    let pwd = std::env::current_dir().unwrap();
    #[expect(
        clippy::option_if_let_else,
        reason = "In this case `match` reads better that `map_or`"
    )]
    let rust_log_filters = match std::env::var_os("TATTOY_LOG") {
        Some(value) => format!("TATTOY_LOG={value:?}"),
        None => String::new(),
    };

    let bin_paths = std::env::var("PATH").unwrap();
    let xdg_runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();

    let seperator = std::ffi::OsString::from(" ".to_owned());
    let shell = shell_as_vec.join(&seperator);
    let minimum_env = format!(
        "\
            SHELL='{shell:?}' \
            PATH='{bin_paths}' \
            TATTOY_UNDER_TEST=1 \
            PWD='{pwd:?}' \
            PS1='{prompt}' \
            TERM=xterm-256color \
            XDG_RUNTIME_DIR='{xdg_runtime_dir}' \
            {rust_log_filters} \
            "
    );
    let command = format!(
        "\
            unset $(env | cut -d= -f1) && \
            {} {} \
            --use random_walker \
            --use minimap \
            --disable-indicator \
            --command 'bash --norc --noprofile' \
            --config-dir {} \
            --log-path ./tests.log \
            --log-level trace \
            ",
        minimum_env,
        tattoy_binary_path(),
        config_dir
    );

    tracing::debug!("Full command: {}", command);
    command
}

pub async fn assert_random_walker_moves(tattoy: &mut SteppableTerminal) {
    let iterations = 3000;
    tattoy.wait_for_string("▀", Some(iterations)).await.unwrap();
    let coords = tattoy.get_coords_of_cell_by_content("▀").unwrap();
    for i in 0..=iterations {
        tattoy.render_all_output().await.unwrap();
        assert!(
            i != iterations,
            "Random walker didn't move in a {iterations} iterations."
        );

        tattoy.wait_for_string("▀", Some(iterations)).await.unwrap();
        let next_coords = tattoy.get_coords_of_cell_by_content("▀").unwrap();
        if coords != next_coords {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }
}

pub fn setup_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

// TODO: move this into `shadow-terminal`.
pub fn move_mouse(x: u32, y: u32) -> shadow_terminal::steppable_terminal::Input {
    shadow_terminal::steppable_terminal::Input::Event(format!("{ESCAPE}[<35;{x};{y}M"))
}

pub fn get_colours(
    cell: &termwiz::cell::Cell,
) -> (
    palette::Alpha<palette::rgb::Rgb, f32>,
    palette::Alpha<palette::rgb::Rgb, f32>,
) {
    let fg_raw = SteppableTerminal::extract_colour(cell.attrs().foreground()).unwrap();
    let bg_raw = SteppableTerminal::extract_colour(cell.attrs().background()).unwrap();
    let fg = palette::Srgba::new(fg_raw.0, fg_raw.1, fg_raw.2, fg_raw.3);
    let bg = palette::Srgba::new(bg_raw.0, bg_raw.1, bg_raw.2, bg_raw.3);
    dbg!(bg, fg);
    (bg, fg)
}

pub async fn wait_for_stable_pixel(tattoy: &mut SteppableTerminal, x: usize, y: usize) {
    let mut previous_cell = termwiz::cell::Cell::blank();

    let stabilisation_target = 100;
    let mut stabilisation_start = tokio::time::Instant::now();
    let stabilisation_duration = tokio::time::Duration::from_millis(stabilisation_target);

    let timeout = 500;
    let timeout_start = tokio::time::Instant::now();
    let timeout_duration = tokio::time::Duration::from_millis(timeout);

    loop {
        tattoy.render_all_output().await.unwrap();
        let cell = tattoy.get_cell_at(x, y).unwrap().unwrap();
        if cell.attrs() != previous_cell.attrs() {
            stabilisation_start = tokio::time::Instant::now();
        }
        if timeout_start.elapsed() > timeout_duration {
            tattoy.dump_screen().unwrap();
            panic!("Cell colour didn't stabilise in {timeout}ms");
        }

        if stabilisation_start.elapsed() > stabilisation_duration {
            return;
        }
        previous_cell = cell;
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
    }
}

// TODO: how to get this from the `tattoy` crate without having to make a `lib.rs`.
pub fn create_shader_files(path: &std::path::Path) {
    let shader_directory = path.join("shaders");
    let animated_cursor_directory = shader_directory.join("cursors");
    std::fs::create_dir_all(animated_cursor_directory.clone()).unwrap();
    std::fs::copy(
        "../tattoy/src/tattoys/gpu/shaders/soft_shadows.glsl",
        shader_directory.join("soft_shadows.glsl"),
    )
    .unwrap();
    std::fs::copy(
        "../tattoy/src/tattoys/gpu/shaders/smear_fade.glsl",
        animated_cursor_directory.join("smear_fade.glsl"),
    )
    .unwrap();

    std::fs::copy("../tattoy/default_palette.toml", path.join("palette.toml")).unwrap();
}
