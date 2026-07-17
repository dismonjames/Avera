pub mod body;
pub mod place;
pub mod rvalue;
pub mod terminator;

pub use body::{Body, Local};
pub use place::{Place, PlaceElem};
pub use rvalue::{BinOp as MirBinOp, Rvalue, UnOp as MirUnOp};
pub use terminator::Terminator;
