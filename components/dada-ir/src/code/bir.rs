//! The "bir" (pronounced "beer") is the "base ir" that we use
//! for interpretation.

use crate::{
    class::Class,
    code::validated::op::Op,
    filename::Filename,
    function::Function,
    in_ir_db::InIrDb,
    intrinsic::Intrinsic,
    origin_table::HasOriginIn,
    prelude::InIrDbExt,
    storage::{Atomic, SpannedSpecifier},
    word::{SpannedOptionalWord, Word},
};
use dada_id::{id, prelude::*, tables};
use salsa::DebugWithDb;

use super::{syntax, validated};

salsa::entity2! {
    entity Bir in crate::Jar {
        origin: Function,
        #[value ref] data: BirData,
        #[value ref] origins: Origins,
    }
}

impl DebugWithDb<dyn crate::Db + '_> for Bir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &dyn crate::Db) -> std::fmt::Result {
        let self_in_ir_db = &self.in_ir_db(db.as_dyn_ir_db());
        DebugWithDb::fmt(self.data(db), f, self_in_ir_db)
    }
}

impl InIrDb<'_, Bir> {
    fn tables(&self) -> &Tables {
        &self.data(self.db()).tables
    }
}

/// Stores the ast for a function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BirData {
    /// Interning tables for expressions and the like.
    pub tables: Tables,

    /// First N local variables are the parameters.
    pub num_parameters: usize,

    /// The starting block
    pub start_basic_block: BasicBlock,
}

impl DebugWithDb<InIrDb<'_, Bir>> for BirData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        let mut dbg = f.debug_struct("bir::Bir");
        dbg.field("start_basic_block", &self.start_basic_block);

        for basic_block in self.all_basic_blocks() {
            dbg.field(
                &format!("{basic_block:?}"),
                &basic_block.data(db.tables()).debug(db),
            );
        }

        dbg.finish()
    }
}

impl BirData {
    pub fn new(tables: Tables, num_parameters: usize, start_basic_block: BasicBlock) -> Self {
        Self {
            tables,
            num_parameters,
            start_basic_block,
        }
    }

    pub fn tables(&self) -> &Tables {
        &self.tables
    }

    pub fn num_parameters(&self) -> usize {
        self.num_parameters
    }

    pub fn parameters(&self) -> impl Iterator<Item = LocalVariable> {
        LocalVariable::range(0, self.num_parameters)
    }

    pub fn max_local_variable(&self) -> LocalVariable {
        LocalVariable::max_key(&self.tables)
    }

    pub fn max_basic_block(&self) -> BasicBlock {
        BasicBlock::max_key(&self.tables)
    }

    pub fn all_basic_blocks(&self) -> impl Iterator<Item = BasicBlock> {
        self.max_basic_block().iter()
    }
}

tables! {
    /// Tables that store the data for expr in the AST.
    /// You can use `tables[expr]` (etc) to access the data.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Tables {
        local_variables: alloc LocalVariable => LocalVariableData,
        basic_blocks: alloc BasicBlock => BasicBlockData,
        statements: alloc Statement => StatementData,
        terminators: alloc Terminator => TerminatorData,
        exprs: alloc Expr => ExprData,
        places: alloc Place => PlaceData,
        target_places: alloc TargetPlace => TargetPlaceData,
    }
}

origin_table! {
    /// Side table that contains the spans for everything in a syntax tree.
    /// This isn't normally needed except for diagnostics, so it's
    /// kept separate to avoid reducing incremental reuse.
    /// You can request it by invoking the `spans`
    /// method in the `dada_parse` prelude.
    #[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Origins {
        local_variables: LocalVariable => validated::LocalVariableOrigin,
        basic_blocks: BasicBlock => syntax::Expr,
        statements: Statement => syntax::Expr,
        terminator: Terminator => syntax::Expr,
        expr: Expr => syntax::Expr,
        place: Place => syntax::Expr,
        target_place: TargetPlace => syntax::Expr,
    }
}

id!(pub struct LocalVariable);

