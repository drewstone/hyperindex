use super::{
    clap_definitions::{
        ContractImportArgs, ExplorerImportArgs, LocalImportArgs, LocalOrExplorerImport,
    },
    inquire_helpers::FilePathCompleter,
    validation::{
        contains_no_whitespace_validator, first_char_is_alphabet_validator, is_abi_file_validator,
        is_only_alpha_numeric_characters_validator, UniqueValueValidator,
    },
};
use crate::{
    cli_args::interactive_init::validation::filter_duplicate_events,
    config_parsing::{
        chain_helpers::{Network, NetworkWithExplorer, SupportedNetwork},
        contract_import::converters::{
            self, ContractImportNetworkSelection, ContractImportSelection,
        },
        human_config::parse_contract_abi,
    },
    utils::address_type::Address,
};
use anyhow::{anyhow, Context, Result};
use inquire::{Confirm, CustomType, Select, Text};
use std::{path::PathBuf, str::FromStr};
use strum::IntoEnumIterator;

///Returns the prompter which can call .prompt() to action, or add validators/other
///properties
fn contract_address_prompter() -> CustomType<'static, Address> {
    CustomType::<Address>::new("What is the address of the contract? (Use the proxy address if your abi is a proxy implementation)")
                        .with_error_message("Please input a valid contract address (should be a hexadecimal starting with (0x))")
}

///Immediately calls the prompter
fn contract_address_prompt() -> Result<Address> {
    contract_address_prompter()
        .prompt()
        .context("Prompting user for contract address")
}

impl ContractImportNetworkSelection {
    ///Recursively asks to add an address to ContractImportNetworkSelection
    fn prompt_add_contract_address_to_network_selection(
        &mut self,
        contract_name: &str,
    ) -> Result<()> {
        let question = format!(
            "Would you like to add a new address for contract {} on network {}? (y/n)",
            contract_name, self.network
        );

        if Confirm::new(&question).prompt()? {
            let address = contract_address_prompter()
                .with_validator(UniqueValueValidator::new(self.addresses.clone()))
                .prompt()
                .context("Failed prompting user for new address")?;
            self.add_address(address);

            self.prompt_add_contract_address_to_network_selection(contract_name)
        } else {
            Ok(())
        }
    }
}

impl ContractImportSelection {
    //Recursively asks to add networks with addresses to ContractImportNetworkSelection
    pub fn prompt_add_network_to_contract_import_selection(&mut self) -> Result<()> {
        let question = format!(
            "Would you like to add a new network for contract {}? (y/n)",
            self.name
        );

        //Confirm if a user would like to add a new network for the given contract
        if Confirm::new(&question).prompt()? {
            //In a new network case, no RPC url could be
            //derived from CLI flags
            const NO_RPC_URL: Option<String> = None;

            //Select a new network (not from the list of existing network ids already added)
            let selected_network = prompt_for_network_id(&NO_RPC_URL, self.get_network_ids())
                .context("Failed selecting network")?;

            //Instantiate a network_selection without any  contract addresses
            let mut network_selection =
                ContractImportNetworkSelection::new_without_addresses(selected_network);
            //Populate contract addresses with prompt
            network_selection
                .prompt_add_contract_address_to_network_selection(&self.name)
                .context("Failed adding new contract address")?;

            //Add the network to the contract selection
            self.add_network(network_selection);

            //Reprompt to add more or exit
            self.prompt_add_network_to_contract_import_selection()
        } else {
            //Exit if the user does not want to add more networks
            Ok(())
        }
    }
}

impl ContractImportArgs {
    pub async fn get_contract_import_selection(&self) -> Result<ContractImportSelection> {
        match &self.get_local_or_explorer()? {
            LocalOrExplorerImport::Explorer(explorer_import_args) => {
                self.get_contract_import_selection_from_explore_import_args(explorer_import_args)
                    .await
            }
            LocalOrExplorerImport::Local(local_import_args) => {
                self.get_contract_import_selection_from_local_import_args(local_import_args)
                    .await
            }
        }
    }

