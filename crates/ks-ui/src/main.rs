#[cfg(feature = "desktop")]
fn main() {
    tracing_subscriber::fmt::init();

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new().with_window(
                dioxus::desktop::WindowBuilder::new()
                    .with_title("KubeStudio")
                    .with_always_on_top(false),
            ),
        )
        .launch(ks_ui::App);
}

#[cfg(not(feature = "desktop"))]
fn main() {
    panic!(
        "This binary requires the 'desktop' feature. Use 'cargo run --features desktop' or run 'ks-connector' for fullstack mode."
    );
}
