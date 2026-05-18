use std::collections::HashMap;
use std::fmt::Display;
use std::io;
use std::process::Command;
use users::get_effective_uid;

#[derive(Debug)]
pub enum Error {
    InsufficientPrivileges,
    FailedExecutingCommand(io::Error),
    CommandExited(String),
    GroupLookupFailed(io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InsufficientPrivileges => write!(f, "Insufficient privileges"),
            Error::FailedExecutingCommand(e) => write!(f, "Failed executing command: {}", e),
            Error::CommandExited(e) => write!(f, "Command exited: {}", e),
            Error::GroupLookupFailed(e) => write!(f, "Failed looking up user groups: {}", e),
        }
    }
}

/// Returns environment variables for `user` from that user's login shell.
///
/// The command runs through `su - ... -c` and executes `"$SHELL" -lc "env -0"`,
/// which is a login, non-interactive shell invocation.
pub fn get_user_env(user: String) -> Result<HashMap<String, String>, Error> {
    if get_effective_uid() != 0 {
        return Err(Error::InsufficientPrivileges);
    }

    // Execute the command and capture null-delimited environment output.
    let output = Command::new("su")
        .arg("-")
        .arg(user)
        .arg("-c")
        .arg(r#"exec "$SHELL" -lc "env -0""#)
        .output()
        .map_err(|e| Error::FailedExecutingCommand(e))?;

    // Check for command execution errors
    if !output.status.success() {
        return Err(Error::CommandExited(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    // Parse null-delimited KEY=VALUE entries from env -0.
    let mut env_map = HashMap::new();
    for entry in output.stdout.split(|b| *b == b'\0') {
        if entry.is_empty() {
            continue;
        }
        let entry_str = String::from_utf8_lossy(entry);
        let mut split = entry_str.splitn(2, '=');
        if let (Some(key), Some(value)) = (split.next(), split.next()) {
            env_map.insert(key.to_string(), value.to_string());
        }
    }

    Ok(env_map)
}
