use crate::{
    capitalization::Capitalize,
    config_parsing::{ConfigContract, ConfigEvent},
    hbs_templating::codegen_templates::{
        ContractTemplate, EventParamTypeTemplate, EventTemplate, FilteredTemplateLists,
        RequiredEntityEntityFieldTemplate, RequiredEntityTemplate,
    },
    project_paths::{handler_paths::ContractUniqueId, ParsedPaths},
};

use std::{collections::HashMap, error::Error};

use std::path::PathBuf;

use ethers::abi::{
    Contract as AbiContract, Event as EthAbiEvent, EventParam as EthAbiEventParam,
    ParamType as EthAbiParamType,
};

use super::{deserialize_config_from_yaml, EventNameOrSig};

pub fn parse_abi(abi: &str) -> Result<AbiContract, Box<dyn Error>> {
    let abi: AbiContract = serde_json::from_str(abi)?;
    Ok(abi)
}

pub fn get_abi_from_file_path(file_path: &PathBuf) -> Result<AbiContract, Box<dyn Error>> {
    let abi_file = std::fs::read_to_string(file_path)?;
    parse_abi(&abi_file)
}

struct EthereumEventParam<'a> {
    name: &'a str,
    abi_type: &'a EthAbiParamType,
}

impl<'a> EthereumEventParam<'a> {
    fn from_ethereum_abi_param(abi_type: &'a EthAbiEventParam) -> EthereumEventParam<'a> {
        EthereumEventParam {
            name: &abi_type.name,
            abi_type: &abi_type.kind,
        }
    }
}

fn abi_type_to_rescript_string(param: &EthereumEventParam) -> String {
    match &param.abi_type {
        EthAbiParamType::Uint(_size) => String::from("Ethers.BigInt.t"),
        EthAbiParamType::Int(_size) => String::from("Ethers.BigInt.t"),
        EthAbiParamType::Bool => String::from("bool"),
        EthAbiParamType::Address => String::from("Ethers.ethAddress"),
        EthAbiParamType::Bytes => String::from("string"),
        EthAbiParamType::String => String::from("string"),
        EthAbiParamType::FixedBytes(_) => String::from("string"),
        EthAbiParamType::Array(abi_type) => {
            let sub_param = EthereumEventParam {
                abi_type,
                name: param.name,
            };
            format!("array<{}>", abi_type_to_rescript_string(&sub_param))
        }
        EthAbiParamType::FixedArray(abi_type, _) => {
            let sub_param = EthereumEventParam {
                abi_type,
                name: param.name,
            };

            format!("array<{}>", abi_type_to_rescript_string(&sub_param))
        }
        EthAbiParamType::Tuple(abi_types) => {
            let rescript_types: Vec<String> = abi_types
                .iter()
                .map(|abi_type| {
                    let ethereum_param = EthereumEventParam {
                        // Note the name doesn't matter since it's creating tuple without keys
                        //   it is only included so that the type is the same for recursion.
                        name: "",
                        abi_type,
                    };

                    abi_type_to_rescript_string(&ethereum_param)
                })
                .collect();

            let tuple_inner = rescript_types.join(", ");

            let tuple = format!("({})", tuple_inner);
            tuple
        }
    }
}

fn get_event_template_from_ethereum_abi_event(
    config_event: &ConfigEvent,
    abi_event: &EthAbiEvent,
    entity_fields_of_required_entity_map: &HashMap<String, Vec<RequiredEntityEntityFieldTemplate>>,
) -> EventTemplate {
    let name = abi_event.name.to_owned().to_capitalized_options();
    let params = abi_event
        .inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let key = if input.name.is_empty() {
                // In the event that key is empty string
                //   make the key _ + index, eg. _0, _1
                // This will be rescript compatable
                format!("_{}", index)
            } else {
                input.name.to_string()
            };
            EventParamTypeTemplate {
                key,
                type_rescript: abi_type_to_rescript_string(
                    &EthereumEventParam::from_ethereum_abi_param(input),
                ),
            }
        })
        .collect();

    let required_entities = match &config_event.required_entities {
        Some(required_entities_config) => required_entities_config
            .iter()
            .map(|required_entity| {
                let entity_fields_of_required_entity_all = entity_fields_of_required_entity_map
                    .get(&required_entity.name)
                    .cloned()
                    .unwrap_or_default();

                //Template needs access to both the full list and filtered for
                //required entities that are not using a "@derivedFrom" directive
                let entity_fields_of_required_entity =
                    FilteredTemplateLists::new(entity_fields_of_required_entity_all);
                RequiredEntityTemplate {
                    name: required_entity.name.to_capitalized_options(),
                    labels: required_entity.labels.clone(),
                    array_labels: required_entity.array_labels.clone(),
                    entity_fields_of_required_entity,
                }
            })
            .collect(),
        None => Vec::new(),
    };

    EventTemplate {
        name,
        params,
        required_entities,
    }
}

