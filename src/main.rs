mod codegen;
mod lexer;
mod parser;
mod symtable;

use codegen::CodeGen;
use lexer::Lexer;
use parser::Parser;

fn main() {
    let lexer = Lexer::new(
        "
        u8 foo;
        i8 bar;
        1 + 2 * 3 / 4 - 5;
        "
        .to_string(),
    );

    let parser = Parser::new(lexer);
    let (stmts, symtable) = parser.into_parts();

    dbg!(&stmts);

    CodeGen::new(symtable).compile(stmts, "./nasm/main.nasm");
}
