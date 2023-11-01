use clap::{error, Parser};
use pam_client::{Context, ConversationHandler, Flag, Session};
use core::fmt;
use std::io::{Stderr, BufReader};
use std::io::BufRead;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread::{sleep, self};
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

use std::collections::HashMap;

fn get_env_as_map() -> Result<HashMap<String, String>, String> {
    // Execute the command and capture the output
    let output = Command::new("su")
        .arg("-")
        .arg("devin")
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

    let start_time = Instant::now();
    let envs = get_env_as_map().unwrap_or_else(|e| {
        println!("Error: {}", e);
        std::process::exit(1);
    });
    let duration = start_time.elapsed();
    println!("Time elapsed for getting envs is: {:?}", duration);

    let mut child = Command::new("R")
        .arg("-e")
        .arg("Sys.info()")
        //.arg(r#"R -e 'names(Sys.getenv()); message("USER: ", Sys.getenv("USER"), "\nHOME: ", Sys.getenv("HOME"),  "\nPATH: ", Sys.getenv("PATH"))'"#)
        //.arg(r#"R -e 'names(Sys.getenv()); message("USER: ", Sys.getenv("USER"), "\nHOME: ", Sys.getenv("HOME"),  "\nPATH: ", Sys.getenv("PATH"))'"#)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(envs)
        .uid(uid)
        .spawn()
        .expect("R command failed to start");

    let stdout_reader = child.stdout.take().expect("Failed to open stdout");
    let stderr_reader = child.stderr.take().expect("Failed to open stderr");

    let stdout_thread = thread::spawn(move || {
        let stdout = BufReader::new(stdout_reader);
        for line in stdout.lines() {
            let line = line.expect("Failed to read stdout line");
            println!("[stdout] {}", line);
        }
    });

    let stderr_thread = thread::spawn(move || {
        let stderr = BufReader::new(stderr_reader);
        for line in stderr.lines() {
            let line = line.expect("Failed to read stderr line");
            println!("[stderr] {}", line);
        }
    });

    let status = child.wait().expect("Failed to wait for child process");
    let duration = start_time.elapsed();
    println!("Time elapsed for everything is: {:?}", duration);
    stdout_thread.join().expect("stdout thread panicked");
    stderr_thread.join().expect("stderr thread panicked");

    println!("Finished with status {:?}", status);

}
