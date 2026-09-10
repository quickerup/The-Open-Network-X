use clap::Parser;
use onx_cli::{balance_query, deploy_contract_boc, serialize_boc_transfer, wallet_create, Cli, Command, WalletCommand};
use std::process;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Wallet { kind } => match kind {
            WalletCommand::Create => {
                let out = match wallet_create("target/onx-cli-wallet") {
                    Ok(resp) => resp,
                    Err(err) => {
                        eprintln!("wallet create failed: {err}");
                        process::exit(1);
                    }
                };
                println!("wallet={} mnemonic={:?} public_key={}", out.wallet, out.mnemonic, out.public_key_hex);
            }
            WalletCommand::Balance { address } => {
                println!("{}", balance_query(&address).unwrap_or_else(|err| {
                    eprintln!("wallet balance failed: {err}");
                    process::exit(1)
                }));
            }
        },
        Command::Transfer { from, to, amount, rpc } => {
            let boc = serialize_boc_transfer(&from, &to, amount).unwrap_or_else(|err| {
                eprintln!("transfer failed: {err}");
                process::exit(1);
            });
            println!("boc={:?}{}", boc, rpc.map(|r| format!(" rpc={r}" )).unwrap_or_default());
        }
        Command::DeployContract { from, code, rpc } => {
            let boc = deploy_contract_boc(&from, &code).unwrap_or_else(|err| {
                eprintln!("deploy-contract failed: {err}");
                process::exit(1);
            });
            println!("boc={:?}{}", boc, rpc.map(|r| format!(" rpc={r}" )).unwrap_or_default());
        }
    }
}