fn get_contract_type_from_config_contract(
    config_contract: &ConfigContract,
    parsed_paths: &ParsedPaths,
    contract_unique_id: ContractUniqueId,
    entity_fields_of_required_entity_map: &HashMap<String, Vec<RequiredEntityEntityFieldTemplate>>,
) -> Result<ContractTemplate, Box<dyn Error>> {
    let mut event_types: Vec<EventTemplate> = Vec::new();

    let contract_abi_opt = parsed_paths.get_contract_abi(&contract_unique_id)?;

    for config_event in config_contract.events.iter() {
        let abi_event = match &config_event.event {
            EventNameOrSig::Name(config_event_name) => contract_abi_opt
                .as_ref()
                .ok_or_else(|| {
                    format!(
                        "Please provide a valid abi_file_path for your named event {} or \
                         alternatively provide a event signature",
                        config_event_name
                    )
                })
                .and_then(|abi| {
                    abi.event(config_event_name).map_err(|_| {
                        format!(
                            "Unable to find an event named {} in your ABI",
                            config_event_name
                        )
                    })
                })?,
            EventNameOrSig::Event(config_defined_event) => {
                if let Some(contract_abi) = &contract_abi_opt {
                    match contract_abi.event(&config_defined_event.name) {
                        Err(_) => {
                            eprintln!(
                                "WARNING: The event signature for {} defined in your config \
                                 does not exist in the provided ABI file",
                                config_defined_event.name
                            );
                        }
                        Ok(abi_file_event) => {
                            if abi_file_event != config_defined_event {
                                eprintln!(
                                    "WARNING: The event signature for {} in your ABI file \
                                     does not match the signature defined in the config",
                                    config_defined_event.name
                                );
                            }
                        }
                    }
                }
                config_defined_event
            }
        };

        let event_type = get_event_template_from_ethereum_abi_event(
            config_event,
            abi_event,
            entity_fields_of_required_entity_map,
        );
        event_types.push(event_type);
    }

    let handler_template = parsed_paths.get_contract_handler_paths_template(&contract_unique_id)?;

    let contract = ContractTemplate {
        name: config_contract.name.to_capitalized_options(),
        events: event_types,
        handler: handler_template,
    };

    Ok(contract)
}

pub fn get_contract_types_from_config(
    parsed_paths: &ParsedPaths,
    entity_fields_of_required_entity_map: &HashMap<String, Vec<RequiredEntityEntityFieldTemplate>>,
) -> Result<Vec<ContractTemplate>, Box<dyn Error>> {
    let config = deserialize_config_from_yaml(&parsed_paths.project_paths.config)?;
    let mut contracts: Vec<ContractTemplate> = Vec::new();
    for network in config.networks.iter() {
        for config_contract in network.contracts.iter() {
            let contract_unique_id = ContractUniqueId {
                network_id: network.id,
                name: config_contract.name.clone(),
            };

            let contract = get_contract_type_from_config_contract(
                config_contract,
                parsed_paths,
                contract_unique_id,
                entity_fields_of_required_entity_map,
            )?;
            contracts.push(contract);
        }
    }
    Ok(contracts)
}

#[cfg(test)]
mod tests {
    use crate::{
        capitalization::Capitalize,
        config_parsing::{self, EventNameOrSig, RequiredEntity},
        hbs_templating::codegen_templates::{
            EventParamTypeTemplate, EventTemplate, FilteredTemplateLists, RequiredEntityTemplate,
        },
    };
    use ethers::abi::{Event as AbiEvent, EventParam, ParamType};
    use std::collections::HashMap;

    use super::{abi_type_to_rescript_string, get_event_template_from_ethereum_abi_event};

