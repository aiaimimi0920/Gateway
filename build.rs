fn main() {
    for path in [
        "apps/desktop/package.json",
        "apps/desktop/package-lock.json",
        "apps/desktop/rsbuild.config.ts",
        "apps/desktop/tsconfig.json",
        "apps/desktop/src",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    let npm_program = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let status = std::process::Command::new(npm_program)
        .args(["run", "build:web", "--prefix", "apps/desktop"])
        .status()
        .expect("failed to launch npm for Gateway web console build");
    if !status.success() {
        panic!("Gateway web console build failed with status: {status}");
    }
}
