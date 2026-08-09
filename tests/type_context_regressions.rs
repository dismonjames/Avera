use avera_compiler::types::intern::TyCtxt;
use avera_compiler::types::ty::TyData;

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
