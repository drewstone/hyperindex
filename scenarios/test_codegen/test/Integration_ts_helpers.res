// import hre from "hardhat";

type hre
@module external hre: hre = "hardhat"
@get @scope("ethers") external getProvider: hre => Ethers.JsonRpcProvider.t = "provider"

@genType.opaque
type chainConfig = Config.chainConfig

@genType
let getLocalChainConfig = (nftFactoryContractAddress): chainConfig => {
  let provider = hre->getProvider

  {
    confirmedBlockThreshold: 200,
    syncSource: Rpc({
      provider,
      syncConfig: {
        initialBlockInterval: 10000,
        backoffMultiplicative: 10000.,
        accelerationAdditive: 10000,
        intervalCeiling: 10000,
        backoffMillis: 10000,
        queryTimeoutMillis: 10000,
      },
    }),
    startBlock: 1,
    endBlock: None,
    chain: MockConfig.chain1337,
    contracts: [
      {
        name: "NftFactory",
        abi: Abis.nftFactoryAbi->Ethers.makeAbi,
        addresses: [nftFactoryContractAddress],
        events: [NftFactory_SimpleNftCreated],
      },
      {
        name: "SimpleNft",
        abi: Abis.simpleNftAbi->Ethers.makeAbi,
        addresses: [],
        events: [SimpleNft_Transfer],
      },
    ],
  }
}

@genType.opaque
type chainManager = ChainManager.t

@genType
let makeChainManager = (cfg: chainConfig): chainManager => {
  // FIXME: Should fork from the main ChainMap?
  ChainManager.makeFromConfig(~config=Config.make(~isUnorderedMultichainMode=true, ~chains=[cfg]))
}

@genType
let startProcessing = (cfg: chainConfig, chainManager: chainManager) => {
  let globalState = GlobalState.make(~chainManager)

  let gsManager = globalState->GlobalStateManager.make

  gsManager->GlobalStateManager.dispatchTask(NextQuery(Chain(cfg.chain)))
}
