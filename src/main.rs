use checkvist_cli::{exit_code, run};

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", err);
        std::process::exit(exit_code(err.kind()));
    }
}
