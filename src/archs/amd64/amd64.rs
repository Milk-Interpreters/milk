use crate::{
    archs::{ArchError, Architecture},
    codegen::locations::{self, Global, Local, MoveDestination, MoveSource, Offset, SourceParam},
    parser::{CmpOp, ExprLit, Expression},
    register::{
        allocator::{AllocatorError, RegisterAllocator},
        Register,
    },
    scope::Scope,
    types::{Type, TypeError},
};
use indoc::formatdoc;

pub struct Amd64 {
    buf: String,
    registers: RegisterAllocator,
}

impl Architecture for Amd64 {
    fn new() -> Self {
        Self {
            buf: String::new(),
            registers: RegisterAllocator::new(vec![
                Register::new("r15b", "r15w", "r15d", "r15"),
                Register::new("r14b", "r14w", "r14d", "r14"),
                Register::new("r13b", "r13w", "r13d", "r13"),
                Register::new("r12b", "r12w", "r12d", "r12"),
                Register::new("r11b", "r11w", "r11d", "r11"),
                Register::new("r10b", "r10w", "r10d", "r10"),
                Register::new("r9b", "r9w", "r9d", "r9"),
                Register::new("r8b", "r8w", "r8d", "r8"),
                Register::new("cl", "cx", "ecx", "rcx"),
                Register::new("dl", "dx", "edx", "rdx"),
                Register::new("sil", "si", "esi", "rsi"),
                Register::new("dil", "di", "edi", "rdi"),
            ]),
        }
    }

    #[inline]
    fn alignment(&self) -> usize {
        16
    }

    fn size(&self, type_: &Type) -> usize {
        match type_ {
            _ => unreachable!(),
        }
    }

    fn alloc(&mut self) -> Result<Register, AllocatorError> {
        self.registers.alloc()
    }

    fn free(&mut self, register: Register) -> Result<(), AllocatorError> {
        self.registers.free(register)
    }

    fn size_name(size: usize) -> &'static str {
        match size {
            1 => "byte ptr",
            2 => "word ptr",
            4 => "dword ptr",
            8 => "qword ptr",
            _ => unreachable!(),
        }
    }

    fn mov(
        &mut self,
        src: MoveSource,
        dest: MoveDestination,
        scope: &Scope,
    ) -> Result<(), ArchError> {
        match src {
            MoveSource::Global(global, signed) => self.mov_global(global, dest, signed, scope),
            MoveSource::Local(local, signed) => self.mov_local(local, dest, signed, scope),
            MoveSource::Param(param, signed) => self.mov_param(param, dest, signed, scope),
            MoveSource::Register(register, signed) => {
                self.mov_register(register, dest, signed, scope)
            }
            MoveSource::Lit(literal) => self.mov_literal(literal, dest, scope),
        }
    }

    fn declare(&mut self, name: &str, size: usize) {
        self.buf.push_str(&formatdoc!(
            "
            \t.comm {} {}
            ",
            name,
            size,
        ));
    }

    fn negate(&mut self, r: &Register) {
        self.buf.push_str(&formatdoc!(
            "
            \tneg {}
            ",
            r.qword(),
        ));
    }

    fn not(&mut self, r1: &Register, r2: &Register) {
        self.buf.push_str(&formatdoc!(
            "
            \tcmp {}, 0
            \tsete {}
            ",
            r1.qword(),
            r2.byte(),
        ));
    }

    fn add(&mut self, dest: &MoveDestination, r2: &locations::Register) {
        self.buf.push_str(&formatdoc!(
            "
            \tadd {}, {}
            ",
            dest,
            r2
        ));
    }

    fn sub(&mut self, r1: &Register, r2: &Register) {
        self.buf.push_str(&formatdoc!(
            "
            \tsub {}, {}
            ",
            r1.qword(),
            r2.qword(),
        ));
    }

    fn mul(&mut self, r1: &Register, r2: &Register) {
        self.buf.push_str(&formatdoc!(
            "
            \timul {}, {}
            ",
            r1.qword(),
            r2.qword(),
        ));
    }

    //NOTE: if mafs doesn't works, prolly because of this
    fn div(&mut self, r1: &Register, r2: &Register) {
        self.buf.push_str(&formatdoc!(
            "
            \tmov rax, {}
            \tcqo
            \tidiv {}
            \tmov {}, rax
            ",
            r1.qword(),
            r2.qword(),
            r1.qword(),
        ));
    }

    fn cmp(&mut self, r1: &Register, r2: &Register, cmp: CmpOp) {
        let ins = match cmp {
            CmpOp::LessThan => formatdoc!("setl {}", r1.byte()),
            CmpOp::LessEqual => formatdoc!("setle {}", r1.byte()),
            CmpOp::GreaterThan => formatdoc!("setg {}", r1.byte()),
            CmpOp::GreaterEqual => formatdoc!("setge {}", r1.byte()),
            CmpOp::Equal => formatdoc!("sete {}", r1.byte()),
            CmpOp::NotEqual => formatdoc!("setne {}", r1.byte()),
        };

        self.buf.push_str(&formatdoc!(
            "
           \tcmp {}, {}
           \t{}
           ",
            r1.qword(),
            r2.qword(),
            ins,
        ));
    }

    fn fn_preamble(&mut self, name: &str, stackframe: usize) {
        self.buf.push_str(&formatdoc!(
            "
            .global {name}
            {name}:
                push rbp
                mov rbp, rsp
                sub rsp, {stackframe}
            ",
        ));
    }

    fn fn_postamble(&mut self, name: &str, stackframe: usize) {
        self.buf.push_str(&formatdoc!(
            "
            {}_ret:
                add rsp, {}
                leave
                ret
            ",
            name,
            stackframe,
        ));
    }

    fn ret(&mut self, r: Register, type_: Type, scope: &Scope) -> Result<(), TypeError> {
        self.mov_impl(
            (
                &MoveDestination::Register(locations::Register {
                    register: &r,
                    size: type_.size(self, scope)?,
                    offset: None,
                }),
                type_.size(self, scope)?,
            ),
            ("rax", 8),
            type_.signed(),
        );

        Ok(())
    }

    fn jmp(&mut self, label: &str) {
        self.buf.push_str(&formatdoc!(
            "
            \tjmp {}
            ",
            label
        ));
    }

    fn call_fn(&mut self, name: &str, r: Option<&Register>) {
        match r {
            Some(r) => {
                self.buf.push_str(&formatdoc!(
                    "
                    \tcall {name}
                    \tmov {}, rax
                    ",
                    r.qword()
                ));
            }
            None => {
                self.buf.push_str(&formatdoc!(
                    "
                    \tcall {name}
                    ",
                ));
            }
        }
    }

    fn move_function_argument(&mut self, r: Register, i: usize) {
        self.buf.push_str(&formatdoc!(
            "
            \tmov {}, {}
            ",
            self.registers
                .get(self.registers.len() - i - 1)
                .unwrap()
                .qword(),
            r.qword(),
        ));
    }

    fn lea(&mut self, dest: &Register, offset: usize) {
        self.buf.push_str(&formatdoc!(
            "
            \tlea {}, [rbp - {}]
            ",
            dest.qword(),
            offset,
        ));
    }

    fn finish(&mut self) -> Vec<u8> {
        self.buf.insert_str(0, ".section .text\n");
        self.buf.as_bytes().to_vec()
    }
}

