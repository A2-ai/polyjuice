use clap::Parser;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::thread::{self};
use std::time::Instant;
use std::{fs, process};
use users::get_user_by_name;
use users::os::unix::UserExt;

use polyjuice::{cmd_as_user, try_pam_session};

/// Define the command-line arguments and options using the clap derive pattern
#[derive(Parser, Debug, Clone)]
struct Args {
    /// "The username to check and create a home directory for"
    #[clap(short, long, value_name = "USERNAME", required = true)]
    username: String,
    #[clap(long, value_name = "PAM")]
    use_pam_session: bool,
}

fn main() {
    // Parse the command-line arguments using the Args struct
    let args: Args = Args::parse();

    let start_time = Instant::now();
    let user = get_user_by_name(&args.username.clone()).unwrap_or_else(|| {
        println!("Error: user {} does not exist", args.username);
        process::exit(1);
    });
    println!("got user in {:?}", start_time.elapsed());
    let home_dir = user.home_dir().to_str().unwrap_or_else(|| {
        println!(
            "Error: user {} does not have a home directory",
            args.username
        );
        process::exit(1);
    });
    println!("UID: {}", user.uid());
    println!("GUID: {}", user.primary_group_id());
    println!("home_dir: {}", home_dir);

    if let Ok(metadata) = fs::metadata(home_dir) {
        if metadata.is_dir() {
            println!("Home directory exists: {}", home_dir);
        } else {
            eprintln!("Error: {} is not a directory", home_dir);
        }
    } else {
        println!("Error: Failed to access home directory at {}", home_dir);
        let start_time = Instant::now();
        try_pam_session(args.username.clone()).unwrap_or_else(|e| {
            println!("Error: {}", e);
            std::process::exit(1);
        });
        println!("pam session created in {:?}", start_time.elapsed());
        if let Ok(metadata) = fs::metadata(home_dir) {
            if metadata.is_dir() {
                println!("Home directory now exists: {}", home_dir);
            } else {
                eprintln!("Error: {} still doesn't exist", home_dir);
            }
        }
    }
    let start_time = Instant::now();

    // add environmetn variables with those from the pam session
    // TODO: should the pam env replace? is this the right order or should it be the inverse?
    let duration = start_time.elapsed();
    println!("Time elapsed for getting envs is: {:?}", duration);

    let mut child = cmd_as_user("R", args.username.clone()).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        process::exit(1);
    })
        .arg("-e")
        .arg(r#"names(Sys.getenv()); message("USER: ", Sys.getenv("USER"), "\nHOME: ", Sys.getenv("HOME"),  "\nPATH: ", Sys.getenv("PATH"))"#)
        //.arg(r#"R -e 'names(Sys.getenv()); message("USER: ", Sys.getenv("USER"), "\nHOME: ", Sys.getenv("HOME"),  "\nPATH: ", Sys.getenv("PATH"))'"#)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().expect("R command failed to start");

    // println!("R process started with pid {}", child.id());
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