    async fn get_contract_import_selection_from_local_import_args(
        &self,
        local_import_args: &LocalImportArgs,
    ) -> Result<ContractImportSelection> {
        let parsed_abi = local_import_args
            .get_parsed_abi()
            .context("Failed getting parsed abi")?;

        let network = local_import_args
            .get_network()
            .context("Failed getting chosen network")?;

        let contract_name = local_import_args
            .get_contract_name()
            .context("Failed getting contract name")?;

        let address = self
            .get_contract_address()
            .context("Failed getting contract address")?;

        let mut network_selection = ContractImportNetworkSelection::new(network, address);

        //If the flag for --single-contract was not added, continue to prompt for adding
        //addresses to the given network for this contract
        if !self.single_contract {
            network_selection
                .prompt_add_contract_address_to_network_selection(&contract_name)
                .context("Failed prompting for more contract addresses on network")?;
        }

        let contract_selection =
            ContractImportSelection::from_abi(network_selection, contract_name, parsed_abi);

        Ok(contract_selection)
    }

    async fn get_contract_import_selection_from_explore_import_args(
        &self,
        explorer_import_args: &ExplorerImportArgs,
    ) -> Result<ContractImportSelection> {
        let network_with_explorer = explorer_import_args
            .get_network_with_explorer()
            .context("Failed getting NetworkWithExporer")?;

        let chosen_contract_address = self
            .get_contract_address()
            .context("Failed getting contract address")?;

        let mut contract_selection = ContractImportSelection::from_etherscan(
            &network_with_explorer,
            chosen_contract_address,
        )
        .await
        .context("Failed getting ContractImportSelection from explorer")?;

        let last_network_selection = contract_selection.networks.last_mut().ok_or_else(|| {
            anyhow!("Expected a network seletion to be constructed with ContractImportSelection")
        })?;

        if !self.single_contract {
            //If the flag for --single-contract was not added, continue to prompt for adding
            //addresses to the given network for this contract
            last_network_selection
                .prompt_add_contract_address_to_network_selection(&contract_selection.name)?;
        }

        Ok(contract_selection)
    }

    ///Takes either the address passed in by cli flag or prompts
    ///for an address
    fn get_contract_address(&self) -> Result<Address> {
        match &self.contract_address {
            Some(c) => Ok(c.clone()),
            None => contract_address_prompt(),
        }
    }

    ///Takes either the "local" or "explorer" subcommand from the cli args
    ///or prompts for a choice from the user
    fn get_local_or_explorer(&self) -> Result<LocalOrExplorerImport> {
        match &self.local_or_explorer {
            Some(v) => Ok(v.clone()),
            None => {
                let options = LocalOrExplorerImport::iter().collect();

                Select::new(
                    "Would you like to import from a block explorer or a local abi?",
                    options,
                )
                .prompt()
                .context("Failed prompting for import from block explorer or local abi")
            }
        }
    }
}

///Prompts for a Supported network or for the user to enter an
///id, if it is unsupported it requires an RPC url. If the rpc is already
///known it can be passed in as the first arg. Otherwise this will be prompted.
///It also checks that the network does not belong to a given list of network ids
///To validate that a user is not double selecting a network id
fn prompt_for_network_id(
    opt_rpc_url: &Option<String>,
    already_selected_ids: Vec<u64>,
) -> Result<converters::Network> {
    //The first option of the list, funnels the user to enter a u64
    let enter_id = "<Enter Network Id>";

    //Select one of our supported networks
    let networks = SupportedNetwork::iter()
        //Don't allow selection of networks that have been previously
        //selected.
        .filter(|n| {
            let network_id = *n as u64;
            !already_selected_ids.contains(&network_id)
        })
        .map(|n| n.to_string())
        .collect::<Vec<_>>();

    //User's options to either enter an id or select a supported network
    let options = vec![vec![enter_id.to_string()], networks].concat();

    //Action prompt
    let choose_from_networks = Select::new("Choose network:", options)
        .prompt()
        .context("Failed during prompt for abi file path")?;

    let selected = match choose_from_networks.as_str() {
        //If the user's choice evaluates to the enter network id option, prompt them for
        //a network id
        choice if choice == enter_id => {
            let network_id = CustomType::<u64>::new("Enter the network id:")
                //Validate that this ID is not already selected
                .with_validator(UniqueValueValidator::new(already_selected_ids))
                .with_error_message("Invalid network id input, please enter a number")
                .prompt()?;

            //Convert the id into a supported or unsupported network.
            //If unsupported, it will use the optional rpc url or prompt
            //for an rpc url
            get_converter_network_u64(network_id, opt_rpc_url)?
        }
        //If a supported network choice was selected. We should be able to
        //parse it back to a supported network since it was serialized as a
        //string
        choice => converters::Network::Supported(
            SupportedNetwork::from_str(&choice)
                .context("Unexpected input, not a supported network.")?,
        ),
    };

    Ok(selected)
}

