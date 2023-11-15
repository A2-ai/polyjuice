use pam_client::{Context, Flag};
use users::get_effective_uid; 
use std::process::Command;
use std::collections::HashMap;


/// Retrieves the environment variables for a specified user as a map.
///
/// Starts a login shell as the user and returns the values
/// in a `HashMap` where each key-value pair corresponds to
/// an environment variable and its value.
///
/// # Parameters
///
/// * `user`: A `String` specifying the username whose environment variables are to be retrieved.
///
/// # Returns
///
/// Returns a `Result<HashMap<String, String>, String>`. On success, the `Ok` variant contains
/// the `HashMap` with environment variables. On failure, the `Err` variant contains an error message.
///
/// # Errors
///
/// Returns an error if:
/// 
/// - There is an issue executing the command (e.g., command not found).
/// - The command execution is not successful (including if the user does not exist).
/// - There is an issue converting command output to UTF-8.
///
/// # Examples
///
/// ```no_run
/// use your_crate_name::get_user_env_as_map;
///
/// let user = "example_user".to_string();
/// match get_user_env_vars(user) {
///     Ok(env_map) => println!("Environment Variables: {:?}", env_map),
///     Err(e) => println!("Error: {}", e),
/// }
/// ```
///
pub fn get_user_env_vars(user: String) -> Result<HashMap<String, String>, String> {
 // Check if EUID is 0 (root)
 if get_effective_uid() == 0 {
    // Execute the command and capture the output
    let output = Command::new("su")
        .arg("-")
        .arg(user)
        .arg("-c")
        .arg("printenv")
        .output()
        .map_err(|e| e.to_string())?;

    // Check for command execution errors
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into());
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
} else {
    Err("Insufficient privileges: function requires root access.".to_string())
}
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