    #[test]
    fn abi_event_to_record_1() {
        let input1_name = String::from("id");

        let input1 = EventParam {
            name: input1_name.clone(),
            indexed: false,
            kind: ParamType::Uint(256),
        };

        let input2_name = String::from("owner");
        let input2 = EventParam {
            name: input2_name.clone(),
            indexed: false,
            kind: ParamType::Address,
        };

        let inputs = vec![input1, input2];
        let event_name = String::from("NewGravatar");

        let abi_event: ethers::abi::Event = AbiEvent {
            name: event_name.clone(),
            anonymous: false,
            inputs,
        };
        let config_event = config_parsing::ConfigEvent {
            event: EventNameOrSig::Name(event_name.clone()),
            required_entities: None,
        };

        let parsed_event_template =
            get_event_template_from_ethereum_abi_event(&config_event, &abi_event, &HashMap::new());

        let expected_event_template = EventTemplate {
            name: event_name.to_capitalized_options(),
            params: vec![
                EventParamTypeTemplate {
                    key: input1_name,
                    type_rescript: String::from("Ethers.BigInt.t"),
                },
                EventParamTypeTemplate {
                    key: input2_name,
                    type_rescript: String::from("Ethers.ethAddress"),
                },
            ],
            required_entities: vec![],
        };
        assert_eq!(parsed_event_template, expected_event_template)
    }

    #[test]
    fn abi_event_to_record_2() {
        let input1_name = String::from("id");

        let input1 = EventParam {
            name: input1_name.clone(),
            indexed: false,
            kind: ParamType::Uint(256),
        };

        let input2_name = String::from("owner");
        let input2 = EventParam {
            name: input2_name.clone(),
            indexed: false,
            kind: ParamType::Address,
        };

        let inputs = vec![input1, input2];
        let event_name = String::from("NewGravatar");

        let abi_event = AbiEvent {
            name: event_name.clone(),
            anonymous: false,
            inputs,
        };
        let config_event = config_parsing::ConfigEvent {
            event: EventNameOrSig::Name(event_name.clone()),
            required_entities: Some(vec![RequiredEntity {
                name: String::from("Gravatar"),
                labels: Some(vec![String::from("gravatarWithChanges")]),
                array_labels: None,
            }]),
        };

        let parsed_event_template =
            get_event_template_from_ethereum_abi_event(&config_event, &abi_event, &HashMap::new());

        let expected_event_template = EventTemplate {
            name: event_name.to_capitalized_options(),
            params: vec![
                EventParamTypeTemplate {
                    key: input1_name,
                    type_rescript: String::from("Ethers.BigInt.t"),
                },
                EventParamTypeTemplate {
                    key: input2_name,
                    type_rescript: String::from("Ethers.ethAddress"),
                },
            ],
            required_entities: vec![RequiredEntityTemplate {
                name: String::from("Gravatar").to_capitalized_options(),
                labels: Some(vec![String::from("gravatarWithChanges")]),
                array_labels: None,
                entity_fields_of_required_entity: FilteredTemplateLists::empty(),
            }],
        };
        assert_eq!(parsed_event_template, expected_event_template)
    }

    #[test]
    fn test_record_type_array() {
        let array_string_type = ParamType::Array(Box::new(ParamType::String));
        let param = super::EthereumEventParam {
            abi_type: &array_string_type,
            name: "myArray",
        };

        let parsed_rescript_string = abi_type_to_rescript_string(&param);

        assert_eq!(parsed_rescript_string, String::from("array<string>"))
    }

    #[test]
    fn test_record_type_fixed_array() {
        let array_fixed_arr_type = ParamType::FixedArray(Box::new(ParamType::String), 1);
        let param = super::EthereumEventParam {
            abi_type: &array_fixed_arr_type,
            name: "myArrayFixed",
        };
        let parsed_rescript_string = abi_type_to_rescript_string(&param);

        assert_eq!(parsed_rescript_string, String::from("array<string>"))
    }

    #[test]
    fn test_record_type_tuple() {
        let tuple_type = ParamType::Tuple(vec![ParamType::String, ParamType::Uint(256)]);
        let param = super::EthereumEventParam {
            abi_type: &tuple_type,
            name: "myArrayFixed",
        };

        let parsed_rescript_string = abi_type_to_rescript_string(&param);

        assert_eq!(
            parsed_rescript_string,
            String::from("(string, Ethers.BigInt.t)")
        )
    }
}