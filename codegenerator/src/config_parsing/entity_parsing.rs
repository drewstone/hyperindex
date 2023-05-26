use crate::{
    capitalization::Capitalize, project_paths::ParsedPaths, EntityParamType, EntityRecordType,
    EntityRelationalTypes,
};
use graphql_parser::schema::{Definition, Type, TypeDefinition};
use std::collections::HashSet;

pub fn get_entity_record_types_from_schema(
    parsed_paths: &ParsedPaths,
) -> Result<Vec<EntityRecordType>, String> {
    let schema_string = std::fs::read_to_string(&parsed_paths.schema_path).map_err(|err| {
        format!(
            "Failed to read schema file at {} with Error: {}",
            &parsed_paths.schema_path.to_str().unwrap_or("unknown file"),
            err.to_string()
        )
    })?;

    let schema_doc = graphql_parser::parse_schema::<String>(&schema_string)
        .map_err(|err| format!("Failed to parse schema with Error: {}", err.to_string()))?;
    let mut schema_object_types = Vec::new();
    let mut entities_set: HashSet<String> = HashSet::new();

    for definition in schema_doc.definitions.iter() {
        match definition {
            Definition::SchemaDefinition(_) => (),
            Definition::TypeDefinition(def) => match def {
                TypeDefinition::Scalar(_) => (),
                TypeDefinition::Object(object) => {
                    entities_set.insert(object.name.clone());
                    schema_object_types.push(object);
                }
                TypeDefinition::Interface(_) => (),
                TypeDefinition::Union(_) => (),
                TypeDefinition::Enum(_) => (),
                TypeDefinition::InputObject(_) => (),
            },
            Definition::DirectiveDefinition(_) => (),
            Definition::TypeExtension(_) => (),
        };
    }
    let mut entity_records = Vec::new();
    for object in schema_object_types.iter() {
        let mut params = Vec::new();
        let mut relational_params = Vec::new();
        for field in object.fields.iter() {
            let param_type = gql_type_to_rescript_type(&field.field_type, &entities_set)?;
            let param_pg_type = gql_type_to_postgres_type(&field.field_type, &entities_set)?;
            let relationship_type =
                gql_type_to_postgres_relational_type(&field.name, &field.field_type, &entities_set);

            params.push(EntityParamType {
                key: field.name.to_owned(),
                type_rescript: param_type,
                type_pg: param_pg_type,
            });
            relational_params.push(relationship_type);
        }

        entity_records.push(EntityRecordType {
            name: object.name.to_owned().to_capitalized_options(),
            params,
            relational_params,
        })
    }
    Ok(entity_records)
}

enum BuiltInGqlScalar {
    ID,
    String,
    Int,
    Float,
    Boolean,
}

enum AdditionalGqlScalar {
    BigInt,
    Bytes,
}

enum GqlScalar {
    BuiltIn(BuiltInGqlScalar),
    Additional(AdditionalGqlScalar),
    Custom(String),
}

fn gql_named_to_scalar(named_type: &str) -> GqlScalar {
    match named_type {
        "ID" => GqlScalar::BuiltIn(BuiltInGqlScalar::ID),
        "String" => GqlScalar::BuiltIn(BuiltInGqlScalar::String),
        "Int" => GqlScalar::BuiltIn(BuiltInGqlScalar::Int),
        "Float" => GqlScalar::BuiltIn(BuiltInGqlScalar::Float), // Should we allow this type? Rounding issues will abound.
        "Boolean" => GqlScalar::BuiltIn(BuiltInGqlScalar::Boolean),
        "BigInt" => GqlScalar::Additional(AdditionalGqlScalar::BigInt), // NOTE: we aren't setting precision and scale - see (8.1.2) https://www.postgresql.org/docs/current/datatype-numeric.html
        "Bytes" => GqlScalar::Additional(AdditionalGqlScalar::Bytes),
        custom_type => GqlScalar::Custom(custom_type.to_string()),
    }
}

