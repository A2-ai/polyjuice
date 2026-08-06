use std::collections::HashMap;
use std::fmt::Display;
use std::io;
use std::process::Command;
use nix::unistd::geteuid;

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
pub fn get_user_env(user: &str) -> Result<HashMap<String, String>, Error> {
    if geteuid().as_raw() != 0 {
        return Err(Error::InsufficientPrivileges);
    }

    // Execute the command and capture null-delimited environment output.
    let output = Command::new("su")
        .arg("-")
        .arg(user)
        .arg("-c")
        .arg(r#"exec "$SHELL" -lc "env -0""#)
        .output()
        .map_err(Error::FailedExecutingCommand)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    // Without root, get_user_env refuses to run `su` at all. This runs in a
    // plain `cargo test`; it will fail if the suite is run as root.
    #[test]
    #[should_panic(expected = "InsufficientPrivileges")]
    fn su_invocation_refused_without_root() {
        get_user_env("root").unwrap();
    }

    // Same as above, but from inside a multi-threaded Tokio runtime — the
    // context this crate is actually used from.
    #[tokio::test(flavor = "multi_thread")]
    #[should_panic(expected = "InsufficientPrivileges")]
    async fn su_invocation_refused_without_root_tokio() {
        get_user_env("root").unwrap();
    }

    // These tests exercise the real `su` invocation, so they need root.
    // Run with: sudo cargo test -- --ignored
    #[test]
    #[ignore = "requires root: sudo cargo test -- --ignored"]
    fn su_invocation_returns_user_env() {
        let user = std::env::var("SUDO_USER")
            .expect("SUDO_USER not set; run via `sudo cargo test -- --ignored`");

        let env = get_user_env(&user).expect("get_user_env failed");

        assert_eq!(env.get("USER"), Some(&user));
        assert!(env.contains_key("HOME"));
        assert!(env.contains_key("SHELL"));
    }

    // The real fork-under-Tokio case: `su` is actually spawned while the
    // multi-threaded runtime's worker threads are alive.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires root: sudo cargo test -- --ignored"]
    async fn su_invocation_returns_user_env_tokio() {
        let user = std::env::var("SUDO_USER")
            .expect("SUDO_USER not set; run via `sudo cargo test -- --ignored`");

        let env = get_user_env(&user).expect("get_user_env failed");

        assert_eq!(env.get("USER"), Some(&user));
        assert!(env.contains_key("HOME"));
        assert!(env.contains_key("SHELL"));
    }

    #[test]
    #[ignore = "requires root: sudo cargo test -- --ignored"]
    fn su_invocation_fails_for_unknown_user() {
        let result = get_user_env("polyjuice_no_such_user");
        assert!(matches!(result, Err(Error::CommandExited(_))));
    }
}
