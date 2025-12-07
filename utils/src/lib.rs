use std::{
    env, error,
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    CargoLocateProjectFailed(ExitStatus),
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::CargoLocateProjectFailed(_) => write!(f, "`cargo locate-project` failed"),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

pub fn cargo_manifest_path() -> Result<PathBuf, Error> {
    let output = Command::new(env::var_os("CARGO").unwrap_or("cargo".into()))
        .args(["locate-project", "--message-format", "plain"])
        .stderr(Stdio::inherit())
        .output()?;
    if output.status.success() {
        // When success, guaranteed to be UTF-8
        let project_location = String::from_utf8(output.stdout).unwrap();
        Ok(project_location.into())
    } else {
        Err(Error::CargoLocateProjectFailed(output.status))
    }
}

pub fn cargo_manifest_dir() -> Result<PathBuf, Error> {
    cargo_manifest_path().map(|mut path| {
        path.pop();
        path
    })
}

#[cfg(test)]
mod tests {}