fn gql_named_types_to_postgres_types(
    scalar_type: &GqlScalar,
    entities_set: &HashSet<String>,
) -> Result<String, String> {
    let converted = match scalar_type {
        GqlScalar::BuiltIn(scalar) => {
            use BuiltInGqlScalar::{Boolean, Float, Int, String, ID};
            match scalar {
                ID => "text".to_owned(),
                String => "text".to_owned(),
                Int => "integer".to_owned(),
                Float => "numeric".to_owned(), // Should we allow this type? Rounding issues will abound.
                Boolean => "boolean".to_owned(),
            }
        }
        GqlScalar::Additional(scalar) => {
            use AdditionalGqlScalar::{BigInt, Bytes};
            match scalar {
                Bytes => "text".to_owned(),
                BigInt => "numeric".to_owned(), // NOTE: we aren't setting precision and scale - see (8.1.2) https://www.postgresql.org/docs/current/datatype-numeric.html
            }
        }
        GqlScalar::Custom(named_type) => {
            if entities_set.contains(named_type) {
                "text".to_owned() //This would be the ID of another defined entity
            } else {
                let error_message = format!("Failed to parse undefined type: {}", named_type);
                Err(error_message.to_owned())?
            }
        }
    };
    Ok(converted)
}

fn gql_type_to_postgres_type(
    gql_type: &Type<String>,
    entities_set: &HashSet<String>,
) -> Result<String, String> {
    let composed_type_name = match gql_type {
        Type::NamedType(named) => {
            let scalar = gql_named_to_scalar(named);
            gql_named_types_to_postgres_types(&scalar, entities_set)?
        }
        Type::ListType(gql_type) => match *gql_type.clone() {
            //Postgres doesn't support nullable types inside of arrays
            Type::NonNullType(gql_type) =>format!("{}[]", gql_type_to_postgres_type(&gql_type, entities_set)?),
            Type::NamedType(named)   => Err(format!(
                "Nullable scalars inside lists are unsupported. Please include a '!' after your '{}' scalar", named
            ))?,
            Type::ListType(_) => Err("Nullable multidemensional lists types are unsupported, please include a '!' for your inner list type eg. [[Int!]!]")?,
        },
        Type::NonNullType(gql_type) => format!(
            "{} NOT NULL",
            gql_type_to_postgres_type(&gql_type, entities_set)?
        ),
    };
    Ok(composed_type_name)
}

fn gql_type_to_postgres_relational_type(
    field_name: &String,
    gql_type: &Type<String>,
    entities_set: &HashSet<String>,
) -> EntityRelationalTypes {
    match gql_type {
        Type::NamedType(named) if entities_set.contains(named) => EntityRelationalTypes {
            is_entity_relationship: true,
            relational_key: field_name.clone(),
            mapped_entity: named.to_capitalized_options(),
            relationship_type: "object".to_owned(),
        },
        Type::NamedType(_) => EntityRelationalTypes {
            is_entity_relationship: false,
            relational_key: field_name.clone(),
            mapped_entity: "scalar".to_owned().to_capitalized_options(),
            relationship_type: "object".to_owned(),
        },
        Type::ListType(gql_type) => {
            let mut relational_type =
                gql_type_to_postgres_relational_type(&field_name, &gql_type, &entities_set);
            relational_type.relationship_type = "array".to_owned();
            relational_type
        }
        Type::NonNullType(gql_type) => {
            gql_type_to_postgres_relational_type(&field_name, &gql_type, &entities_set)
        }
    }
}
fn gql_named_types_to_rescript_types(
    named_type: &str,
    entities_set: &HashSet<String>,
) -> Result<String, String> {
    match named_type {
        "ID" => Ok("string".to_owned()),
        "String" => Ok("string".to_owned()),
        "Int" => Ok("int".to_owned()),
        "BigInt" => Ok("Ethers.BigInt.t".to_owned()),
        "Float" => Ok("float".to_owned()),
        "Bytes" => Ok("string".to_owned()),
        "Boolean" => Ok("bool".to_owned()),
        custom_type => {
            if entities_set.contains(custom_type) {
                Ok("id".to_owned())
            } else {
                let error_message = format!("Failed to parse undefined type: {}", custom_type);
                Err(error_message.to_owned())
            }
        }
    }
}

enum NullableContainer {
    NotNullable,
    Nullable,
}

