mod app;
mod render;
mod tutorial;

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let render_root = session_render_root();
    fs::create_dir_all(&render_root).expect("failed to create the render workspace");
    app::run(render_root);
}

fn session_render_root() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis();

    std::env::temp_dir().join(format!(
        "lilypond-studio-{}-{timestamp}",
        std::process::id()
    ))
}
