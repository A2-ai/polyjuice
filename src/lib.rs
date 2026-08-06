use std::{
    ffi::{CStr, CString, OsStr},
    fmt::Display,
    os::unix::process::CommandExt,
    process::Command,
};

use env::get_user_env;

#[cfg(feature = "pam")]
use pam_client::{Context, Flag};

use nix::unistd::User;

mod env;

#[derive(Debug)]
pub enum CmdError {
    UserNotFound,
    InvalidUsername,
    UserLookupFailed(nix::Error),
    FailedGettingEnv(env::Error),
}

impl Display for CmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CmdError::UserNotFound => write!(f, "User not found"),
            CmdError::InvalidUsername => write!(f, "Invalid username"),
            CmdError::UserLookupFailed(e) => write!(f, "Failed to look up user: {}", e),
            CmdError::FailedGettingEnv(e) => write!(f, "Failed to get user environment: {}", e),
        }
    }
}

/// This function creates a new command instance with the specified program and username.
/// It retrieves the user's information and environment variables using the `nix` crate and `env` module.
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
/// use polyjuice::cmd_as_username;
///
/// let program = "ls";
/// let username = "example_user";
/// match cmd_as_username(program, username) {
///     Ok(cmd) => {
///         // Use the new command instance
///     }
///     Err(e) => {
///         // Handle the error
///     }
/// }
/// ```
pub fn cmd_as_username(program: impl AsRef<OsStr>, username: impl AsRef<OsStr>) -> Result<Command, CmdError> {
    let username = username
        .as_ref()
        .to_str()
        .ok_or(CmdError::InvalidUsername)?;

    let user = User::from_name(username)
        .map_err(CmdError::UserLookupFailed)?
        .ok_or(CmdError::UserNotFound)?;
    cmd_as_user(&program, user).map_err(CmdError::FailedGettingEnv)
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
/// use nix::unistd::{Uid, User};
/// use polyjuice::cmd_as_user;
///
/// let user = User::from_uid(Uid::from_raw(1000))
///     .expect("user lookup failed")
///     .expect("no user with uid 1000");
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
    let env = get_user_env(&user.name)?;

    let uid = user.uid.as_raw();
    let primary_gid = user.gid.as_raw();

    // Resolve the user's full supplementary group list IN THE PARENT, where
    // calling into NSS is safe. Doing this from inside pre_exec runs after
    // fork() in a multi-threaded program (Tokio), which is undefined per POSIX
    // and in practice causes NSS modules like nss_sss / nss_ldap to silently
    // return only locally-resolvable groups — dropping all LDAP/SSSD-provided
    // memberships. The child would then EPERM on chown/setgid to any group
    // that lives outside /etc/group.
    let name = CString::new(user.name.as_bytes())
        .expect("username from passwd database must not contain a NUL byte");
    let groups = getgrouplist(&name, primary_gid).map_err(env::Error::GroupLookupFailed)?;

    let mut new_cmd = Command::new(program);
    unsafe {
        new_cmd.pre_exec(move || {
            // Between fork() and execve() only async-signal-safe syscalls are
            // valid in a multi-threaded program. setgroups / setgid / setuid
            // are all single syscalls and safe; libc::initgroups is not.
            if libc::setgroups(groups.len() as _, groups.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::setgid(primary_gid as libc::gid_t) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::setuid(uid as libc::uid_t) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    new_cmd.env_clear().envs(env);

    Ok(new_cmd)
}

// On Linux glibc, getgrouplist takes (uid_t/gid_t) for basegroup and a
// *mut gid_t buffer. On macOS the same function takes c_int basegroup and a
// *mut c_int buffer. We resolve via the platform-appropriate signature, then
// normalize to Vec<gid_t> for the caller (setgroups takes *const gid_t on
// both platforms).
#[cfg(not(target_os = "macos"))]
fn getgrouplist(name: &CStr, primary_gid: u32) -> std::io::Result<Vec<libc::gid_t>> {
    let basegroup = primary_gid as libc::gid_t;
    let mut ngroups: libc::c_int = 64;
    loop {
        let cap = ngroups as usize;
        let mut buf: Vec<libc::gid_t> = vec![0; cap];
        let rc =
            unsafe { libc::getgrouplist(name.as_ptr(), basegroup, buf.as_mut_ptr(), &mut ngroups) };
        if rc >= 0 {
            buf.truncate(ngroups as usize);
            return Ok(buf);
        }
        if (ngroups as usize) <= cap {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "getgrouplist failed without growing ngroups",
            ));
        }
    }
}

#[cfg(target_os = "macos")]
fn getgrouplist(name: &CStr, primary_gid: u32) -> std::io::Result<Vec<libc::gid_t>> {
    let basegroup = primary_gid as libc::c_int;
    let mut ngroups: libc::c_int = 64;
    loop {
        let cap = ngroups as usize;
        let mut buf: Vec<libc::c_int> = vec![0; cap];
        let rc =
            unsafe { libc::getgrouplist(name.as_ptr(), basegroup, buf.as_mut_ptr(), &mut ngroups) };
        if rc >= 0 {
            buf.truncate(ngroups as usize);
            return Ok(buf.into_iter().map(|g| g as libc::gid_t).collect());
        }
        if (ngroups as usize) <= cap {
            return Err(std::io::Error::other(
                "getgrouplist failed without growing ngroups",
            ));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_user_is_user_not_found() {
        let result = cmd_as_username("ls", "polyjuice_no_such_user");
        assert!(matches!(result, Err(CmdError::UserNotFound)));
    }

    // nix's User::from_name maps a NUL-containing name to Ok(None) rather
    // than an error, so this must surface as UserNotFound.
    #[test]
    fn nul_in_username_is_user_not_found() {
        let result = cmd_as_username("ls", "root\0oops");
        assert!(matches!(result, Err(CmdError::UserNotFound)));
    }

    // Without root the lookup succeeds but fetching the user's env is
    // refused. This runs in a plain `cargo test`; it will fail if the suite
    // is run as root.
    #[test]
    fn known_user_without_root_fails_getting_env() {
        let me = User::from_uid(nix::unistd::getuid())
            .expect("uid lookup failed")
            .expect("current uid has no passwd entry");

        let result = cmd_as_username("ls", &me.name);

        assert!(matches!(
            result,
            Err(CmdError::FailedGettingEnv(env::Error::InsufficientPrivileges))
        ));
    }

    // End-to-end: build the command as SUDO_USER, actually run `id -u`, and
    // check the child process really became that user.
    #[test]
    #[ignore = "requires root: sudo cargo test -- --ignored"]
    fn runs_program_as_target_user() {
        let username = std::env::var("SUDO_USER")
            .expect("SUDO_USER not set; run via `sudo cargo test -- --ignored`");
        let expected_uid = User::from_name(&username)
            .expect("user lookup failed")
            .expect("SUDO_USER has no passwd entry")
            .uid;

        let mut cmd = cmd_as_username("id", &username).expect("cmd_as_username failed");
        let output = cmd.arg("-u").output().expect("failed running id");

        assert!(output.status.success());
        let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(uid, expected_uid.as_raw().to_string());
    }

    // Same end-to-end check, but the child is forked (pre_exec) while the
    // multi-threaded Tokio runtime's worker threads are alive — the exact
    // scenario the group-resolution comments in cmd_as_user are about.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires root: sudo cargo test -- --ignored"]
    async fn runs_program_as_target_user_tokio() {
        let username = std::env::var("SUDO_USER")
            .expect("SUDO_USER not set; run via `sudo cargo test -- --ignored`");
        let expected_uid = User::from_name(&username)
            .expect("user lookup failed")
            .expect("SUDO_USER has no passwd entry")
            .uid;

        let mut cmd = cmd_as_username("id", &username).expect("cmd_as_username failed");
        let output = cmd.arg("-u").output().expect("failed running id");

        assert!(output.status.success());
        let uid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(uid, expected_uid.as_raw().to_string());
    }
}