impl DebugWithDb<InIrDb<'_, Bir>> for LocalVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        let id = u32::from(*self);
        let data = self.data(db.tables());
        let name = data.name.map(|n| n.as_str(db.db())).unwrap_or("temp");
        write!(f, "{name}{{{id}}}")
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Hash, Debug)]
pub struct LocalVariableData {
    /// Name given to this variable by the user.
    /// If it is None, then this is a temporary
    /// introduced by the compiler.
    pub name: Option<Word>,

    /// Specifier given this variable by the user
    /// (possibly defaulted). If this is `None`,
    /// then this is a temporary introduced by the compiler,
    /// and it gets the specifier `Any`.
    pub specifier: Option<SpannedSpecifier>,

    pub atomic: Atomic,
}

id!(pub struct BasicBlock);

impl<Db: ?Sized> DebugWithDb<Db> for BasicBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, _db: &Db) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

impl BasicBlockData {
    /// Number of "elements" in this block, including the terminator.
    pub fn elements(&self) -> usize {
        self.statements.len() + 1
    }

    /// Get a particular "element" in this block (either a statement or a terminator).
    pub fn element_at(&self, index: usize) -> BasicBlockElement {
        if index < self.statements.len() {
            BasicBlockElement::Statement(self.statements[index])
        } else {
            BasicBlockElement::Terminator(self.terminator)
        }
    }
}

impl DebugWithDb<InIrDb<'_, Bir>> for BasicBlockData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        f.debug_tuple("BasicBlockData")
            .field(&self.statements.debug(db))
            .field(&self.terminator.debug(db))
            .finish()
    }
}

/// An "element" of a basic block is a statement or terminator.
///
/// (In case you are curious, I made this term up, it's not standard.)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BasicBlockElement {
    Statement(Statement),
    Terminator(Terminator),
}

id!(pub struct Statement);

impl DebugWithDb<InIrDb<'_, Bir>> for Statement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        let origin = self.origin_in(db.origins(db.db()));
        let result = f
            .debug_tuple("")
            .field(&self.data(db.tables()).debug(db))
            .field(&origin)
            .finish();
        result
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum StatementData {
    /// Assign the result of evaluating an expression to a place.
    /// This is the preferred form of assignment, and covers
    /// cases like `a := b` as well as `a := 22`. In these case, either
    /// (a) we know statically the declared mode for `a` and so we
    /// can prepare an expression like `b.give` or `b.lease` in advance
    /// or (b) the rhs is an rvalue, like `22`, and so is always given.
    AssignExpr(TargetPlace, Expr),

    /// Captures an assignment like
    ///
    /// ```notrust
    /// foo.bar := baz
    /// ```
    ///
    /// This case is challenging because, until we know
    /// the declared type of `bar` at runtime, we don't
    /// know whether to give `baz`, lease it, or what.
    AssignPlace(TargetPlace, Place),

    /// Clears the value from the given local variable.
    Clear(LocalVariable),

    /// In terms of the semantics, this is a no-op.
    /// It is used by the time traveling debugger.
    ///
    /// It indicates the moment when one of the breakpoint expressions
    /// in the given file (identified by the usize index) is about
    /// to start executon.
    BreakpointStart(Filename, usize),

    /// In terms of the semantics, this is a no-op.
    /// It is used by the time traveling debugger.
    ///
    /// It indicates the moment when one of the breakpoint expressions
    /// in the given file (identified by the usize index) is about
    /// to complete and produce the (optional) `Place` as its value.
    ///
    /// The `syntax::Expr` argument is the expression that just
    /// completed. This may not be the same as the expression on which
    /// the breakpoint was set, if that expression was part of a larger
    /// place or other "compound" that could not be executed independently.
    ///
    /// Any side-effects from the breakpoint will have taken place
    /// when this statement executes.
    BreakpointEnd(Filename, usize, syntax::Expr, Option<Place>),
}

