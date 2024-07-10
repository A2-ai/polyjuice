use std::{fmt::Display, os::unix::process::CommandExt, process::Command};

use env::get_user_env;

mod env;

pub enum CmdError {
    UserNotFound,
    FailedGettingEnv(env::Error),
}

impl Display for CmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmdError::UserNotFound => write!(f, "User not found"),
            CmdError::FailedGettingEnv(e) => write!(f, "Failed to get user environment: {}", e),
        }
    }
}

/// This function creates a new command instance with the specified program and username.
/// It retrieves the user's information and environment variables using the `users` and `env` modules.
/// The new command is then configured with the user's UID, primary group ID, and environment variables.
///
/// # Parameters
///
/// * `program`: A string slice that represents the program to be executed.
/// * `username`: A string that represents the username of the user for whom the command is being executed.
///
/// # Returns
///
/// If successful, returns a `Result` containing the new `Command` instance.
/// On failure, returns a `Result` containing a `SetRunUserError` variant.
///
/// # Errors
///
/// Returns a `SetRunUserError::UserNotFound` error if the user is not found.
/// Returns a `SetRunUserError::FailedGettingEnv` error if there is an issue getting the user's environment variables.
///
/// # Examples
///
/// ```no_run
/// use your_crate_name::cmd_as_user;
///
/// let program = "ls";
/// let username = "example_user".to_string();
/// match cmd_as_user(program, username) {
///     Ok(cmd) => {
///         // Use the new command instance
///     }
///     Err(e) => {
///         // Handle the error
///     }
/// }
/// ```
pub fn cmd_as_user(program: &str, username: String) -> Result<Command, CmdError> {
    let user = users::get_user_by_name(&username).ok_or(CmdError::UserNotFound)?;
    let env = get_user_env(username.clone()).map_err(|e| CmdError::FailedGettingEnv(e))?;

    let mut new_cmd = Command::new(program);
    new_cmd.uid(user.uid()).gid(user.primary_group_id());
    new_cmd.env_clear().envs(env);

    Ok(new_cmd)
}
