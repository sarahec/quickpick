//! `qp` executable: thin wrapper around the `quickpick` library.

fn main() {
    if let Err(e) = quickpick::run(quickpick::parse()) {
        eprintln!("qp: {e}");
    }
}
