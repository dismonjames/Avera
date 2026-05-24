use std::fmt;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Default)]
        pub struct $name(u32);

        impl $name {
            #[inline]
            pub const fn new(v: u32) -> Self {
                Self(v)
            }
            #[inline]
            pub const fn get(self) -> u32 {
                self.0
            }
            #[inline]
            pub const fn from_usize(v: usize) -> Self {
                Self(v as u32)
            }
            #[inline]
            pub const fn to_usize(self) -> usize {
                self.0 as usize
            }
            #[inline]
            pub const fn placeholder() -> Self {
                Self(u32::MAX)
            }
            #[inline]
            pub const fn is_placeholder(self) -> bool {
                self.0 == u32::MAX
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.is_placeholder() {
                    write!(f, "{}::<none>", stringify!($name))
                } else {
                    write!(f, "{}({})", stringify!($name), self.0)
                }
            }
        }

        impl From<usize> for $name {
            #[inline]
            fn from(v: usize) -> Self {
                Self::from_usize(v)
            }
        }

        impl From<$name> for usize {
            #[inline]
            fn from(v: $name) -> usize {
                v.to_usize()
            }
        }

        impl crate::symbol::Idx for $name {
            #[inline]
            fn from_usize(v: usize) -> Self {
                Self::from_usize(v)
            }
            #[inline]
            fn to_usize(self) -> usize {
                self.to_usize()
            }
        }
    };
}

pub trait Idx: Copy + Eq + Ord + std::hash::Hash {
    fn from_usize(v: usize) -> Self;
    fn to_usize(self) -> usize;
}

define_id!(/// An AST node id.
NodeId);
define_id!(/// A slot in the type interner.
TypeId);
define_id!(/// A resolved symbol id.
SymbolId);
define_id!(/// A local slot in a MIR body.
LocalId);
define_id!(/// A basic block id.
BlockId);
define_id!(/// A place id (MIR).
PlaceId);
define_id!(/// A field index inside a shape/choice.
FieldId);
define_id!(/// A variant index inside a choice.
VariantId);
define_id!(/// An ability id.
AbilityId);
define_id!(/// A module id.
ModuleId);
define_id!(/// A source file id inside the source map.
FileId);
define_id!(/// A MIR temp (SSA value).
TempId);

pub struct Arena<I: Idx, T> {
    items: Vec<T>,
    _marker: std::marker::PhantomData<I>,
}

impl<I: Idx, T> Default for Arena<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Idx, T> Arena<I, T> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            _marker: std::marker::PhantomData,
        }
    }
    pub fn with_capacity(c: usize) -> Self {
        Self {
            items: Vec::with_capacity(c),
            _marker: std::marker::PhantomData,
        }
    }
    pub fn push(&mut self, value: T) -> I {
        let id = I::from_usize(self.items.len());
        self.items.push(value);
        id
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn next_id(&self) -> I {
        I::from_usize(self.items.len())
    }
}
impl<I: Idx, T> std::ops::Index<I> for Arena<I, T> {
    type Output = T;
    fn index(&self, id: I) -> &T {
        &self.items[id.to_usize()]
    }
}
impl<I: Idx, T> std::ops::IndexMut<I> for Arena<I, T> {
    fn index_mut(&mut self, id: I) -> &mut T {
        &mut self.items[id.to_usize()]
    }
}
impl<I: Idx, T> std::iter::FromIterator<T> for Arena<I, T> {
    fn from_iter<J: IntoIterator<Item = T>>(iter: J) -> Self {
        Self {
            items: iter.into_iter().collect(),
            _marker: std::marker::PhantomData,
        }
    }
}
impl<I: Idx, T: Clone> Arena<I, T> {
    pub fn fill_to(&mut self, id: I, value: T) {
        while self.items.len() <= id.to_usize() {
            self.items.push(value.clone());
        }
    }
}
