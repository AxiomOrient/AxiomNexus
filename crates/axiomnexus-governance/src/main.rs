fn main() {
    if let Err(message) = axiomnexus_governance::run(std::env::args().skip(1)) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
