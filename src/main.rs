mod app;
mod cli;
mod command;
mod entities;
mod io;
mod linetypes;
mod modules;
mod patterns;
mod scene;
mod snap;
mod store;
mod types;
mod ui;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if let Some(batch) = cli::parse_batch_args(&args[1..]) {
        match cli::run_batch_export(batch) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("h7cad: {e}");
                std::process::ExitCode::from(1)
            }
        }
    } else {
        match app::run() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("h7cad (GUI): {e:?}");
                std::process::ExitCode::from(1)
            }
        }
    }
}
