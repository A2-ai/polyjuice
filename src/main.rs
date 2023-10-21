use clap::{Parser};
use pam_client::{Context, Flag};

use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::{Instant, Duration};

/// Define the command-line arguments and options using the clap derive pattern
#[derive(Parser)]
struct Args {
    /// "The username to check and create a home directory for"
    #[clap(short, long, value_name = "USERNAME", required = true)]
    username: String,
}

fn check_file_exists_with_polling(username: &str) -> bool {
    let max_polling_time = Duration::from_secs(90); // 1.5 minutes
    let poll_interval = Duration::from_secs(1);
    let extensions_json_path = format!("/data/user-homes/{}/.local/share/code-server/extensions/extensions.json", username);

    let mut elapsed_time = Duration::from_secs(0);

    while elapsed_time < max_polling_time {
        if fs::metadata(&extensions_json_path).is_ok() {
            println!("File found at: {}", extensions_json_path);
            return true;
        }

        // Sleep for the poll interval before checking again
        sleep(poll_interval);
        elapsed_time += poll_interval;
    }

    println!("File not found within the polling time.");
    false
}

fn login_as_user(username: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Check if the user's home directory exists
    let home_directory = format!("/data/user-homes/{}", username); // Modify the path as needed
    if !Path::new(&home_directory).exists() {
        println!("Home directory does not exist. Creating a PAM session to create it...");

        // Initialize a PAM context
        let mut context = Context::new(
            "my-service",     // Service name
            Some(username),   // Preset username
            pam_client::conv_null::Conversation::new(),
        )?;

        // Skip authentication if already done by other means (e.g., SSH key)

        // Measure the time taken for PAM session creation
        let start_time = Instant::now();

        // Validate the account
        context.acct_mgmt(Flag::NONE).expect("Account validation failed");


        // Open a session and initialize credentials
        let mut _session = context.open_session(Flag::SILENT).expect("Session opening failed");
        // Calculate and print the elapsed time
        let elapsed_time = start_time.elapsed();
        println!("PAM session opened in {:.2?}.", elapsed_time);

        let extensions_present = check_file_exists_with_polling(username);
        let elapsed_time_to_extensions = start_time.elapsed();
        if extensions_present {
            println!("Extensions present in user home dir in {:.2?}.", elapsed_time_to_extensions);
        } else {
            println!("Extensions not present in user home dir in {:.2?}.", elapsed_time_to_extensions);
        }
    }
    Ok(())
}

fn main() {
    // Parse the command-line arguments using the Args struct
    let args: Args = Args::parse();

    if let Err(err) = login_as_user(&args.username) {
        eprintln!("Error: {}", err);
    }
}
