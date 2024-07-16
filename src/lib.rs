use std::{ffi::OsStr, fmt::Display, os::unix::process::CommandExt, process::Command};

use env::get_user_env;

#[cfg(feature = "pam")]
use pam_client::{Context, Flag};
use users::User;

mod env;

#[derive(Debug)]
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
pub fn cmd_as_username(
    program: impl AsRef<OsStr>,
    username: impl AsRef<OsStr>,
) -> Result<Command, CmdError> {
    let user = users::get_user_by_name(&username).ok_or(CmdError::UserNotFound)?;
    cmd_as_user(&program, user).map_err(|e| CmdError::FailedGettingEnv(e))
}

/// Creates a new command instance configured to run as a specific user.
///
/// This function takes a program name and a `User` object, and returns a `Command`
/// instance that, when executed, will run the specified program with the privileges
/// and environment of the given user.
///
/// # Parameters
///
/// * `program`: A string slice representing the name or path of the program to be executed.
/// * `user`: A `User` object representing the user as whom the command should be run.
///
/// # Returns
///
/// Returns a `Result` containing:
/// - `Ok(Command)`: A configured `Command` instance if successful.
/// - `Err(env::Error)`: An error if retrieving the user's environment variables fails.
///
/// # Details
///
/// The function performs the following steps:
/// 1. Retrieves the user's environment variables.
/// 2. Creates a new `Command` instance for the specified program.
/// 3. Sets the UID and GID of the command to match the specified user.
/// 4. Clears any existing environment variables and sets them to the user's environment.
///
/// # Errors
///
/// This function will return an `Err` if:
/// - The call to `get_user_env` fails, which could happen if the user's environment
///   cannot be retrieved or parsed correctly.
///
/// # Examples
///
/// ```no_run
/// use users::User;
/// use your_crate_name::cmd_as_user;
///
/// let user = User::from_uid(1000).expect("Failed to get user");
/// match cmd_as_user("ls", user) {
///     Ok(mut cmd) => {
///         // The command is now configured to run as the specified user
///         cmd.arg("-l");
///         let output = cmd.output().expect("Failed to execute command");
///         println!("Command output: {:?}", output);
///     },
///     Err(e) => eprintln!("Failed to create command: {}", e),
/// }
/// ```
///
/// # Security Considerations
///
/// This function allows running commands as different users, which can have significant
/// security implications. Ensure that:
/// - The calling process has the necessary privileges to switch users.
/// - The `program` parameter is properly sanitized to prevent command injection.
/// - The `User` object is obtained from a trusted source.
pub fn cmd_as_user(program: impl AsRef<OsStr>, user: User) -> Result<Command, env::Error> {
    let env = get_user_env(user.name().to_string_lossy().to_string())?;

    let mut new_cmd = Command::new(program);
    new_cmd.uid(user.uid()).gid(user.primary_group_id());
    new_cmd.env_clear().envs(env);

    Ok(new_cmd)
}

/// Attempts to create a PAM session for a specified user.
///
/// This function initializes a PAM context for the given username and tries to
/// open a session. It's intended for authentication and session management
/// using PAM (Pluggable Authentication Modules).
///
/// This is particularly useful to prompt PAM to activated session related triggers
/// such as pam_mkhomedir
///
/// # Parameters
///
/// * `username`: The username for which to create the PAM session. This should
///   be a valid username on the system.
///
/// # Returns
///
/// If successful, returns `Ok(())`. On failure, returns a `Box<dyn std::error::Error>`
/// with the error details.
///
/// # Errors
///
/// Returns an error if:
///
/// - The PAM context cannot be initialized (e.g., if the provided username is invalid).
/// - The account management step (`acct_mgmt`) fails.
/// - The session cannot be opened.
///
/// # Examples
///
/// ```no_run
/// use your_crate_name::try_user_pam_session;
///
/// let username = "example_user".to_string();
/// match try_pam_session(username) {
///     Ok(()) => println!("Session created successfully"),
///     Err(e) => println!("Failed to create session: {}", e),
/// }
/// ```
///
#[cfg(feature = "pam")]
pub fn try_pam_session(username: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut context = Context::new(
        "polyjuice",     // Service name
        Some(&username), // Preset username
        pam_client::conv_null::Conversation::new(),
    )?;
    context.acct_mgmt(Flag::NONE)?;
    let _session = context.open_session(Flag::SILENT)?;
    Ok(())
}