fn gql_type_to_rescript_type_with_container_wrapper(
    gql_type: &Type<String>,
    container_type: NullableContainer,
    entities_set: &HashSet<String>,
) -> Result<String, String> {
    let composed_type_name = match (gql_type, container_type) {
        (Type::NamedType(named), NullableContainer::NotNullable) => {
            gql_named_types_to_rescript_types(named, entities_set)?
        }
        (Type::NamedType(named), NullableContainer::Nullable) => {
            format!(
                "option<{}>",
                gql_named_types_to_rescript_types(named, entities_set)?
            )
        }
        (Type::ListType(gql_type), NullableContainer::NotNullable) => format!(
            "array<{}>",
            gql_type_to_rescript_type_with_container_wrapper(
                &gql_type,
                NullableContainer::Nullable,
                entities_set
            )?
        ),
        (Type::ListType(gql_type), NullableContainer::Nullable) => format!(
            "option<array<{}>>",
            gql_type_to_rescript_type_with_container_wrapper(
                &gql_type,
                NullableContainer::Nullable,
                entities_set
            )?
        ),
        (Type::NonNullType(gql_type), _) => format!(
            "{}",
            gql_type_to_rescript_type_with_container_wrapper(
                &gql_type,
                NullableContainer::NotNullable,
                entities_set
            )?
        ),
    };
    Ok(composed_type_name)
}

