//! Bilateral teleop scaffold — out of scope for first hardware milestone.

fn main() {
    marengo_support::init_tracing();
    tracing::info!(
        "teleop scaffold: implement leader/follower after unilateral G-comp (see docs/tuning.md)"
    );
}
