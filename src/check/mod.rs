pub mod borrow;
pub mod drop_elaborate;
pub mod magnet;
pub mod ownership;
pub mod validate;

pub use borrow::check_borrow;
pub use drop_elaborate::elaborate_drops;
pub use magnet::check_magnet;
pub use ownership::check_ownership;
pub use validate::{validate_mir, validate_return_consistency};
