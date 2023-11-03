use clap::Parser;
use polyjuice::login_as_user;

/// Define the command-line arguments and options using the clap derive pattern
#[derive(Parser)]
struct Args {
    /// "The username to check and create a home directory for"
    #[clap(short, long, value_name = "USERNAME", required = true)]
    username: String,
}

fn main() {
    // Parse the command-line arguments using the Args struct
    let args: Args = Args::parse();

    if let Err(err) = login_as_user(&args.username) {
        eprintln!("Error: {}", err);
    }
}