impl Amd64 {
    fn mov_impl<T, I>(
        &mut self,
        (src, src_size): (&T, usize),
        (dest, dest_size): (&I, usize),
        signed: bool,
    ) where
        T: std::fmt::Display + ?Sized,
        I: std::fmt::Display + ?Sized,
    {
        if dest_size > src_size {
            if signed {
                self.buf.push_str(&formatdoc!("\tmovsx {dest}, {src}\n",));
            } else {
                self.buf.push_str(&formatdoc!("\tmovzx {dest}, {src}\n",));
            }
        } else {
            self.buf.push_str(&formatdoc!("\tmov {dest}, {src}\n",));
        }
    }

    fn mov_literal(
        &mut self,
        literal: ExprLit,
        dest: MoveDestination,
        scope: &Scope,
    ) -> Result<(), ArchError> {
        self.mov_impl(
            (&literal, 8),
            (&dest, dest.size()),
            literal.type_(scope)?.signed(),
        );

        Ok(())
    }

    fn mov_local(
        &mut self,
        src: Local,
        dest: MoveDestination,
        signed: bool,
        scope: &Scope,
    ) -> Result<(), ArchError> {
        match dest {
            // NOTE: x86-64 doesn't support indirect to indirect addressing mode so we use tools we already have
            MoveDestination::Local(local) => {
                let mut size = src.size;
                let r = self.alloc()?;

                self.lea(&r, src.offset);

                while size > 0 {
                    let chunk_size = match size {
                        8.. => 8,
                        4..=7 => 4,
                        2..=3 => 2,
                        1 => 1,
                        0 => unreachable!(),
                    };

                    let r_tmp = self.alloc()?;

                    self.mov_register(
                        locations::Register {
                            size: chunk_size,
                            offset: Some(Offset((size - chunk_size).try_into().unwrap())),
                            register: &r,
                        },
                        MoveDestination::Register(locations::Register {
                            size: chunk_size,
                            offset: None,
                            register: &r_tmp,
                        }),
                        signed,
                        scope,
                    )?;
                    self.mov_register(
                        locations::Register {
                            size: chunk_size,
                            offset: None,
                            register: &r_tmp,
                        },
                        MoveDestination::Local(Local {
                            offset: local.offset,
                            size: chunk_size,
                        }),
                        signed,
                        scope,
                    )?;

                    self.free(r_tmp)?;
                    size -= chunk_size;
                }

                self.free(r)?;
            }
            MoveDestination::Global(_) => {
                todo!();
            }
            MoveDestination::Register(register) => {
                self.mov_impl((&src, src.size), (&register, register.size), signed);
            }
        };

        Ok(())
    }