impl DebugWithDb<InIrDb<'_, Bir>> for StatementData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        match self {
            StatementData::AssignExpr(place, expr) => f
                .debug_tuple("AssignExpr")
                .field(&place.debug(db))
                .field(&expr.debug(db))
                .finish(),

            StatementData::AssignPlace(target, source) => f
                .debug_tuple("AssignPlace")
                .field(&target.debug(db))
                .field(&source.debug(db))
                .finish(),

            StatementData::Clear(lv) => f.debug_tuple("Clear").field(&lv.debug(db)).finish(),

            StatementData::BreakpointStart(filename, index) => f
                .debug_tuple("BreakpoingStart")
                .field(&filename.debug(db.db()))
                .field(index)
                .finish(),

            StatementData::BreakpointEnd(filename, index, e, p) => f
                .debug_tuple("BreakpointEnd")
                .field(&filename.debug(db.db()))
                .field(index)
                .field(e)
                .field(&p.debug(db))
                .finish(),
        }
    }
}

id!(pub struct Terminator);

impl DebugWithDb<InIrDb<'_, Bir>> for Terminator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        DebugWithDb::fmt(self.data(db.tables()), f, db)
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum TerminatorData {
    Goto(BasicBlock),
    If(Place, BasicBlock, BasicBlock),
    StartAtomic(BasicBlock),
    EndAtomic(BasicBlock),
    Return(Place),
    Assign(TargetPlace, TerminatorExpr, BasicBlock),
    Error,
    Panic,
}

impl DebugWithDb<InIrDb<'_, Bir>> for TerminatorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        match self {
            TerminatorData::Goto(block) => f.debug_tuple("Goto").field(block).finish(),
            TerminatorData::If(condition, if_true, if_false) => f
                .debug_tuple("If")
                .field(&condition.debug(db))
                .field(&if_true.debug(db))
                .field(&if_false.debug(db))
                .finish(),
            TerminatorData::StartAtomic(block) => {
                f.debug_tuple("StartAomic").field(&block.debug(db)).finish()
            }
            TerminatorData::EndAtomic(block) => {
                f.debug_tuple("EndAtomic").field(&block.debug(db)).finish()
            }
            TerminatorData::Return(value) => {
                f.debug_tuple("Return").field(&value.debug(db)).finish()
            }
            TerminatorData::Assign(target, expr, next) => f
                .debug_tuple("Assign")
                .field(&target.debug(db))
                .field(&expr.debug(db))
                .field(&next.debug(db))
                .finish(),
            TerminatorData::Error => f.debug_tuple("Error").finish(),
            TerminatorData::Panic => f.debug_tuple("Panic").finish(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum TerminatorExpr {
    Await(Place),

    /// Call `function(arguments...)`. The `labels` for each
    /// argument are present as well.
    Call {
        function: Place,
        arguments: Vec<Place>,
        labels: Vec<SpannedOptionalWord>,
    },
}

impl DebugWithDb<InIrDb<'_, Bir>> for TerminatorExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        match self {
            TerminatorExpr::Await(place) => f.debug_tuple("Await").field(&place.debug(db)).finish(),
            TerminatorExpr::Call {
                function,
                arguments,
                labels,
            } => f
                .debug_tuple("Call")
                .field(&function.debug(db))
                .field(&arguments.debug(db))
                .field(&labels.debug(db.db()))
                .finish(),
        }
    }
}

id!(pub struct Expr);

impl DebugWithDb<InIrDb<'_, Bir>> for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        write!(f, "{:?}", self.data(db.tables()).debug(db))
    }
}
#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum ExprData {
    /// true, false
    BooleanLiteral(bool),

    /// `22i`, `22_222i`, etc
    SignedIntegerLiteral(i64),

    /// `22u`, `22_222u`, etc
    UnsignedIntegerLiteral(u64),

    /// `22`, `22_222`, etc
    IntegerLiteral(u64),

    /// `2.2`
    FloatLiteral(eq_float::F64),

    /// `"foo"` with no format strings
    StringLiteral(Word),

    /// `expr.reserve`
    ///
    /// not (presently) actual syntax, emitted as part of lowering
    Reserve(Place),

    /// `expr.share`
    Share(Place),

    /// `expr.lease`
    Lease(Place),

    /// `expr.shlease`
    Shlease(Place),

    /// `expr.give`
    Give(Place),

    /// `()`
    Unit,

    /// `(a, b, ...)` (i.e., at least 2)
    Tuple(Vec<Place>),

    /// `a + b`
    Op(Place, Op, Place),

    /// `- 1`
    Unary(Op, Place),

    /// parse or other error
    Error,
}

