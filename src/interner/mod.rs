use std::rc::Rc;

use fxhash::FxHashMap;

mod symbol;
pub use symbol::sym;

pub type Symbol = u32;

#[derive(Debug, Clone)]
pub struct Interner {
    /// Used for checking if a symbol is already interned
    map: FxHashMap<Rc<str>, Symbol>,
    /// Used for retrieving the symbol value
    vec: Vec<Rc<str>>,
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

impl Interner {
    pub fn new() -> Self {
        let mut this = Self {
            map: FxHashMap::default(),
            vec: Vec::new(),
        };
        this.prefill();
        this
    }

    fn prefill(&mut self) {
        for &name in symbol::SYM_PREFILL {
            let s: Rc<str> = name.into();
            let idx = self.vec.len() as Symbol;
            self.vec.push(s.clone());
            self.map.insert(s, idx);
        }
    }

    pub fn intern(&mut self, s: impl AsRef<str>) -> Symbol {
        let s = s.as_ref();
        if let Some(&sym) = self.map.get(s) {
            return sym;
        }

        let s: Rc<str> = s.into();
        let idx = self.vec.len() as Symbol;
        self.vec.push(s.clone());
        self.map.insert(s, idx);
        idx
    }

    pub fn lookup(&self, sym: Symbol) -> &str {
        &self.vec[sym as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_identical_strings() {
        let mut interner = Interner::new();
        let sym1 = interner.intern("hello");
        let sym2 = interner.intern("hello");
        assert_eq!(sym1, sym2);
    }

    #[test]
    fn test_intern_different_strings() {
        let mut interner = Interner::new();
        let sym1 = interner.intern("hello");
        let sym2 = interner.intern("world");
        assert_ne!(sym1, sym2);
    }

    #[test]
    fn test_lookup() {
        let mut interner = Interner::new();
        let sym = interner.intern("hello");
        assert_eq!(interner.lookup(sym), "hello");
    }

    #[test]
    fn test_intern_multiple_lookups() {
        let mut interner = Interner::new();
        let s1 = "apple";
        let s2 = "banana";
        let s3 = "cherry";

        let sym1 = interner.intern(s1);
        let sym2 = interner.intern(s2);
        let sym3 = interner.intern(s3);

        assert_eq!(interner.lookup(sym1), s1);
        assert_eq!(interner.lookup(sym2), s2);
        assert_eq!(interner.lookup(sym3), s3);

        assert_eq!(interner.intern(s1), sym1);
        assert_eq!(interner.intern(s2), sym2);
        assert_eq!(interner.intern(s3), sym3);
    }
}
