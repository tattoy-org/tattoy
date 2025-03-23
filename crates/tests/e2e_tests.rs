//! End to end tests

#[expect(
    clippy::large_futures,
    clippy::unreadable_literal,
    reason = "
        These are just tests, and the downsides should mainfest as a showstopping stack
        overflow, so we'll know about it soon enough.
    "
)]
#[cfg(not(target_os = "windows"))]
#[cfg(test)]
mod e2e {
    const ESCAPE: &str = "\x1b";

    use shadow_terminal::{
        shadow_terminal::Config,
        steppable_terminal::{Input, SteppableTerminal},
    };

    fn tattoy_binary_path() -> String {
        shadow_terminal::tests::helpers::workspace_dir()
            .join("target")
            .join("debug")
            .join("tattoy")
            .display()
            .to_string()
    }

    async fn start_tattoy(maybe_config_path: Option<String>) -> SteppableTerminal {
        let shell = shadow_terminal::steppable_terminal::get_canonical_shell();

        let prompt = "tattoy $ ";

        // TODO: this directory gets deleted at the end of the _function_, not the end of the test.
        let temp_dir = tempfile::tempdir().unwrap();

        let config = Config {
            width: 50,
            height: 10,
            command: shell.clone(),
            ..Config::default()
        };
        let mut stepper = SteppableTerminal::start(config).await.unwrap();

        let config_path = match maybe_config_path {
            None => {
                std::fs::copy(
                    "resources/palette.toml",
                    temp_dir.path().join("palette.toml"),
                )
                .unwrap();
                temp_dir.path().display().to_string()
            }
            Some(path) => path,
        };

        let command = generate_tattoy_command(&shell, prompt, config_path.as_ref());
        stepper.send_command(&command).unwrap();
        stepper.wait_for_string(prompt, None).await.unwrap();
        assert_random_walker_moves(&mut stepper).await;
        stepper
    }

