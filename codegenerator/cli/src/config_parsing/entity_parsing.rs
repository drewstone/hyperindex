use anyhow::{anyhow, Context};
use ethers::abi::ethabi::ParamType as EthAbiParamType;
use graphql_parser::schema::{
    Definition, Directive, Document, Field as ObjField, ObjectType, Type as ObjType,
    TypeDefinition, Value,
};
use serde::{Serialize, Serializer};
use std::{collections::HashSet, fmt, path::PathBuf};
use subenum::subenum;

use super::config::EntityMap;
#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    pub entities: Vec<Entity>,
}

impl Schema {
    pub fn empty() -> Self {
        Schema { entities: vec![] }
    }

    fn from_document(document: Document<String>) -> anyhow::Result<Self> {
        let entities = document
            .definitions
            .iter()
            .filter_map(|d| match d {
                Definition::TypeDefinition(type_def) => Some(type_def),
                _ => None,
            })
            .filter_map(|type_def| match type_def {
                TypeDefinition::Object(obj) => Some(obj),
                _ => None,
            })
            .map(|obj| Entity::from_object(obj))
            .collect::<anyhow::Result<_>>()
            .context("Failed contstructing schema from document")?;

        Ok(Schema { entities })
    }

    pub fn parse_from_file(path_to_schema: &PathBuf) -> anyhow::Result<Self> {
        let schema_string = std::fs::read_to_string(&path_to_schema).context(
            format!(
                "EE200: Failed to read schema file at {}. Please ensure that the schema file is placed correctly in the directory.",
                &path_to_schema.to_str().unwrap_or_else(||"bad file path"),
            )
        )?;

        let schema_doc = graphql_parser::parse_schema::<String>(&schema_string)
            .context("EE201: Failed to parse schema as document")?;

        Self::from_document(schema_doc).context("Failed converting schema doc to schema struct")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Entity {
    pub name: String,
    pub fields: Vec<Field>,
}

impl Entity {
    fn from_object(obj: &ObjectType<String>) -> anyhow::Result<Self> {
        let name = obj.name.clone();

        let fields = obj
            .fields
            .iter()
            .map(|f| Field::from_obj_field(f))
            .collect::<anyhow::Result<_>>()
            .context("Failed contsructing fields")?;

        Ok(Entity { name, fields })
    }

    pub fn get_related_entities<'a>(
        &'a self,
        other_entities: &'a EntityMap,
    ) -> anyhow::Result<Vec<(&'a Field, &'a Self)>> {
        let required_entities_with_field = self
            .fields
            .iter()
            .filter_map(|field| {
                let gql_scalar = field.field_type.get_underlying_scalar();
                if let GqlScalar::Custom(entity_name) = gql_scalar {
                    let field_and_entity = other_entities
                        .get(entity_name)
                        .map(|entity| (field, entity))
                        .ok_or_else(|| anyhow!("Entity {} does not exist", entity_name));
                    Some(field_and_entity)
                } else {
                    None
                }
            })
            .collect::<anyhow::Result<_>>()?;

        Ok(required_entities_with_field)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub derived_from_field: Option<String>,
}

impl Field {
    fn from_obj_field(field: &ObjField<String>) -> anyhow::Result<Self> {
        //Get all gql derictives labeled @derivedFrom
        let derived_from_directives = field
            .directives
            .iter()
            .filter(|directive| directive.name == "derivedFrom")
            .collect::<Vec<&Directive<'_, String>>>();

        //Do not allow for multiple @derivedFrom directives
        //If this step is not important and we are fine with just taking the first one
        //in the case of multiple we can just use a find rather than a filter method above
        if derived_from_directives.len() > 1 {
            let msg = anyhow!(
                "EE202: Cannot use more than one @derivedFrom directive at field {}",
                field.name
            );
            return Err(msg);
        }

        let maybe_derived_from_directive = derived_from_directives.get(0);
        let derived_from_field = match maybe_derived_from_directive {
            None => None,
            Some(d) => {
                let field_arg = d.arguments.iter().find(|a| a.0 == "field").ok_or_else(|| {
                    anyhow!(
                        "EE203: No 'field' argument supplied to @derivedFrom directive on field {}",
                        field.name
                    )
                })?;
                match &field_arg.1 {
                        Value::String(val) => Some(val.clone()),
                        _ => Err(anyhow!("EE204: 'field' argument in @derivedFrom directive on field {} needs to contain a string", field.name))?
                    }
            }
        };

        let field_type = FieldType::from_obj_field_type(&field.field_type);

        Ok(Field {
            name: field.name.clone(),
            derived_from_field,
            field_type,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldType {
    Single(GqlScalar),
    ListType(Box<FieldType>),
    NonNullType(Box<FieldType>),
}

impl FieldType {
    fn from_obj_field_type(field_type: &ObjType<'_, String>) -> Self {
        match field_type {
            ObjType::NamedType(name) => FieldType::Single(name.as_str().into()),
            ObjType::NonNullType(inner) => {
                FieldType::NonNullType(Box::new(Self::from_obj_field_type(inner.as_ref())))
            }
            ObjType::ListType(inner) => {
                FieldType::ListType(Box::new(Self::from_obj_field_type(inner.as_ref())))
            }
        }
    }

    pub fn to_postgres_type(&self, entities_set: &HashSet<String>) -> anyhow::Result<String> {
        let composed_type_name = match self {
        Self::Single(gql_scalar) => {
                gql_scalar.to_postgres_type(entities_set)?
        }
        Self::ListType(field_type) => match field_type.as_ref() {
            //Postgres doesn't support nullable types inside of arrays
            Self::NonNullType(field_type) =>format!("{}[]",field_type.to_postgres_type(entities_set)?),
            Self::Single(gql_scalar)   => Err(anyhow!(
                "EE208: Nullable scalars inside lists are unsupported. Please include a '!' after your '{}' scalar", gql_scalar
            ))?,
            Self::ListType(_) => Err(anyhow!("EE209: Nullable multidimensional lists types are unsupported,\
                please include a '!' for your inner list type eg. [[Int!]!]"))?,
        },
        Self::NonNullType(field_type) => format!(
            "{} NOT NULL",
            field_type.to_postgres_type(entities_set)?
        ),
    };
        Ok(composed_type_name)
    }

    pub fn is_optional(&self) -> bool {
        !matches!(self, Self::NonNullType(_))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::ListType(_))
            || matches!(
                self,
                Self::NonNullType(field_type) if field_type.is_array()
            )
    }

    pub fn to_rescript_type(&self, entities_set: &HashSet<String>) -> anyhow::Result<String> {
        let composed_type_name = match self {
            //Only types in here should be non optional
            Self::NonNullType(field_type) => match field_type.as_ref() {
                Self::Single(gql_scalar) => gql_scalar.to_rescript_type(entities_set)?,
                Self::ListType(field_type) => {
                    format!("array<{}>", field_type.to_rescript_type(entities_set)?)
                }
                //This case shouldn't happen, and should recurse without adding any types if so
                //A double non null would be !! in gql
                Self::NonNullType(field_type) => field_type.to_rescript_type(entities_set)?,
            },
            //If we match this case it missed the non null path entirely and should be optional
            Self::Single(gql_scalar) => {
                format!("option<{}>", gql_scalar.to_rescript_type(entities_set)?)
            }
            //If we match this case it missed the non null path entirely and should be optional
            Self::ListType(field_type) => {
                format!(
                    "option<array<{}>>",
                    field_type.to_rescript_type(entities_set)?
                )
            }
        };
        Ok(composed_type_name)
    }

    fn get_underlying_scalar(&self) -> &GqlScalar {
        match self {
            Self::Single(gql_scalar) => gql_scalar,
            Self::ListType(field_type) | Self::NonNullType(field_type) => {
                field_type.get_underlying_scalar()
            }
        }
    }

    pub fn get_maybe_entity_name(&self) -> Option<String> {
        if let GqlScalar::Custom(entity_name) = self.get_underlying_scalar() {
            Some(entity_name.clone())
        } else {
            None
        }
    }

    fn to_string(&self) -> String {
        match &self {
            Self::Single(gql_scalar) => gql_scalar.to_string(),
            Self::ListType(field_type) => format!("[{}]", field_type.to_string()),
            Self::NonNullType(field_type) => format!("{}!", field_type.to_string()),
        }
    }
}

// Implement the Display trait for the custom struct
impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Serialize for FieldType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

#[subenum(BuiltInGqlScalar, AdditionalGqlScalar)]
#[derive(Debug, Clone, PartialEq, strum_macros::Display, Eq, Hash)]
pub enum GqlScalar {
    #[subenum(BuiltInGqlScalar)]
    ID,
    #[subenum(BuiltInGqlScalar)]
    String,
    #[subenum(BuiltInGqlScalar)]
    Int,
    #[subenum(BuiltInGqlScalar)]
    Float,
    #[subenum(BuiltInGqlScalar)]
    Boolean,
    #[subenum(AdditionalGqlScalar)]
    BigInt,
    #[subenum(AdditionalGqlScalar)]
    Bytes,
    Custom(String),
}

pub fn ethabi_type_to_field_type(abi_type: &EthAbiParamType) -> anyhow::Result<FieldType> {
    use FieldType::{ListType, NonNullType, Single};
    match abi_type {
        EthAbiParamType::Uint(_size) | EthAbiParamType::Int(_size) => {
            Ok(NonNullType(Box::new(Single(GqlScalar::BigInt))))
        }
        EthAbiParamType::Bool => Ok(NonNullType(Box::new(Single(GqlScalar::Boolean)))),
        EthAbiParamType::Address
        | EthAbiParamType::Bytes
        | EthAbiParamType::String
        | EthAbiParamType::FixedBytes(_) => Ok(NonNullType(Box::new(Single(GqlScalar::String)))),
        EthAbiParamType::Array(abi_type) | EthAbiParamType::FixedArray(abi_type, _) => {
            let inner_type = ethabi_type_to_field_type(abi_type)?;
            Ok(NonNullType(Box::new(ListType(Box::new(inner_type)))))
        }
        EthAbiParamType::Tuple(_abi_types) => Err(anyhow!("Tuples not handled currently.")),
    }
}

impl From<&str> for GqlScalar {
    fn from(s: &str) -> Self {
        match s {
            "ID" => GqlScalar::ID,
            "String" => GqlScalar::String,
            "Int" => GqlScalar::Int,
            "Float" => GqlScalar::Float, // Should we allow this type? Rounding issues will abound.
            "Boolean" => GqlScalar::Boolean,
            "BigInt" => GqlScalar::BigInt, // NOTE: we aren't setting precision and scale - see (8.1.2) https://www.postgresql.org/docs/current/datatype-numeric.html
            "Bytes" => GqlScalar::Bytes,
            custom_type => GqlScalar::Custom(custom_type.to_string()),
        }
    }
}

impl GqlScalar {
    fn to_postgres_type(&self, entities_set: &HashSet<String>) -> anyhow::Result<String> {
        let converted = match self {
            GqlScalar::ID => "text",
            GqlScalar::String => "text",
            GqlScalar::Int => "integer",
            GqlScalar::Float => "numeric", // Should we allow this type? Rounding issues will abound.
            GqlScalar::Boolean => "boolean",
            GqlScalar::Bytes => "text",
            GqlScalar::BigInt => "numeric", // NOTE: we aren't setting precision and scale - see (8.1.2) https://www.postgresql.org/docs/current/datatype-numeric.html
            GqlScalar::Custom(named_type) => {
                if entities_set.contains(named_type) {
                    "text" //This would be the ID of another defined entity
                } else {
                    Err(anyhow!(
                        "EE207: Failed to parse undefined type: {}",
                        named_type
                    ))?
                }
            }
        };
        Ok(converted.to_string())
    }

    fn to_rescript_type(&self, entities_set: &HashSet<String>) -> anyhow::Result<String> {
        let res_type = match self {
            GqlScalar::ID => "string",
            GqlScalar::String => "string",
            GqlScalar::Int => "int",
            GqlScalar::BigInt => "Ethers.BigInt.t",
            GqlScalar::Float => "float",
            GqlScalar::Bytes => "string",
            GqlScalar::Boolean => "bool",
            GqlScalar::Custom(entity_name) => {
                if entities_set.contains(entity_name) {
                    "id"
                } else {
                    Err(anyhow!(
                        "EE207: Failed to parse undefined type: {}",
                        entity_name
                    ))?
                }
            }
        };
        Ok(res_type.to_string())
    }
}

// //impl
// fn gql_type_is_optional(gql_type: &ObjType<String>) -> bool {
//     !matches!(gql_type, ObjType::NonNullType(_))
// }
//
// //impl
// fn gql_type_to_postgres_relational_type(
//     field_name: &String,
//     gql_type: &ObjType<String>,
//     entities_set: &HashSet<String>,
//     derived_from_field_key: Option<String>,
// ) -> Option<EntityRelationalTypesTemplate> {
//     match gql_type {
//         ObjType::NamedType(named) if entities_set.contains(named) => {
//             Some(EntityRelationalTypesTemplate {
//                 relational_key: field_name.clone().to_capitalized_options(),
//                 mapped_entity: named.to_capitalized_options(),
//                 relationship_type: RelationshipTypeTemplate::Object,
//                 is_optional: true,
//                 is_array: false,
//                 derived_from_field_key,
//             })
//         }
//         ObjType::NamedType(_) => None,
//         ObjType::ListType(gql_type) => {
//             match gql_type_to_postgres_relational_type(
//                 field_name,
//                 gql_type,
//                 entities_set,
//                 derived_from_field_key,
//             ) {
//                 Some(mut relational_type) => {
//                     relational_type.relationship_type = RelationshipTypeTemplate::Array;
//                     relational_type.is_array = true;
//
//                     Some(relational_type)
//                 }
//                 None => None,
//             }
//         }
//         ObjType::NonNullType(gql_type) => {
//             match gql_type_to_postgres_relational_type(
//                 field_name,
//                 gql_type,
//                 entities_set,
//                 derived_from_field_key,
//             ) {
//                 Some(mut relational_type) => {
//                     relational_type.is_optional = false;
//                     Some(relational_type)
//                 }
//                 None => None,
//             }
//         }
//     }
// }
//
// //impl
// fn gql_named_types_to_rescript_types(
//     named_type: &str,
//     entities_set: &HashSet<String>,
// ) -> Result<String, String> {
//     match named_type {
//         "ID" => Ok("string".to_owned()),
//         "String" => Ok("string".to_owned()),
//         "Int" => Ok("int".to_owned()),
//         "BigInt" => Ok("Ethers.BigInt.t".to_owned()),
//         "Float" => Ok("float".to_owned()),
//         "Bytes" => Ok("string".to_owned()),
//         "Boolean" => Ok("bool".to_owned()),
//         custom_type => {
//             if entities_set.contains(custom_type) {
//                 Ok("id".to_owned())
//             } else {
//                 let error_message = format!("Failed to parse undefined type: {}", custom_type);
//                 Err(error_message.to_owned())
//             }
//         }
//     }
// }
//
// //impl
// enum NullableContainer {
//     NotNullable,
//     Nullable,
// }
//
// //impl
// fn gql_type_to_rescript_type_with_container_wrapper(
//     gql_type: &ObjType<String>,
//     container_type: NullableContainer,
//     entities_set: &HashSet<String>,
// ) -> Result<String, String> {
//     let composed_type_name = match (gql_type, container_type) {
//         (ObjType::NamedType(named), NullableContainer::NotNullable) => {
//             gql_named_types_to_rescript_types(named, entities_set)?
//         }
//         (ObjType::NamedType(named), NullableContainer::Nullable) => {
//             format!(
//                 "option<{}>",
//                 gql_named_types_to_rescript_types(named, entities_set)?
//             )
//         }
//         (ObjType::ListType(gql_type), NullableContainer::NotNullable) => format!(
//             "array<{}>",
//             gql_type_to_rescript_type_with_container_wrapper(
//                 gql_type,
//                 NullableContainer::Nullable,
//                 entities_set
//             )?
//         ),
//         (ObjType::ListType(gql_type), NullableContainer::Nullable) => format!(
//             "option<array<{}>>",
//             gql_type_to_rescript_type_with_container_wrapper(
//                 gql_type,
//                 NullableContainer::Nullable,
//                 entities_set
//             )?
//         ),
//         (ObjType::NonNullType(gql_type), _) => (gql_type_to_rescript_type_with_container_wrapper(
//             gql_type,
//             NullableContainer::NotNullable,
//             entities_set,
//         )?)
//         .to_string(),
//     };
//     Ok(composed_type_name)
// }
//
// fn gql_type_to_rescript_type(
//     gql_type: &ObjType<String>,
//     entities_set: &HashSet<String>,
// ) -> Result<String, String> {
//     gql_type_to_rescript_type_with_container_wrapper(
//         gql_type,
//         NullableContainer::Nullable,
//         entities_set,
//     )
// }

pub fn strip_option_from_rescript_type_str(s: &str) -> String {
    let prefix = "option<";
    let suffix = ">";
    if s.starts_with(prefix) && s.ends_with(suffix) {
        let without_prefix = s.strip_prefix(prefix).unwrap();
        let without_suffix = without_prefix.strip_suffix(suffix).unwrap();
        return without_suffix.to_string();
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::{FieldType, GqlScalar, Schema};
    use std::collections::HashSet;

    #[test]
    fn gql_type_to_rescript_type_string() {
        let empty_set = HashSet::new();
        let rescript_type = FieldType::Single(GqlScalar::String)
            .to_rescript_type(&empty_set)
            .expect("expected rescript option string");

        assert_eq!(rescript_type, "option<string>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_int() {
        let empty_set = HashSet::new();
        let rescript_type = FieldType::Single(GqlScalar::Int)
            .to_rescript_type(&empty_set)
            .expect("expected rescript option string");

        assert_eq!(rescript_type, "option<int>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_non_null_int() {
        let empty_set = HashSet::new();
        let rescript_type = FieldType::NonNullType(Box::new(FieldType::Single(GqlScalar::Int)))
            .to_rescript_type(&empty_set)
            .expect("expected rescript type string");

        assert_eq!(rescript_type, "int".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_non_null_array() {
        let empty_set = HashSet::new();
        let rescript_type = FieldType::NonNullType(Box::new(FieldType::ListType(Box::new(
            FieldType::NonNullType(Box::new(FieldType::Single(GqlScalar::Int))),
        ))))
        .to_rescript_type(&empty_set)
        .expect("expected rescript type string");

        assert_eq!(rescript_type, "array<int>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_null_array_int() {
        let empty_set = HashSet::new();

        let rescript_type = FieldType::ListType(Box::new(FieldType::Single(GqlScalar::Int)))
            .to_rescript_type(&empty_set)
            .expect("expected rescript type string");

        assert_eq!(rescript_type, "option<array<option<int>>>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_entity() {
        let mut entity_set = HashSet::new();
        let test_entity_string = String::from("TestEntity");
        entity_set.insert(test_entity_string.clone());
        let rescript_type = FieldType::Single(GqlScalar::Custom(test_entity_string))
            .to_rescript_type(&entity_set)
            .expect("expected rescript type string");

        assert_eq!(rescript_type, "option<id>".to_owned());
    }

    // #[test]
    // fn gql_type_to_relational_type_entity() {
    //     let mut entity_set = HashSet::new();
    //     let test_entity_string = String::from("TestEntity");
    //     entity_set.insert(test_entity_string.clone());
    //     let gql_object_type = Type::NamedType(test_entity_string.clone());
    //     let field_name = String::from("testField1");
    //     let derived_from_field_key = None;
    //     let result = gql_type_to_postgres_relational_type(
    //         &field_name,
    //         &gql_object_type,
    //         &entity_set,
    //         derived_from_field_key.clone(),
    //     );
    //     let expect_output = Some(EntityRelationalTypesTemplate {
    //         is_optional: true,
    //         is_array: false,
    //         relational_key: field_name.to_capitalized_options(),
    //         mapped_entity: test_entity_string.to_capitalized_options(),
    //         relationship_type: RelationshipTypeTemplate::Object,
    //         derived_from_field_key,
    //     });
    //     assert_eq!(result, expect_output);
    // }
    //
    // #[test]
    // fn gql_type_to_non_null_relational_type_entity() {
    //     let mut entity_set = HashSet::new();
    //     let test_entity_string = String::from("TestEntity");
    //     entity_set.insert(test_entity_string.clone());
    //     let gql_object_type =
    //         Type::NonNullType(Box::new(Type::NamedType(test_entity_string.clone())));
    //     let field_name = String::from("testField1");
    //     let derived_from_field_key = None;
    //     let result = gql_type_to_postgres_relational_type(
    //         &field_name,
    //         &gql_object_type,
    //         &entity_set,
    //         derived_from_field_key.clone(),
    //     );
    //     let expect_output = Some(EntityRelationalTypesTemplate {
    //         is_optional: false,
    //         is_array: false,
    //         relational_key: field_name.to_capitalized_options(),
    //         mapped_entity: test_entity_string.to_capitalized_options(),
    //         relationship_type: RelationshipTypeTemplate::Object,
    //         derived_from_field_key,
    //     });
    //     assert_eq!(result, expect_output);
    // }
    //
    // #[test]
    // fn gql_type_to_relational_type_array_entity() {
    //     let mut entity_set = HashSet::new();
    //     let test_entity_string = String::from("TestEntity");
    //     entity_set.insert(test_entity_string.clone());
    //     let gql_array_object_type =
    //         Type::ListType(Box::new(Type::NamedType(test_entity_string.clone())));
    //
    //     let field_name = String::from("testField1");
    //     let derived_from_field_key = None;
    //     let result = gql_type_to_postgres_relational_type(
    //         &field_name,
    //         &gql_array_object_type,
    //         &entity_set,
    //         derived_from_field_key.clone(),
    //     );
    //     let expect_output = Some(EntityRelationalTypesTemplate {
    //         is_optional: true,
    //         is_array: true,
    //         relational_key: field_name.to_capitalized_options(),
    //         mapped_entity: test_entity_string.to_capitalized_options(),
    //         relationship_type: RelationshipTypeTemplate::Array,
    //         derived_from_field_key,
    //     });
    //     assert_eq!(result, expect_output);
    // }
    // #[test]
    // fn gql_type_to_non_null_relational_type_array_entity() {
    //     let mut entity_set = HashSet::new();
    //     let test_entity_string = String::from("TestEntity");
    //     entity_set.insert(test_entity_string.clone());
    //     let gql_array_object_type = Type::NonNullType(Box::new(Type::ListType(Box::new(
    //         Type::NonNullType(Box::new(Type::NamedType(test_entity_string.clone()))),
    //     ))));
    //
    //     let field_name = String::from("testField1");
    //     let derived_from_field_key = None;
    //     let result = gql_type_to_postgres_relational_type(
    //         &field_name,
    //         &gql_array_object_type,
    //         &entity_set,
    //         derived_from_field_key.clone(),
    //     );
    //     let expect_output = Some(EntityRelationalTypesTemplate {
    //         is_optional: false,
    //         is_array: true,
    //         relational_key: field_name.to_capitalized_options(),
    //         mapped_entity: test_entity_string.to_capitalized_options(),
    //         relationship_type: RelationshipTypeTemplate::Array,
    //         derived_from_field_key,
    //     });
    //     assert_eq!(result, expect_output);
    // }

    #[test]
    fn field_type_is_optional_test() {
        let test_scalar = GqlScalar::Custom(String::from("TestEntity"));
        let test_field_type = FieldType::Single(test_scalar);
        assert!(
            test_field_type.is_optional(),
            "single field should have been optional"
        );

        // ListType:
        let test_list_type = FieldType::ListType(Box::new(test_field_type));
        assert!(
            test_list_type.is_optional(),
            "list field should have been optional"
        );

        // NonNullType
        let gql_array_non_null_type = FieldType::NonNullType(Box::new(test_list_type));
        assert!(
            !gql_array_non_null_type.is_optional(),
            "non-null field should not be optioonal"
        );
    }

    fn gql_type_to_postgres_type_test_helper(gql_field_str: &str) -> String {
        let schema_string = format!(
            r#"
        type TestEntity @entity {{
          test_field: {}
        }}
        "#,
            gql_field_str
        );
        let schema_doc = graphql_parser::schema::parse_schema::<String>(&schema_string).unwrap();
        let empty_entities_set = HashSet::new();

        let schema = Schema::from_document(schema_doc).expect("bad schema");

        let test_field = schema.entities[0].fields[0].clone();

        test_field
            .field_type
            .to_postgres_type(&empty_entities_set)
            .expect("unable to get postgres type")
    }

    #[test]
    fn gql_single_not_null_array_to_pg_type() {
        let gql_type = "[String!]!";
        let pg_type = gql_type_to_postgres_type_test_helper(gql_type);
        assert_eq!(pg_type, "text[] NOT NULL");
    }

    #[test]
    fn gql_multi_not_null_array_to_pg_type() {
        let gql_type = "[[Int!]!]!";
        let pg_type = gql_type_to_postgres_type_test_helper(gql_type);
        assert_eq!(pg_type, "integer[][] NOT NULL");
    }

    #[test]
    #[should_panic]
    fn gql_single_nullable_array_to_pg_type_should_panic() {
        let gql_type = "[Int]!"; //Nested lists need to be not nullable
        gql_type_to_postgres_type_test_helper(gql_type);
    }

    #[test]
    #[should_panic]
    fn gql_multi_nullable_array_to_pg_type_should_panic() {
        let gql_type = "[[Int!]]!"; //Nested lists need to be not nullable
        gql_type_to_postgres_type_test_helper(gql_type);
    }

    #[test]
    fn strip_option_removes_option() {
        assert_eq!(
            super::strip_option_from_rescript_type_str("option<bool>"),
            "bool"
        );

        assert_eq!(
            super::strip_option_from_rescript_type_str("option<array<string>>"),
            "array<string>"
        );

        assert_eq!(
            super::strip_option_from_rescript_type_str("array<string>"),
            "array<string>"
        );
        assert_eq!(
            super::strip_option_from_rescript_type_str("array<string>"),
            "array<string>"
        );
        assert_eq!(super::strip_option_from_rescript_type_str("option<>"), "");
        assert_eq!(
            super::strip_option_from_rescript_type_str("option<"),
            "option<"
        );
    }

    #[test]
    fn test_nullability_to_string() {
        use FieldType::{ListType, NonNullType, Single};
        let scalar = NonNullType(Box::new(ListType(Box::new(Single(GqlScalar::Int)))));

        let expected_output = "[Int]!".to_string();

        assert_eq!(scalar.to_string(), expected_output);
    }
}
