use crate::{
    cli_args::clap_definitions::Language,
    config_parsing::{
        chain_helpers::{NetworkWithExplorer, SupportedNetwork},
        human_config::{
            self, ConfigEvent, EventNameOrSig, GlobalContractConfig, HumanConfig,
            LocalContractConfig, RequiredEntity, RpcConfig, SyncSourceConfig,
        },
    },
    utils::{address_type::Address, unique_hashmap},
};
use anyhow::{Context, Result};
use itertools::{self, Itertools};
use std::collections::HashMap;

use super::etherscan_helpers::fetch_contract_auto_selection_from_etherscan;

///A an object that holds all the values a user can select during
///the auto config generation. Values can come from etherscan or
///abis etc.
#[derive(Clone)]
pub struct AutoConfigSelection {
    project_name: String,
    selected_contracts: Vec<ContractImportSelection>,
    language: Language,
}

impl AutoConfigSelection {
    pub fn new(
        project_name: String,
        language: Language,
        selected_contract: ContractImportSelection,
    ) -> Self {
        Self {
            project_name,
            language,
            selected_contracts: vec![selected_contract],
        }
    }

    pub async fn from_etherscan(
        project_name: String,
        language: Language,
        network: &NetworkWithExplorer,
        address: Address,
    ) -> Result<Self> {
        let selected_contract = fetch_contract_auto_selection_from_etherscan(address, network)
            .await
            .context("Failed fetching selected contract")?;

        Ok(Self::new(project_name, language, selected_contract))
    }

    pub fn from_abi(
        project_name: String,
        language: Language,
        network: Network,
        address: Address,
        contract_name: String,
        abi: ethers::abi::Contract,
    ) -> Self {
        let selected_contract =
            ContractImportSelection::from_abi(network, address, contract_name, abi);

        Self::new(project_name, language, selected_contract)
    }
}

///The hierarchy is based on how you would add items to
///your selection as you go. Ie. Once you have constructed
///the selection of a contract you can add more addresses or
///networks
#[derive(Clone)]
pub struct ContractImportSelection {
    name: String,
    networks: Vec<ContractImportNetworkSelection>,
    events: Vec<ethers::abi::Event>,
}

impl ContractImportSelection {
    pub fn new(
        name: String,
        network_selection: ContractImportNetworkSelection,
        events: Vec<ethers::abi::Event>,
    ) -> Self {
        Self {
            name,
            networks: vec![network_selection],
            events,
        }
    }

    pub fn from_abi(
        network: Network,
        address: Address,
        contract_name: String,
        abi: ethers::abi::Contract,
    ) -> Self {
        let network_selection = ContractImportNetworkSelection::new(network, address);
        let events = abi.events().cloned().collect();
        Self::new(contract_name, network_selection, events)
    }

    pub fn add_network(&mut self, network_selection: ContractImportNetworkSelection) {
        self.networks.push(network_selection)
    }

    pub async fn from_etherscan(network: &NetworkWithExplorer, address: Address) -> Result<Self> {
        fetch_contract_auto_selection_from_etherscan(address, network).await
    }
}

type NetworkId = u64;
type RpcUrl = String;

#[derive(Clone)]
pub enum Network {
    Supported(SupportedNetwork),
    Unsupported(NetworkId, RpcUrl),
}

impl Network {
    fn get_network_id(&self) -> NetworkId {
        match self {
            Network::Supported(n) => n.clone() as u64,
            Network::Unsupported(n, _) => *n,
        }
    }
}

#[derive(Clone)]
pub struct ContractImportNetworkSelection {
    network: Network,
    addresses: Vec<Address>,
}

impl ContractImportNetworkSelection {
    pub fn new(network: Network, address: Address) -> Self {
        Self {
            network,
            addresses: vec![address],
        }
    }

    pub fn add_address(&mut self, address: Address) {
        self.addresses.push(address)
    }
}

///Converts the selection object into a human config
type ContractName = String;
impl TryFrom<AutoConfigSelection> for HumanConfig {
    type Error = anyhow::Error;
    fn try_from(selection: AutoConfigSelection) -> Result<Self, Self::Error> {
        let mut networks_map: HashMap<u64, human_config::Network> = HashMap::new();
        let mut global_contracts: HashMap<ContractName, GlobalContractConfig> = HashMap::new();

        for selected_contract in selection.selected_contracts {
            let is_multi_chain_contract = selected_contract.networks.len() > 1;

            let events: Vec<ConfigEvent> = selected_contract
                .events
                .into_iter()
                .map(|event| human_config::ConfigEvent {
                    event: EventNameOrSig::Event(event.clone()),
                    required_entities: Some(vec![RequiredEntity {
                        //Required entity needed for autogen schema
                        name: "EventsSummary".to_string(),
                        labels: None,
                        array_labels: None,
                    }]),
                })
                .collect();

            let handler = get_event_handler_directory(&selection.language);

            let local_contract_config = if is_multi_chain_contract {
                //Add the contract to global contract config and return none for local contract
                //config
                let global_contract = GlobalContractConfig {
                    name: selected_contract.name.clone(),
                    abi_file_path: None,
                    handler,
                    events,
                };

                unique_hashmap::try_insert(
                    &mut global_contracts,
                    selected_contract.name.clone(),
                    global_contract,
                )
                .context(format!(
                    "Unexpected, failed to add global contract {}. Contract should have unique names",
                    selected_contract.name
                ))?;
                None
            } else {
                //Return some for local contract config
                Some(LocalContractConfig {
                    abi_file_path: None,
                    handler,
                    events,
                })
            };

            for selected_network in &selected_contract.networks {
                let address = selected_network
                    .addresses
                    .iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .into();

                let network = networks_map
                    .entry(selected_network.network.get_network_id())
                    .or_insert({
                        let sync_source = match &selected_network.network {
                            Network::Supported(_) => None,
                            Network::Unsupported(_, url) => {
                                Some(SyncSourceConfig::RpcConfig(RpcConfig {
                                    url: url.clone(),
                                    unstable__sync_config: None,
                                }))
                            }
                        };

                        human_config::Network {
                            id: selected_network.network.get_network_id(),
                            sync_source,
                            start_block: 0,
                            contracts: Vec::new(),
                        }
                    });

                let contract = human_config::NetworkContractConfig {
                    name: selected_contract.name.clone(),
                    address,
                    local_contract_config: local_contract_config.clone(),
                };

                network.contracts.push(contract);
            }
        }

        let contracts = match global_contracts
            .into_values()
            .sorted_by_key(|v| v.name.clone())
            .collect::<Vec<_>>()
        {
            values if values.is_empty() => None,
            values => Some(values),
        };

        let networks = networks_map.into_values().sorted_by_key(|v| v.id).collect();

        Ok(HumanConfig {
            name: selection.project_name,
            description: None,
            schema: None,
            contracts,
            networks,
        })
    }
}

// Logic to get the event handler directory based on the language
fn get_event_handler_directory(language: &Language) -> String {
    match language {
        Language::Rescript => "./src/EventHandlers.bs.js".to_string(),
        Language::Typescript => "src/EventHandlers.ts".to_string(),
        Language::Javascript => "./src/EventHandlers.js".to_string(),
    }
}