    // We use the minimum possible ENV to support reproducibility of tests.
    fn generate_tattoy_command(
        shell_as_vec: &[std::ffi::OsString],
        prompt: &str,
        config_dir: &str,
    ) -> String {
        let pwd = std::env::current_dir().unwrap();
        #[expect(
            clippy::option_if_let_else,
            reason = "In this case `match` reads better that `map_or`"
        )]
        let rust_log_filters = match std::env::var_os("RUST_LOG") {
            Some(value) => format!("RUST_LOG={value:?}"),
            None => String::new(),
        };

        let bin_paths = std::env::var("PATH").unwrap();

        let seperator = std::ffi::OsString::from(" ".to_owned());
        let shell = shell_as_vec.join(&seperator);
        let minimum_env = format!(
            "\
            SHELL='{shell:?}' \
            PATH='{bin_paths}' \
            TATTOY_UNDER_TEST=1 \
            PWD='{pwd:?}' \
            PS1='{prompt}' \
            {rust_log_filters} \
            "
        );
        let command = format!(
            "\
            unset $(env | cut -d= -f1) && \
            {} {} \
            --use random_walker \
            --use minimap \
            --command 'bash --norc --noprofile' \
            --config-dir {} \
            --log-path ./tests.log \
            ",
            minimum_env,
            tattoy_binary_path(),
            config_dir
        );

        tracing::debug!("Full command: {}", command);
        command
    }

    async fn assert_random_walker_moves(tattoy: &mut SteppableTerminal) {
        let iterations = 1000;
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

    fn setup_logging() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }

    fn move_mouse(x: u32, y: u32) -> Input {
        Input::Event(format!("{ESCAPE}[<35;{x};{y}M"))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn basic_interactivity() {
        let mut tattoy = start_tattoy(None).await;

        assert_random_walker_moves(&mut tattoy).await;

        tattoy.send_command("echo $((1+1))").unwrap();
        tattoy.wait_for_string("2", None).await.unwrap();

        assert_random_walker_moves(&mut tattoy).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resizing() {
        let mut tattoy = start_tattoy(None).await;
        tattoy.send_command("nano --restricted").unwrap();
        tattoy.wait_for_string("GNU nano", None).await.unwrap();

        assert_random_walker_moves(&mut tattoy).await;

        let size = tattoy.shadow_terminal.terminal.get_size();
        let bottom = size.rows - 1;
        let right = size.cols - 1;
        assert_eq!(bottom, 9);
        assert_eq!(right, 49);
        tattoy
            .wait_for_string_at("Paste", right - 10, bottom, None)
            .await
            .unwrap();

        tattoy
            .shadow_terminal
            .resize(
                u16::try_from(size.cols + 3).unwrap(),
                u16::try_from(size.rows + 3).unwrap(),
            )
            .unwrap();
        let resized_size = tattoy.shadow_terminal.terminal.get_size();
        let resized_bottom = resized_size.rows - 1;
        let resized_right = resized_size.cols - 1;
        assert_eq!(resized_bottom, 12);
        assert_eq!(resized_right, 52);
        tattoy
            .wait_for_string_at("^X Exit", 0, resized_bottom, Some(1000))
            .await
            .unwrap();
        let resized_menu_item_paste = tattoy
            .get_string_at(resized_right - 10, resized_bottom, 5)
            .unwrap();
        assert_eq!(resized_menu_item_paste, "Paste");

        assert_random_walker_moves(&mut tattoy).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scrolling() {
        async fn assert_scrolling_off(tattoy: &mut SteppableTerminal) {
            let size = tattoy.shadow_terminal.terminal.get_size();
            let bottom = size.rows - 1;
            let right = size.cols - 1;
            tattoy
                .wait_for_string_at("nulla pariatur?", 0, bottom - 1, None)
                .await
                .unwrap();

            // Check for absence of scrollbar
            tattoy
                .wait_for_bg_color_at(None, right, bottom - 3, None)
                .await
                .unwrap();
        }

        async fn assert_scrolled_up(tattoy: &mut SteppableTerminal) {
            let size = tattoy.shadow_terminal.terminal.get_size();
            let bottom = size.rows - 1;
            let right = size.cols - 1;
            tattoy
                .wait_for_string_at("riosam, nisi", 0, bottom, None)
                .await
                .unwrap();

            // Check for scrollbar
            tattoy
                .wait_for_bg_color_at(
                    Some((0.33333334, 0.33333334, 0.33333334, 1.0)),
                    right,
                    bottom - 2,
                    None,
                )
                .await
                .unwrap();
        }

        setup_logging();
        let mouse_up = "\x1b[<64;14;2M";
        let mouse_down = "\x1b[<65;15;5M";

        let mut tattoy = start_tattoy(None).await;

        tattoy
            .send_command("cat resources/LOREM_IPSUM.txt")
            .unwrap();
        assert_scrolling_off(&mut tattoy).await;

        tattoy
            .send_input(Input::Event(mouse_up.to_owned()))
            .unwrap();
        assert_scrolled_up(&mut tattoy).await;

        tattoy
            .send_input(Input::Event(mouse_down.to_owned()))
            .unwrap();
        tattoy
            .send_input(Input::Event(mouse_down.to_owned()))
            .unwrap();
        assert_scrolling_off(&mut tattoy).await;

        tattoy
            .send_input(Input::Event(mouse_up.to_owned()))
            .unwrap();
        assert_scrolled_up(&mut tattoy).await;

        tattoy.send_input(Input::Event(ESCAPE.to_owned())).unwrap();
        assert_scrolling_off(&mut tattoy).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn palette_to_true_colour() {
        let mut tattoy = start_tattoy(None).await;

        tattoy
            .send_command("echo -e \"\\033[0;31m$((1000-1))\\033[m\"")
            .unwrap();
        tattoy.wait_for_string("999", None).await.unwrap();

        let cell = tattoy.get_cell_at(0, 1).unwrap().unwrap();

        assert_eq!(cell.str(), "9");
        assert_eq!(
            cell.attrs().foreground(),
            termwiz::color::ColorAttribute::TrueColorWithDefaultFallback(
                termwiz::color::SrgbaTuple(0.96862745, 0.4627451, 0.5568628, 1.0)
            ),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn minimap() {
        let mut tattoy = start_tattoy(None).await;
        let size = tattoy.shadow_terminal.terminal.get_size();
        setup_logging();

        tattoy
            .send_command("cat resources/LOREM_IPSUM.txt")
            .unwrap();
        tattoy.wait_for_string("nulla", None).await.unwrap();
        tattoy
            .send_input(move_mouse(u32::try_from(size.cols).unwrap() - 1, 1))
            .unwrap();

        tattoy.wait_for_string("co▀▀▀▀▀▀▀▀▀▀", None).await.unwrap();
    }
}
