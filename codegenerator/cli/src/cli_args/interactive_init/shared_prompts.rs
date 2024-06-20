use std::fmt::Display;

use crate::init_config::{evm, Ecosystem};

use super::{
    clap_definitions::{self, InitFlow},
    fuel_prompts,
    inquire_helpers::FilePathCompleter,
    validation::{
        contains_no_whitespace_validator, first_char_is_alphabet_validator,
        is_only_alpha_numeric_characters_validator, UniqueValueValidator,
    },
};

use anyhow::{Context, Result};
use inquire::{validator::Validation, CustomType, MultiSelect, Select, Text};

use std::str::FromStr;
use strum::{Display, EnumIter, IntoEnumIterator};

#[derive(Clone, Debug, Display, PartialEq, EnumIter)]
pub enum EcosystemOption {
    Evm,
    Fuel,
}

pub fn prompt_template<T: Display>(options: Vec<T>) -> Result<T> {
    Select::new("Which template would you like to use?", options)
        .prompt()
        .context("Prompting user for template selection")
}

pub struct SelectItem<T> {
    pub item: T,
    pub display: String,
}

impl<T> Display for SelectItem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

///Takes a vec of Events and sets up a multi selecet prompt
///with all selected by default. Whatever is selected in the prompt
///is returned
pub fn prompt_events_selection<T>(events: Vec<SelectItem<T>>) -> Result<Vec<T>> {
    //Collect all the indexes of the vector in another vector which will be used
    //to preselect all events
    let all_indexes_of_events = events
        .iter()
        .enumerate()
        .map(|(i, _)| i)
        .collect::<Vec<usize>>();

    //Prompt for selection with all events selected by default
    let selected_wrapped_events = MultiSelect::new("Which events would you like to index?", events)
        .with_default(&all_indexes_of_events)
        .prompt()?;

    //Unwrap the selected events and return
    let selected_events = selected_wrapped_events
        .into_iter()
        .map(|w_event| w_event.item)
        .collect();

    Ok(selected_events)
}

pub fn prompt_abi_file_path(
    abi_validator: fn(abi_file_path: &str) -> Validation,
) -> Result<String> {
    Text::new("What is the path to your json abi file?")
        //Auto completes path for user with tab/selection
        .with_autocomplete(FilePathCompleter::default())
        //Tries to parse the abi to ensure its valid and doesn't
        //crash the prompt if not. Simply asks for a valid abi
        .with_validator(move |path: &str| Ok(abi_validator(path)))
        .prompt()
        .context("Failed during prompt for abi file path")
}

pub fn prompt_contract_name() -> Result<String> {
    Text::new("What is the name of this contract?")
        .with_validator(contains_no_whitespace_validator)
        .with_validator(is_only_alpha_numeric_characters_validator)
        .with_validator(first_char_is_alphabet_validator)
        .prompt()
        .context("Failed during contract name prompt")
}

pub fn prompt_contract_address<T: Clone + FromStr + Display + PartialEq + 'static>(
    selected: Option<&Vec<T>>,
) -> Result<T> {
    let mut prompter = CustomType::<T>::new("What is the address of the contract?")
        .with_help_message("Use the proxy address if your abi is a proxy implementation")
        .with_error_message(
            "Please input a valid contract address (should be a hexadecimal starting with (0x))",
        );
    if let Some(selected) = selected {
        prompter = prompter.with_validator(UniqueValueValidator::new(selected.clone()))
    }
    prompter
        .prompt()
        .context("Failed during contract address prompt")
}

pub async fn prompt_ecosystem(cli_init_flow: Option<InitFlow>) -> Result<Ecosystem> {
    let init_flow = match cli_init_flow {
        Some(v) => v,
        None => {
            let ecosystem_options = EcosystemOption::iter().collect();

            let ecosystem_option = Select::new("Choose blockchain ecosystem", ecosystem_options)
                .prompt()
                .context("Failed prompting for blockchain ecosystem")?;

            match ecosystem_option {
                EcosystemOption::Fuel => InitFlow::Fuel { init_flow: None },
                EcosystemOption::Evm => {
                    // Start prompt to ask the user which initialization option they want
                    // Explicitelly build options, since we don't want to include graph migration and other ecosystem selection subcomands
                    let user_response_options =
                        clap_definitions::EvmInitFlowInteractive::iter().collect();

                    Select::new("Choose an initialization option", user_response_options)
                        .prompt()
                        .context("Failed prompting for Evm initialization option")?
                        .into()
                }
            }
        }
    };

    let initialization = match init_flow {
        InitFlow::Fuel {
            init_flow: maybe_init_flow,
        } => match fuel_prompts::prompt_init_flow_missing(maybe_init_flow)? {
            clap_definitions::fuel::InitFlow::Template(args) => Ecosystem::Fuel {
                init_flow: fuel_prompts::prompt_template_init_flow(args)?,
            },
            clap_definitions::fuel::InitFlow::ContractImport(args) => Ecosystem::Fuel {
                init_flow: fuel_prompts::prompt_contract_import_init_flow(args).await?,
            },
        },
        InitFlow::Template(args) => {
            let chosen_template = match args.template {
                Some(template) => template,
                None => {
                    let options = evm::Template::iter().collect();
                    prompt_template(options)?
                }
            };
            Ecosystem::Evm {
                init_flow: evm::InitFlow::Template(chosen_template),
            }
        }
        InitFlow::SubgraphMigration(args) => {
            let input_subgraph_id = match args.subgraph_id {
                Some(id) => id,
                None => Text::new("[BETA VERSION] What is the subgraph ID?")
                    .prompt()
                    .context("Prompting user for subgraph id")?,
            };
            Ecosystem::Evm {
                init_flow: evm::InitFlow::SubgraphID(input_subgraph_id),
            }
        }

        InitFlow::ContractImport(args) => {
            let auto_config_selection = args
                .get_auto_config_selection()
                .await
                .context("Failed getting AutoConfigSelection selection")?;
            Ecosystem::Evm {
                init_flow: evm::InitFlow::ContractImport(auto_config_selection),
            }
        }
    };

    Ok(initialization)
}
