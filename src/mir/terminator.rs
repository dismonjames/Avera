use crate::mir::place::Place;
use crate::symbol::BlockId;

#[derive(Clone, Debug)]
pub enum Terminator {
    Goto(BlockId),
    Switch {
        discr: Place,
        targets: Vec<(u64, BlockId)>,
        otherwise: Option<BlockId>,
    },
    SwitchInt {
        discr: Place,
        targets: Vec<(i128, BlockId)>,
        otherwise: BlockId,
    },
    Return {
        value: Option<Place>,
    },
    Abort,
    Unreachable,
}
