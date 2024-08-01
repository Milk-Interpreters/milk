use crate::{archs::Architecture, scope::Scope};

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Struct(TypeStruct),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeStruct {
    pub name: String,
    pub fields: Vec<(String, crate::type_::Type)>,
}

impl TypeStruct {
    pub fn size(&self, arch: &dyn Architecture, scope: &Scope) -> usize {
        self.fields
            .iter()
            .map(|(_, type_)| type_.size(arch, scope))
            .sum()
    }

    pub fn offset(&self, arch: &dyn Architecture, name: &str, scope: &Scope) -> usize {
        let mut offset = 0;

        for (field_name, type_) in &self.fields {
            if name == field_name {
                break;
            }

            offset += type_.size(arch, scope);
        }

        offset
    }

    pub fn get_field_type(&self, field: &str) -> Option<&crate::type_::Type> {
        self.fields
            .iter()
            .find(|(name, _)| name == field)
            .map(|(_, type_)| type_)
    }

    pub fn contains(&self, field: &str) -> bool {
        self.fields.iter().any(|(name, _)| name == field)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeTable(Vec<Type>);

impl TypeTable {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn define(&mut self, type_: Type) {
        self.0.push(type_);
    }

    pub fn find(&self, type_name: &str) -> Option<&Type> {
        self.0.iter().find(|type_| match type_ {
            Type::Struct(type_struct) => type_struct.name == type_name,
        })
    }
}
