use super::Pass;
use crate::{
    parser::{BinOp, Block, Expr, ExprBinary, Expression, ParserError, Stmt, UnOp},
    scope::Scope,
    symbol_table::SymbolTableError,
    type_table::{self as tt, TypeTable},
    types::{Type, TypeError, UintType},
};
use std::collections::HashSet;

type Result<T> = std::result::Result<T, ParserError>;
pub struct TypeChecker;

impl Pass for TypeChecker {
    type State = ();
    type Output = Result<()>;

    fn new(_: Self::State) -> Self {
        Self {}
    }

    fn run_pass(self, stmts: &mut Vec<Stmt>, scope: &mut Scope) -> Self::Output {
        Self::check_type_table(scope.type_table(), scope)?;
        for stmt in stmts {
            Self::check_stmt(stmt, scope)?;
        }

        Ok(())
    }
}

impl TypeChecker {
    fn check_block(block: &Block, scope: &mut Scope) -> Result<()> {
        scope.enter(block.scope.clone());
        Self::check_type_table(scope.type_table(), scope)?;
        for stmt in &block.statements {
            Self::check_stmt(stmt, scope)?;
        }
        scope.leave();

        Ok(())
    }

    fn check_stmt(stmt: &Stmt, scope: &mut Scope) -> Result<()> {
        Ok(match stmt {
            Stmt::VarDecl(stmt) => {
                Self::check_type(&stmt.type_, scope)?;

                if let Some(expr) = &stmt.value {
                    Self::check_assign(stmt.type_.clone(), expr, scope)?;
                }
            }
            Stmt::Expr(expr) => {
                Self::check_expr(expr, scope)?;
            }
            Stmt::Function(stmt) => {
                Self::check_type(&stmt.return_type, scope)?;
                for (_, type_) in &stmt.params {
                    Self::check_type(type_, scope)?;
                }
                Self::check_block(&stmt.block, scope)?;
            }
            Stmt::Return(stmt) => {
                if let Some(expr) = &stmt.expr {
                    Self::check_assign(scope.return_type().unwrap().to_owned(), expr, scope)?;
                }
            }
            Stmt::If(stmt) => {
                Self::bool_expr(&stmt.condition, scope)?;
                Self::check_block(&stmt.consequence, scope)?;
                if let Some(alternative) = &stmt.alternative {
                    Self::check_block(alternative, scope)?;
                }
            }
            Stmt::While(stmt) => {
                Self::bool_expr(&stmt.condition, scope)?;
                Self::check_block(&stmt.block, scope)?;
            }
            Stmt::For(stmt) => {
                if let Some(condition) = &stmt.condition {
                    Self::bool_expr(&condition, scope)?;
                }
                Self::check_block(&stmt.block, scope)?;
            }
            Stmt::Continue | Stmt::Break => (),
        })
    }

