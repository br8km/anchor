fn main() {
    if let Err(err) = anchor::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
