use avera_compiler::types::intern::TyCtxt;
use avera_compiler::types::ty::{CopyKind, TyData};

#[test]
fn new_from_preserves_interned_types_and_ids() {
    let mut original = TyCtxt::new();
    let array = original.array(original.i64, 37);
    let borrow = original.borrow(array, true);

    let cloned = TyCtxt::new_from(&original);

    assert!(matches!(
        cloned.data(array),
        TyData::Array { elem, size } if *elem == original.i64 && *size == 37
    ));
    assert!(matches!(
        cloned.data(borrow),
        TyData::Borrow { inner, is_mut } if *inner == array && *is_mut
    ));
    assert_eq!(cloned.show(borrow), "&![I64; 37]");
}

#[test]
fn unset_composite_abilities_are_conservative_instead_of_panicking() {
    let mut cx = TyCtxt::new();
    let array = cx.array(cx.i64, 4);
    let abilities = cx.abilities(array);

    assert_eq!(abilities.copy, CopyKind::No);
    assert!(!abilities.drop);
    assert!(!abilities.display);
    assert!(!abilities.equal);
    assert!(!abilities.order);
}
