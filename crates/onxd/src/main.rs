use onxd::{parse_cli_args, run_daemon};
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let config = match parse_cli_args(&args) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    if let Err(err) = run_daemon(config).await {
        eprintln!("onxd failed: {err}");
        std::process::exit(1);
    }
}