    fn mov_param(
        &mut self,
        src: SourceParam,
        dest: MoveDestination,
        signed: bool,
        scope: &Scope,
    ) -> Result<(), ArchError> {
        self.mov(
            MoveSource::Register(
                locations::Register {
                    register: &self
                        .registers
                        .get(self.registers.len() - 1 - src.n)
                        .unwrap(),
                    size: src.size,
                    offset: None,
                },
                signed,
            ),
            dest,
            scope,
        )?;

        Ok(())
    }

    fn mov_global(
        &self,
        _src: Global,
        _dest: MoveDestination,
        _signed: bool,
        _scope: &Scope,
    ) -> Result<(), ArchError> {
        todo!();
    }

    fn mov_register(
        &mut self,
        src: locations::Register,
        dest: MoveDestination,
        signed: bool,
        _: &Scope,
    ) -> Result<(), ArchError> {
        self.mov_impl((&src, src.size), (&dest, dest.size()), signed);

        Ok(())
    }
}

impl std::fmt::Display for locations::Register<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.offset {
            Some(offset) => {
                write!(
                    f,
                    "{} [{}{}]",
                    Amd64::size_name(self.size),
                    self.register.qword(),
                    offset
                )
            }
            None => {
                write!(f, "{}", self.register.from_size(self.size))
            }
        }
    }
}

impl std::fmt::Display for Local {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [rbp - {}]", Amd64::size_name(self.size), self.offset)
    }
}

impl std::fmt::Display for Global<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.offset {
            Some(offset) => {
                write!(
                    f,
                    "{} [{} - {}]",
                    Amd64::size_name(self.size),
                    self.label,
                    offset
                )
            }
            None => {
                write!(f, "{} [{}]", Amd64::size_name(self.size), self.label)
            }
        }
    }
}

impl std::fmt::Display for MoveDestination<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global(global) => write!(f, "{global}"),
            Self::Local(local) => write!(f, "{local}"),
            Self::Register(register) => write!(f, "{register}"),
        }
    }
}

mod test {
    use crate::parser::IntLitRepr;

    #[test]
    fn mov_literal() {
        use super::Amd64;
        use crate::codegen::locations::Offset;
        use crate::{
            archs::Architecture,
            codegen::locations::{self, MoveDestination},
            parser::{ExprLit, UIntLitRepr},
            register::Register,
            scope::Scope,
        };

        let r = Register::new("r15b", "r15w", "r15d", "r15");
        let scope = Scope::new();
        let tests = vec![
            (
                (
                    MoveDestination::Global(locations::Global {
                        size: 4,
                        offset: Some(5),
                        label: "foo",
                    }),
                    ExprLit::UInt(UIntLitRepr::new(15_000)),
                ),
                "\tmov dword ptr [foo - 5], 15000\n",
            ),
            (
                (
                    MoveDestination::Global(locations::Global {
                        size: 8,
                        offset: None,
                        label: "foo",
                    }),
                    ExprLit::Int(IntLitRepr::new(-5)),
                ),
                "\tmov qword ptr [foo], -5\n",
            ),
            (
                (
                    MoveDestination::Local(locations::Local { size: 4, offset: 1 }),
                    ExprLit::UInt(UIntLitRepr::new(5)),
                ),
                "\tmov dword ptr [rbp - 1], 5\n",
            ),
            (
                (
                    MoveDestination::Register(locations::Register {
                        size: 8,
                        offset: None,
                        register: &r,
                    }),
                    ExprLit::UInt(UIntLitRepr::new(5)),
                ),
                "\tmov r15, 5\n",
            ),
            (
                (
                    MoveDestination::Register(locations::Register {
                        size: 8,
                        offset: Some(Offset(-15)),
                        register: &r,
                    }),
                    ExprLit::UInt(UIntLitRepr::new(5)),
                ),
                "\tmov qword ptr [r15 - 15], 5\n",
            ),
            (
                (
                    MoveDestination::Register(locations::Register {
                        size: 2,
                        offset: Some(Offset(8)),
                        register: &r,
                    }),
                    ExprLit::Int(IntLitRepr::new(-7)),
                ),
                "\tmov word ptr [r15 + 8], -7\n",
            ),
        ];

        for ((dest, lit), expected) in tests {
            let mut arch = Amd64::new();
            arch.mov_literal(lit, dest, &scope).unwrap();

            assert_eq!(arch.buf, expected);
        }
    }
}
