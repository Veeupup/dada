use crate::{class::Class, code::Code, function::Function, span::FileSpan, word::Word};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Item {
    Function(Function),
    Class(Class),
}

impl Item {
    pub fn span(self, db: &dyn crate::Db) -> FileSpan {
        match self {
            Item::Function(f) => f.span(db),
            Item::Class(c) => c.span(db),
        }
    }

    pub fn name(self, db: &dyn crate::Db) -> Word {
        match self {
            Item::Function(f) => f.name(db).word(db),
            Item::Class(c) => c.name(db).word(db),
        }
    }

    pub fn name_span(self, db: &dyn crate::Db) -> FileSpan {
        match self {
            Item::Function(f) => f.name(db).span(db),
            Item::Class(c) => c.name(db).span(db),
        }
    }

    pub fn kind_str(self) -> &'static str {
        match self {
            Item::Function(_) => "function",
            Item::Class(_) => "class",
        }
    }

    /// If this item has a code block associated with it, return it.
    /// Else return None.
    pub fn code(self, db: &dyn crate::Db) -> Option<Code> {
        match self {
            Item::Function(f) => Some(f.code(db)),
            Item::Class(_) => None,
        }
    }
}

impl From<Function> for Item {
    fn from(value: Function) -> Self {
        Self::Function(value)
    }
}

impl From<Class> for Item {
    fn from(value: Class) -> Self {
        Self::Class(value)
    }
}

impl<Db: ?Sized + crate::Db> salsa::DebugWithDb<Db> for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &Db) -> std::fmt::Result {
        match self {
            Item::Function(v) => std::fmt::Debug::fmt(&v.debug(db), f),
            Item::Class(v) => std::fmt::Debug::fmt(&v.debug(db), f),
        }
    }
}
