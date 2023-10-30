use clap::{error, Parser};
use pam_client::{Context, ConversationHandler, Flag, Session};
use core::fmt;
use std::io::Stderr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{env, fs};
use users::get_user_by_name;

/// Define the command-line arguments and options using the clap derive pattern
#[derive(Parser, Debug, Clone)]
struct Args {
    /// "The username to check and create a home directory for"
    #[clap(short, long, value_name = "USERNAME", required = true)]
    username: String,
    #[clap(long, value_name = "PAM")]
    use_pam_session: bool,
}

fn setup_pam_context(
    username: String,
) -> Result<Context<pam_client::conv_null::Conversation>, Box<dyn std::error::Error>> {
    // Check if the user's home directory exists

    // Initialize a PAM context
    let mut context = Context::new(
        "polyjuice",     // Service name
        Some(&username), // Preset username
        pam_client::conv_null::Conversation::new(),
    )?;

    // Skip authentication if already done by other means (e.g., SSH key)

    // Measure the time taken for PAM session creation

    // Validate the account
    context.acct_mgmt(Flag::NONE)?;
    return Ok(context);
}

fn main() {
    // Parse the command-line arguments using the Args struct
    let args: Args = Args::parse();

    let uid = get_user_by_name(&args.username.clone()).unwrap().uid();
    println!("UID: {}", uid);
    let mut ctx = setup_pam_context(args.username).unwrap_or_else(|e| {
        println!("Error: {}", e);
        std::process::exit(1);
    });
    let session = ctx.open_session(Flag::SILENT).unwrap_or_else(|e| {
        println!("Error: {}", e);
        std::process::exit(1);
    });
    println!("-------------- env vars process --------------");
    for (key, value) in env::vars() {
        println!("{key}: {value}");
    }
    println!("-------------- pam session vars session --------------");
    // note this current returns nothing as we set no pam related env vars
    for item in &session.envlist() {
        println!("VAR: {}", item);
    }
    let cmd = Command::new("su")
        .arg("-")
        .arg("devin")
        .arg("-c")
        .arg(r#"R -e 'Sys.sleep(60)'"#)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("R command failed to start");
    let output = cmd.wait_with_output().unwrap();
    dbg!(output);

}
