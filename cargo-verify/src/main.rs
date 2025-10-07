use std::{
    env,
    path::Path,
    process::{Command, ExitCode},
};

fn main() -> ExitCode {
    let mut args = env::args_os();
    let driver_path = Path::new(&args.next().unwrap())
        .parent()
        .unwrap()
        .join("verify-driver");
    Command::new(env::var_os("CARGO").unwrap())
        .arg("build")
        .args(args.skip(1))
        .env("RUSTC_WORKSPACE_WRAPPER", driver_path)
        .status()
        .map_or_else(
            |err| {
                eprintln!("{err}");
                ExitCode::FAILURE
            },
            |status| {
                status
                    .code()
                    .map_or(ExitCode::FAILURE, |code| (code as u8).into())
            },
        )
}
