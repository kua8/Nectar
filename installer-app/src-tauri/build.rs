use std::path::Path;

fn main() {
    // `payload.rs` embeds this via `include_bytes!`, which needs the file to exist at
    // compile time even before a real app build has been packaged into it — write an
    // empty placeholder so a first `cargo check`/`cargo build` doesn't fail outright.
    let payload = Path::new("payload.zip");
    if !payload.exists() {
        println!("cargo:warning=payload.zip is a placeholder — run scripts/build-payload step before shipping this installer");
        std::fs::write(payload, []).expect("failed to write placeholder payload.zip");
    }

    tauri_build::build()
}
