use std::io::Write as _;

use palette::color_difference::EuclideanDistance as _;

#[tokio::test(flavor = "multi_thread")]
async fn shaders() {
    let temp_dir = tempfile::tempdir().unwrap();
    let conf_dir = temp_dir.into_path();
    let conf_path = conf_dir.join("tattoy.toml");
    crate::utils::create_shader_files(&conf_dir);
    let mut conf_file = std::fs::File::create(conf_path).unwrap();
    let config = "
        [shader]
        enabled = true
    ";
    conf_file.write_all(config.as_bytes()).unwrap();

    let x = 10;
    let y = 0;

    let (mut tattoy, _) = crate::utils::start_tattoy(Some(conf_dir.to_string_lossy().into())).await;
    tattoy
        .wait_for_string_at("▀", x, y, Some(1000))
        .await
        .unwrap();

    crate::utils::wait_for_stable_pixel(&mut tattoy, x, y).await;

    let cell = tattoy.get_cell_at(x, y).unwrap().unwrap();
    let (bg_actual, fg_actual) = crate::utils::get_colours(&cell);
    tattoy.dump_screen().unwrap();

    let expected = palette::Srgb::new(0.12, 0.11, 0.14);
    assert!(bg_actual.distance(expected) < 0.05);
    assert!(fg_actual.distance(expected) < 0.05);
}

#[tokio::test(flavor = "multi_thread")]
async fn animated_cursor() {
    let temp_dir = tempfile::tempdir().unwrap();
    let conf_dir = temp_dir.into_path();
    let conf_path = conf_dir.join("tattoy.toml");
    crate::utils::create_shader_files(&conf_dir);
    let mut conf_file = std::fs::File::create(conf_path).unwrap();
    let config = "
        [animated_cursor]
        enabled = true
    ";
    conf_file.write_all(config.as_bytes()).unwrap();

    let (mut tattoy, _) = crate::utils::start_tattoy(Some(conf_dir.to_string_lossy().into())).await;
    tattoy.wait_for_string("tattoy", Some(1000)).await.unwrap();
    tattoy.wait_for_string("▀▀", Some(1000)).await.unwrap();
    tattoy.dump_screen().unwrap();

    let x = 9;
    let y = 1;

    let cell = tattoy.get_cell_at(x, y).unwrap().unwrap();
    let (bg_actual, fg_actual) = crate::utils::get_colours(&cell);

    let bg_expected = palette::Srgb::new(0.68, 0.69, 0.76);
    assert!(bg_actual.distance(bg_expected) < 0.1);
    let fg_expected = palette::Srgb::new(0.78, 0.8, 0.87);
    assert!(fg_actual.distance(fg_expected) < 0.1);
}
