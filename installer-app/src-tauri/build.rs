use std::path::Path;

fn main() {
    let payload = Path::new("payload.zip");
    if !payload.exists() {
        println!("cargo:warning=payload.zip is a placeholder — run scripts/build-payload step before shipping this installer");
        std::fs::write(payload, []).expect("failed to write placeholder payload.zip");
    }

    tauri_build::build()
}