fn gql_type_to_rescript_type(
    gql_type: &Type<String>,
    entities_set: &HashSet<String>,
) -> Result<String, String> {
    gql_type_to_rescript_type_with_container_wrapper(
        gql_type,
        NullableContainer::Nullable,
        entities_set,
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        capitalization::Capitalize,
        entity_parsing::{gql_type_to_postgres_relational_type, gql_type_to_rescript_type},
        EntityRelationalTypes,
    };
    use graphql_parser::schema::{Definition, Type, TypeDefinition};
    use std::collections::HashSet;

    #[test]
    fn gql_type_to_rescript_type_string() {
        let empty_set = HashSet::new();
        let gql_string_type = Type::NamedType("String".to_owned());
        let result = gql_type_to_rescript_type(&gql_string_type, &empty_set).unwrap();

        assert_eq!(result, "option<string>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_int() {
        let empty_set = HashSet::new();
        let gql_int_type = Type::NamedType("Int".to_owned());
        let result = gql_type_to_rescript_type(&gql_int_type, &empty_set).unwrap();

        assert_eq!(result, "option<int>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_non_null_int() {
        let empty_set = HashSet::new();
        let gql_int_type = Type::NonNullType(Box::new(Type::NamedType("Int".to_owned())));
        let result = gql_type_to_rescript_type(&gql_int_type, &empty_set).unwrap();

        assert_eq!(result, "int".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_non_null_array() {
        let empty_set = HashSet::new();
        let gql_int_type = Type::NonNullType(Box::new(Type::ListType(Box::new(
            Type::NonNullType(Box::new(Type::NamedType("Int".to_owned()))),
        ))));
        let result = gql_type_to_rescript_type(&gql_int_type, &empty_set).unwrap();

        assert_eq!(result, "array<int>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_null_array_int() {
        let empty_set = HashSet::new();
        let gql_int_type = Type::ListType(Box::new(Type::NamedType("Int".to_owned())));
        let result = gql_type_to_rescript_type(&gql_int_type, &empty_set).unwrap();

        assert_eq!(result, "option<array<option<int>>>".to_owned());
    }

    #[test]
    fn gql_type_to_rescript_type_entity() {
        let mut entity_set = HashSet::new();
        let test_entity_string = String::from("TestEntity");
        entity_set.insert(test_entity_string.clone());
        let gql_string_type = Type::NamedType(test_entity_string);
        let result = gql_type_to_rescript_type(&gql_string_type, &entity_set).unwrap();

        assert_eq!(result, "option<id>".to_owned());
    }

    #[test]
    fn gql_type_to_relational_type_scalar() {
        let entity_set = HashSet::new();

        let gql_object_type = Type::NamedType("Int".to_owned());
        let field_name = String::from("testField1");
        let result =
            gql_type_to_postgres_relational_type(&field_name, &gql_object_type, &entity_set);
        let expect_output = EntityRelationalTypes {
            is_entity_relationship: false,
            relational_key: field_name,
            mapped_entity: "scalar".to_owned().to_capitalized_options(),
            relationship_type: "object".to_owned(),
        };
        assert_eq!(result, expect_output);
    }

    #[test]
    fn gql_type_to_relational_type_entity() {
        let mut entity_set = HashSet::new();
        let test_entity_string = String::from("TestEntity");
        entity_set.insert(test_entity_string.clone());
        let gql_object_type = Type::NamedType(test_entity_string.clone());
        let field_name = String::from("testField1");
        let result =
            gql_type_to_postgres_relational_type(&field_name, &gql_object_type, &entity_set);
        let expect_output = EntityRelationalTypes {
            is_entity_relationship: true,
            relational_key: field_name,
            mapped_entity: test_entity_string.to_capitalized_options(),
            relationship_type: "object".to_owned(),
        };
        assert_eq!(result, expect_output);
    }

    #[test]
    fn gql_type_to_non_null_relational_type_entity() {
        let mut entity_set = HashSet::new();
        let test_entity_string = String::from("TestEntity");
        entity_set.insert(test_entity_string.clone());
        let gql_object_type =
            Type::NonNullType(Box::new(Type::NamedType(test_entity_string.clone())));
        let field_name = String::from("testField1");
        let result =
            gql_type_to_postgres_relational_type(&field_name, &gql_object_type, &entity_set);
        let expect_output = EntityRelationalTypes {
            is_entity_relationship: true,
            relational_key: field_name,
            mapped_entity: test_entity_string.to_capitalized_options(),
            relationship_type: "object".to_owned(),
        };
        assert_eq!(result, expect_output);
    }

    #[test]
    fn gql_type_to_relational_type_array_entity() {
        let mut entity_set = HashSet::new();
        let test_entity_string = String::from("TestEntity");
        entity_set.insert(test_entity_string.clone());
        let gql_array_object_type =
            Type::ListType(Box::new(Type::NamedType(test_entity_string.clone())));

        let field_name = String::from("testField1");
        let result =
            gql_type_to_postgres_relational_type(&field_name, &gql_array_object_type, &entity_set);
        let expect_output = EntityRelationalTypes {
            is_entity_relationship: true,
            relational_key: field_name,
            mapped_entity: test_entity_string.to_capitalized_options(),
            relationship_type: "array".to_owned(),
        };
        assert_eq!(result, expect_output);
    }
    #[test]
    fn gql_type_to_non_null_relational_type_array_entity() {
        let mut entity_set = HashSet::new();
        let test_entity_string = String::from("TestEntity");
        entity_set.insert(test_entity_string.clone());
        let gql_array_object_type = Type::NonNullType(Box::new(Type::ListType(Box::new(
            Type::NonNullType(Box::new(Type::NamedType(test_entity_string.clone()))),
        ))));

        let field_name = String::from("testField1");
        let result =
            gql_type_to_postgres_relational_type(&field_name, &gql_array_object_type, &entity_set);
        let expect_output = EntityRelationalTypes {
            is_entity_relationship: true,
            relational_key: field_name,
            mapped_entity: test_entity_string.to_capitalized_options(),
            relationship_type: "array".to_owned(),
        };
        assert_eq!(result, expect_output);
    }

    fn convert_entity_type_to_pg_type(entity_type: &str) -> String {
        let schema_string = format!(
            r#"
        type TestEntity @entity {{
          test_field: {}
        }}
        "#,
            entity_type
        );
        let schema = graphql_parser::schema::parse_schema::<String>(&schema_string).unwrap();
        let hash_set = HashSet::new();
        let mut gql_type: Option<Type<String>> = None;

        schema.definitions.iter().for_each(|def| {
            if let Definition::TypeDefinition(type_def) = def {
                if let TypeDefinition::Object(obj_def) = type_def {
                    obj_def.fields.iter().for_each(|field| {
                        gql_type = Some(field.field_type.clone());
                    })
                }
            }
        });
        super::gql_type_to_postgres_type(&gql_type.unwrap(), &hash_set).unwrap()
    }

    #[test]
    fn gql_single_not_null_array_to_pg_type() {
        let gql_type = "[String!]!";
        let pg_type = convert_entity_type_to_pg_type(gql_type);
        assert_eq!(pg_type, "text[] NOT NULL");
    }

    #[test]
    fn gql_multi_not_null_array_to_pg_type() {
        let gql_type = "[[Int!]!]!";
        let pg_type = convert_entity_type_to_pg_type(gql_type);
        assert_eq!(pg_type, "integer[][] NOT NULL");
    }

    #[test]
    #[should_panic]
    fn gql_single_nullable_array_to_pg_type_should_panic() {
        let gql_type = "[Int]!"; //Nested lists need to be not nullable
        convert_entity_type_to_pg_type(gql_type);
    }

    #[test]
    #[should_panic]
    fn gql_multi_nullable_array_to_pg_type_should_panic() {
        let gql_type = "[[Int!]]!"; //Nested lists need to be not nullable
        convert_entity_type_to_pg_type(gql_type);
    }
}