impl DebugWithDb<InIrDb<'_, Bir>> for ExprData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        match self {
            ExprData::BooleanLiteral(b) => write!(f, "{}", b),
            ExprData::IntegerLiteral(w) => write!(f, "{}", w),
            ExprData::UnsignedIntegerLiteral(w) => write!(f, "{}", w),
            ExprData::SignedIntegerLiteral(w) => write!(f, "{}", w),
            ExprData::StringLiteral(w) => write!(f, "{:?}", w.as_str(db.db())),
            ExprData::FloatLiteral(w) => write!(f, "{}", w),
            ExprData::Reserve(p) => write!(f, "{:?}.reserve", p.debug(db)),
            ExprData::Share(p) => write!(f, "{:?}.share", p.debug(db)),
            ExprData::Lease(p) => write!(f, "{:?}.lease", p.debug(db)),
            ExprData::Shlease(p) => write!(f, "{:?}.shlease", p.debug(db)),
            ExprData::Give(p) => write!(f, "{:?}.give", p.debug(db)),
            ExprData::Unit => write!(f, "()"),
            ExprData::Tuple(vars) => write_parenthesized_places(f, vars, db),
            ExprData::Op(lhs, op, rhs) => {
                write!(f, "{:?} {} {:?}", lhs.debug(db), op.str(), rhs.debug(db))
            }
            ExprData::Error => write!(f, "<error>"),
            ExprData::Unary(op, rhs) => {
                write!(f, "{} {:?}", op.str(), rhs.debug(db))
            }
        }
    }
}

fn write_parenthesized_places(
    f: &mut std::fmt::Formatter<'_>,
    vars: &[Place],
    db: &InIrDb<'_, Bir>,
) -> std::fmt::Result {
    write!(f, "(")?;
    for (v, i) in vars.iter().zip(0..) {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{:?}", v.debug(db))?;
    }
    write!(f, ")")?;
    Ok(())
}

id!(pub struct Place);

impl DebugWithDb<InIrDb<'_, Bir>> for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        write!(f, "{:?}", self.data(db.tables()).debug(db))
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum PlaceData {
    LocalVariable(LocalVariable),
    Function(Function),
    Class(Class),
    Intrinsic(Intrinsic),
    Dot(Place, Word),
}

impl DebugWithDb<InIrDb<'_, Bir>> for PlaceData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        match self {
            PlaceData::LocalVariable(v) => write!(f, "{:?}", v.debug(db)),
            PlaceData::Function(func) => write!(f, "{:?}", func.debug(db.db())),
            PlaceData::Class(class) => write!(f, "{:?}", class.debug(db.db())),
            PlaceData::Intrinsic(intrinsic) => write!(f, "{:?}", intrinsic),
            PlaceData::Dot(p, id) => write!(f, "{:?}.{}", p.debug(db), id.as_str(db.db())),
        }
    }
}

id!(pub struct TargetPlace);

impl DebugWithDb<InIrDb<'_, Bir>> for TargetPlace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        write!(f, "{:?}", self.data(db.tables()).debug(db))
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Debug)]
pub enum TargetPlaceData {
    LocalVariable(LocalVariable),
    Dot(Place, Word),
}

impl DebugWithDb<InIrDb<'_, Bir>> for TargetPlaceData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, db: &InIrDb<'_, Bir>) -> std::fmt::Result {
        match self {
            TargetPlaceData::LocalVariable(v) => write!(f, "{:?}", v.debug(db)),
            TargetPlaceData::Dot(p, id) => write!(f, "{:?}.{}", p.debug(db), id.as_str(db.db())),
        }
    }
}