    fn check_expr(expr: &Expr, scope: &Scope) -> Result<()> {
        Ok(match expr {
            Expr::Binary(ExprBinary {
                op: BinOp::Assign,
                left,
                right,
            }) => {
                assert!(left.lvalue());
                Self::check_expr(left, scope)?;
                Self::check_expr(right, scope)?;
                Self::check_assign(left.type_(scope)?, right, scope)?;
            }
            Expr::Binary(expr) => {
                Self::check_bin(&expr.op, &expr.left, &expr.right, scope)?;
            }
            Expr::Unary(expr) => match expr.op {
                UnOp::Deref => {
                    let type_ = expr.expr.type_(scope)?;
                    if !matches!(type_, Type::Ptr(..)) {
                        panic!("Can't dereference an expression of type {type_}")
                    }
                }
                UnOp::Address => {
                    if !expr.expr.lvalue() {
                        panic!("Can't get address of {expr:?}");
                    }
                }
                _ => (),
            },
            Expr::Cast(expr) => {
                Self::check_type(&expr.type_, scope)?;
                Self::check_expr(&expr.expr, scope)?;
                Type::cast(expr.expr.type_(&scope)?, expr.type_.clone())?;
            }
            Expr::Lit(_) => (),
            Expr::Ident(expr) => {
                scope.find_symbol(&expr.0).ok_or(ParserError::SymbolTable(
                    SymbolTableError::NotFound(expr.0.clone()),
                ))?;
            }
            Expr::Struct(expr) => {
                Self::check_type(&expr.type_(scope)?, scope)?;

                if scope.find_type(&expr.name).is_none() {
                    return Err(ParserError::Type(TypeError::Nonexistent(expr.name.clone())));
                }
            }
            Expr::Array(expr) => {
                let unique = expr
                    .0
                    .iter()
                    .map(|item| item.type_(scope).unwrap())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();

                if unique.len() != 1 {
                    panic!("Types only of the same type allowed in an array expression")
                }
            }
            Expr::StructAccess(expr) => (),
            Expr::StructMethod(expr) => (),
            Expr::ArrayAccess(expr) => (),
            Expr::FunctionCall(expr) => {
                for expr in &expr.arguments {
                    Self::check_expr(expr, scope)?;
                }

                let (params, return_type) = match expr.expr.type_(scope)? {
                    Type::Fn(params, return_type) => (params, return_type),
                    _ => unreachable!(),
                };
                let args_types = expr
                    .arguments
                    .iter()
                    .map(|expr| expr.type_(scope))
                    .collect::<std::result::Result<Vec<_>, _>>()?;

                if params.len() != expr.arguments.len() {
                    return Err(ParserError::FunctionArguments(
                        params.to_owned(),
                        args_types,
                    ));
                }

                for (expr, type_) in expr.arguments.iter().zip(&params) {
                    if let Err(_) = Self::check_assign(type_.to_owned(), expr, scope) {
                        return Err(ParserError::FunctionArguments(
                            params.to_owned(),
                            args_types,
                        ));
                    }
                }
            }
            Expr::MacroCall(_) => unreachable!(),
        })
    }

    fn bool_expr(expr: &Expr, scope: &Scope) -> Result<()> {
        let type_ = expr.type_(scope)?;

        if type_ != Type::Bool {
            Err(ParserError::Type(TypeError::Mismatched(Type::Bool, type_)))
        } else {
            Ok(())
        }
    }

    fn check_type(type_: &Type, scope: &Scope) -> Result<()> {
        Ok(match type_ {
            Type::Custom(name) => {
                if scope.find_type(name).is_none() {
                    return Err(ParserError::Type(TypeError::Nonexistent(name.to_owned())));
                }
            }
            _ => (),
        })
    }

