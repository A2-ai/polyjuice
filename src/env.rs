use std::collections::HashMap;
use std::io;
use std::process::Command;
use thiserror::Error;
use uzers::get_effective_uid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    #[error("Failed executing command: {0}")]
    FailedExecutingCommand(io::Error),
    #[error("Command exited: {0}")]
    CommandExited(String),
}

pub fn get_user_env(user: String) -> Result<HashMap<String, String>, Error> {
    if get_effective_uid() != 0 {
        return Err(Error::InsufficientPrivileges);
    }

    // Execute the command and capture the output
    let output = Command::new("su")
        .arg("-")
        .arg(user)
        .arg("-c")
        .arg("printenv")
        .output()
        .map_err(|e| Error::FailedExecutingCommand(e))?;

    // Check for command execution errors
    if !output.status.success() {
        return Err(Error::CommandExited(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    // Convert the output bytes to a String
    let output_str = String::from_utf8_lossy(&output.stdout);

    // Parse each line of the output
    let mut env_map = HashMap::new();
    for line in output_str.lines() {
        let mut split = line.splitn(2, '=');
        if let (Some(key), Some(value)) = (split.next(), split.next()) {
            env_map.insert(key.to_string(), value.to_string());
        }
    }

    Ok(env_map)
}
