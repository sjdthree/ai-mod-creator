fn main() {
    // The model directory name comes from the repo's `model.lock` (the model this app ships with).
    let lock = std::fs::read_to_string("../../../model.lock").expect("model.lock at the repo root");
    let name = lock
        .lines()
        .find_map(|l| l.trim().strip_prefix("name="))
        .expect("model.lock has a name= line")
        .trim();
    println!("cargo:rustc-env=MCE_MODEL_NAME={name}");
    println!("cargo:rerun-if-changed=../../../model.lock");
    tauri_build::build();
}