//Takes a u64 network ID and turns it into either "Supported" network or
//"Unsupported" where we need an RPC url. If the RPC url is known, pass it
//in as the 2nd arg otherwise prompt for an rpc url
fn get_converter_network_u64(
    network_id: u64,
    rpc_url: &Option<String>,
) -> Result<converters::Network> {
    let maybe_supported_network =
        Network::from_network_id(network_id).and_then(|n| Ok(SupportedNetwork::try_from(n)?));

    let network = match maybe_supported_network {
        Ok(s) => converters::Network::Supported(s),
        Err(_) => {
            let rpc_url = match rpc_url {
                Some(r) => r.clone(),
                None => prompt_for_rpc_url()?,
            };
            converters::Network::Unsupported(network_id, rpc_url)
        }
    };

    Ok(network)
}

///Prompt the user to enter an rpc url
fn prompt_for_rpc_url() -> Result<String> {
    Text::new(
        "You have entered a network that is unsupported by our servers. \
                        Please provide an rpc url (this can be edited later in config.yaml):",
    )
    .prompt()
    .context("Failed during rpc url prompt")
}

impl ExplorerImportArgs {
    ///Either take the NetworkWithExplorer value from the cli args or prompt
    ///for a user to select one.
    fn get_network_with_explorer(&self) -> Result<NetworkWithExplorer> {
        let chosen_network = match &self.blockchain {
            Some(chain) => chain.clone(),
            None => {
                let options = NetworkWithExplorer::iter()
                    //Filter only our supported networks
                    .filter(|&n| {
                        SupportedNetwork::iter()
                            //able to cast as u64 because networks enum
                            //uses repr(u64) attribute
                            .find(|&sn| n as u64 == sn as u64)
                            .is_some()
                    })
                    .collect();

                Select::new(
                    "Which blockchain would you like to import a contract from?",
                    options,
                )
                .prompt()?
            }
        };

        Ok(chosen_network)
    }
}

impl LocalImportArgs {
    ///Internal function to get the abi path from the cli args or prompt for
    ///a file path to the abi
    fn get_abi_path_string(&self) -> Result<String> {
        match &self.abi_file {
            Some(p) => Ok(p.to_owned()),
            None => {
                let abi_path = Text::new("What is the path to your json abi file?")
                    //Auto completes path for user with tab/selection
                    .with_autocomplete(FilePathCompleter::default())
                    //Tries to parse the abi to ensure its valid and doesn't
                    //crash the prompt if not. Simply asks for a valid abi
                    .with_validator(is_abi_file_validator)
                    .prompt()
                    .context("Failed during prompt for abi file path")?;

                Ok(abi_path)
            }
        }
    }

    ///Get the file path for the abi and parse it into an abi
    fn get_parsed_abi(&self) -> Result<ethers::abi::Abi> {
        let abi_path_string = self.get_abi_path_string()?;

        let mut parsed_abi =
            parse_contract_abi(PathBuf::from(abi_path_string)).context("Failed to parse abi")?;

        parsed_abi.events = filter_duplicate_events(parsed_abi.events);

        Ok(parsed_abi)
    }

    ///Gets the network from from cli args or prompts for
    ///a network
    fn get_network(&self) -> Result<converters::Network> {
        match &self.blockchain {
            Some(b) => {
                let network_id: u64 = (b.clone()).into();
                get_converter_network_u64(network_id, &self.rpc_url)
            }
            None => prompt_for_network_id(&self.rpc_url, vec![]),
        }
    }

    ///Prompts for a contract name
    fn get_contract_name(&self) -> Result<String> {
        match &self.contract_name {
            Some(n) => Ok(n.clone()),
            None => Text::new("What is the name of this contract?")
                .with_validator(contains_no_whitespace_validator)
                .with_validator(is_only_alpha_numeric_characters_validator)
                .with_validator(first_char_is_alphabet_validator)
                .prompt()
                .context("Failed during contract name prompt"),
        }
    }
}