    fn check_type_table(type_table: &TypeTable, scope: &Scope) -> Result<()> {
        for type_ in &type_table.0 {
            match type_ {
                tt::Type::Struct(type_struct) => {
                    for (_, type_) in &type_struct.fields {
                        Self::check_type(type_, scope)?;
                        if type_ == &Type::Custom(type_struct.name.clone()) {
                            panic!("Recursize type {type_} has infinite size");
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub fn check_bin(op: &BinOp, left: &Expr, right: &Expr, scope: &Scope) -> Result<()> {
        Self::check_expr(left, scope)?;
        Self::check_expr(right, scope)?;

        let left_type = left.type_(scope)?;
        let right_type = right.type_(scope)?;

        Ok(match op {
            BinOp::Add => match ((left, left_type), (right, right_type)) {
                // T* + 10 / 10 + T* / T* + foo / foo + T*
                ((_, Type::Ptr(_)), (expr, offset)) | ((expr, offset), (_, Type::Ptr(_)))
                    if Expr::int_lit_only(expr) || offset == Type::UInt(UintType::Usize) =>
                {
                    ()
                }
                // foo + 10 / 10 + foo
                ((_, type_), (expr, expr_type)) | ((expr, expr_type), (_, type_))
                    if type_.int()
                        && Expr::int_lit_only(expr)
                        && type_ >= Type::common_type(type_.clone(), expr_type.clone()) =>
                {
                    ()
                }
                // foo + bar
                ((_, lhs), (_, rhs)) if lhs.int() && rhs.int() && lhs == rhs => (),
                ((_, left_type), (_, right_type)) => {
                    panic!("Can't add {left_type} to {right_type}")
                }
            },
            BinOp::Sub => match ((left, left_type), (right, right_type)) {
                // T* - T*
                ((_, Type::Ptr(lhs)), (_, Type::Ptr(rhs))) if lhs == rhs => (),
                // T* - 10 / T* - foo
                ((_, Type::Ptr(_)), (expr, offset))
                    if Expr::int_lit_only(expr) || offset == Type::UInt(UintType::Usize) =>
                {
                    ()
                }
                // foo - 10 / 10 - foo
                ((_, type_), (expr, expr_type)) | ((expr, expr_type), (_, type_))
                    if type_.int()
                        && Expr::int_lit_only(expr)
                        && type_ >= Type::common_type(type_.clone(), expr_type.clone()) =>
                {
                    ()
                }
                // foo - bar
                ((_, lhs), (_, rhs)) if lhs.int() && rhs.int() && lhs == rhs => (),
                ((_, left_type), (_, right_type)) => {
                    panic!("Can't subtract {left_type} from {right_type}")
                }
            },
            BinOp::Mul
            | BinOp::Div
            | BinOp::LessThan
            | BinOp::LessEqual
            | BinOp::GreaterThan
            | BinOp::GreaterEqual
            | BinOp::BitwiseOr
            | BinOp::BitwiseAnd
            | BinOp::Shl
            | BinOp::Shr => match ((left, left_type), (right, right_type)) {
                // foo _op_ 10 / 10 _op_ foo
                ((_, type_), (expr, expr_type)) | ((expr, expr_type), (_, type_))
                    if type_.int()
                        && Expr::int_lit_only(expr)
                        && type_ >= Type::common_type(type_.clone(), expr_type.clone()) =>
                {
                    ()
                }
                // foo _op_ bar
                ((_, lhs), (_, rhs)) if lhs.int() && rhs.int() && lhs == rhs => (),
                ((_, left_type), (_, right_type)) => {
                    panic!("Can't perform mafs operation between {left_type} and {right_type}")
                }
            },
            BinOp::LogicalAnd | BinOp::LogicalOr => {
                assert_eq!(left_type, Type::Bool);
                assert_eq!(right_type, Type::Bool);
            }
            BinOp::Assign => unreachable!(),
            BinOp::Equal | BinOp::NotEqual => match ((left, left_type), (right, right_type)) {
                // foo _op_ 10 / 10 _op_ foo
                ((_, type_), (expr, expr_type)) | ((expr, expr_type), (_, type_))
                    if type_.int()
                        && Expr::int_lit_only(expr)
                        && type_ >= Type::common_type(type_.clone(), expr_type.clone()) =>
                {
                    ()
                }
                // foo _op_ bar
                ((_, lhs), (_, rhs)) if lhs.int() && rhs.int() && lhs == rhs => (),
                ((_, left_type), (_, right_type)) => {
                    panic!("Can't check equality between {left_type} and {right_type}")
                }
            },
        })
    }

    fn check_assign(left_type: Type, right: &Expr, scope: &Scope) -> Result<()> {
        Self::check_expr(right, scope)?;

        let right_type = right.type_(scope)?;

        if left_type.ptr() && right_type == Type::Null {
            return Ok(());
        }

        if left_type.arr() {
            if let Expr::Array(array) = right {
                for expr in &array.0 {
                    Self::check_assign(left_type.inner()?, expr, scope)?;
                }

                return Ok(());
            } else {
                unreachable!("Can't assign non-array expression to variable of type array");
            }
        }

        if left_type.int() && Expr::int_lit_only(right) {
            assert!(Type::common_type(left_type.clone(), right_type) <= left_type);
        } else {
            assert_eq!(left_type, right_type);
        }

        Ok(())
    }
}
