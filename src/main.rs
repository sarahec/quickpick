//! `qp` executable: thin wrapper around the `quickpick` library.

fn main() {
    quickpick::run(quickpick::parse());
}
