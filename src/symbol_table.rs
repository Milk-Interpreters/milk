use crate::{
    archs::Architecture,
    codegen::locations::{MoveSource, SourceGlobal, SourceLocal, SourceParam},
    scope::Scope,
    type_::Type,
};

const MAX_SYMBOLS: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub enum Symbol {
    Global(SymbolGlobal),
    Local(SymbolLocal),
    Param(SymbolParam),
    Function(SymbolFunction),
}

impl Symbol {
    pub fn name(&self) -> &str {
        match self {
            Self::Global(global) => &global.name,
            Self::Local(local) => &local.name,
            Self::Param(param) => &param.name,
            Self::Function(function) => &function.name,
        }
    }

    pub fn type_(&self) -> Type {
        match self {
            Self::Global(global) => global.type_.clone(),
            Self::Local(local) => local.type_.clone(),
            Self::Param(param) => param.type_.clone(),
            Self::Function(_) => {
                panic!("Type of function, you wanted to get return type of the function or wat?")
            }
        }
    }

    pub fn to_source(&self, arch: &dyn Architecture, scope: &Scope) -> MoveSource {
        match self {
            Self::Local(symbol) => MoveSource::Local(SourceLocal {
                size: symbol.type_.size(arch, scope),
                signed: symbol.type_.signed(),
                offset: symbol.offset,
            }),
            Self::Global(symbol) => MoveSource::Global(SourceGlobal {
                label: &symbol.name,
                size: symbol.type_.size(arch, scope),
                signed: symbol.type_.signed(),
                offset: None,
            }),
            Self::Param(symbol) => MoveSource::Param(SourceParam {
                size: symbol.type_.size(arch, scope),
                signed: symbol.type_.signed(),
                n: symbol.n,
            }),
            Self::Function(_) => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolGlobal {
    pub name: String,
    pub type_: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLocal {
    pub name: String,
    pub offset: usize,
    pub type_: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolParam {
    pub name: String,
    pub n: usize,
    pub type_: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolFunction {
    pub name: String,
    pub return_type: Type,
    pub parameters: Vec<Type>,
}

#[derive(Debug, PartialEq)]
pub enum SymbolTableError {
    Redeclaration(String),
}

impl std::fmt::Display for SymbolTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Redeclaration(name) => write!(f, "Redeclaration of '{}'", name),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SymbolTable(Vec<Symbol>);

impl SymbolTable {
    pub fn new() -> Self {
        Self(Vec::with_capacity(MAX_SYMBOLS))
    }

    pub fn find(&self, name: &str) -> Option<&Symbol> {
        self.0.iter().find(|symbol| symbol.name() == name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        self.0.iter_mut().find(|symbol| symbol.name() == name)
    }

    pub fn push(&mut self, symbol: Symbol) -> Result<(), SymbolTableError> {
        assert!(self.0.len() < MAX_SYMBOLS);

        if self
            .0
            .iter()
            .map(|symbol| symbol.name())
            .collect::<Vec<&str>>()
            .contains(&symbol.name())
        {
            Err(SymbolTableError::Redeclaration(symbol.name().to_owned()))
        } else {
            self.0.push(symbol);

            Ok(())
        }
    }
}
