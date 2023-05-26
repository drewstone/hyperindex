exception QueryTimout(string)

// After an RPC error, how much to scale back the number of blocks requested at once
let backoffMultiplicative = 0.8

// Without RPC errors or timeouts, how much to increase the number of blocks requested by for the next batch
let accelerationAdditive = 20

// After an error, how long to wait before retrying
let backoffMillis = 5000

let queryTimeoutMillis = 20000

// Expose key removal on JS maps, used for cache invalidation
@val external delete: ('a, string) => unit = "delete"

let convertLogs = (
  logsPromise: Promise.t<array<Ethers.log>>,
  ~provider,
  ~addressInterfaceMapping,
  ~fromBlockForLogging,
  ~toBlockForLogging,
  ~chainId,
) => {
  let blockRequestMapping: Js.Dict.t<
    Promise.t<Js.Nullable.t<Ethers.JsonRpcProvider.block>>,
  > = Js.Dict.empty()

  // Many times logs will be from the same block so there is no need to make multiple get block requests in that case
  let getMemoisedBlockPromise = blockNumber => {
    let blockKey = Belt.Int.toString(blockNumber)

    let blockRequestCached = blockRequestMapping->Js.Dict.get(blockKey)

    let blockRequest = switch blockRequestCached {
    | Some(req) => req
    | None =>
      let newRequest = provider->Ethers.JsonRpcProvider.getBlock(blockNumber)
      // Cache the request
      blockRequestMapping->Js.Dict.set(blockKey, newRequest)
      newRequest
    }
    blockRequest
    ->Promise.catch(err => {
      // Invalidate the cache, so that the request can be retried
      delete(blockRequestMapping, blockKey)

      // Propagate failure to where we handle backoff
      Promise.reject(err)
    })
    ->Promise.then(block =>
      switch block->Js.Nullable.toOption {
      | Some(block) => Promise.resolve(block)
      | None =>
        Promise.reject(
          Js.Exn.raiseError(`getBLock(${blockKey}) returned null`),
        )
      }
  }

  let task = async () => {
    let logs = await logsPromise


    Js.log2("Handling number of logs: ", logs->Array.length)

    let events =
      await logs
      ->Belt.Array.map(log => {
        let blockPromise = log.blockNumber->getMemoisedBlockPromise

        //get a specific interface type
        //interface type parses the log
        let optInterface = addressInterfaceMapping->Js.Dict.get(log.address->Obj.magic)

        switch optInterface {
        | None => None
        | Some(interface) => {
            let logDescription = interface->Ethers.Interface.parseLog(~log)

            switch Converters.eventStringToEvent(logDescription.name, Converters.getContractNameFromAddress(log.address, chainId)) {
{{#each contracts as |contract|}}
{{#each contract.events as |event|}}
            | {{contract.name.capitalized}}Contract_{{event.name.capitalized}}Event =>
              let convertedEvent =
                logDescription
                ->Converters.{{contract.name.capitalized}}.convert{{event.name.capitalized}}LogDescription
                ->Converters.{{contract.name.capitalized}}.convert{{event.name.capitalized}}Log(~log, ~blockPromise)

              Some(convertedEvent)
{{/each}}
{{/each}}
            }
          }
        }
      })
      ->Belt.Array.keepMap(opt => opt)
      ->Promise.all

    events
  }

  Time.retryOnCatchAfterDelay(
    ~retryDelayMilliseconds=backoffMillis,
    ~retryMessage=`Failed to handle event logs from block ${fromBlockForLogging->Belt.Int.toString} to block ${toBlockForLogging->Belt.Int.toString}`,
    ~task,
  )
}

let makeCombinedEventFilterQuery = (~provider, ~eventFilters, ~fromBlock, ~toBlock) => {
  open Ethers.BlockTag

  let combinedFilter =
    eventFilters->Ethers.CombinedFilter.combineEventFilters(
      ~fromBlock=BlockNumber(fromBlock)->blockTagFromVariant,
      ~toBlock=BlockNumber(toBlock)->blockTagFromVariant,
    )

  Js.log3("Intiating Combined Query Filter fromBlock toBlock: ", fromBlock, toBlock)

  let task = () =>
    provider
    ->Ethers.JsonRpcProvider.getLogs(
      ~filter={combinedFilter->Ethers.CombinedFilter.combinedFilterToFilter},
    )
    ->Promise.thenResolve(res => {
      Js.log3("Successful Combined Query Filter fromBlock toBlock: ", fromBlock, toBlock)
      res
    })

  Time.retryOnCatchAfterDelay(
    ~retryDelayMilliseconds=5000,
    ~retryMessage=`Failed combined query filter from block ${fromBlock->Belt.Int.toString} to block ${toBlock->Belt.Int.toString}`,
    ~task,
  )
}

let queryEventsWithCombinedFilterAndExecuteHandlers = async (
  ~addressInterfaceMapping,
  ~eventFilters,
  ~fromBlock,
  ~toBlock,
  ~provider,
  ~chainId,
) => {
  let combinedFilter = makeCombinedEventFilterQuery(~provider, ~eventFilters, ~fromBlock, ~toBlock)
  let events =
    await combinedFilter->convertLogs(
      ~provider,
      ~addressInterfaceMapping,
      ~fromBlockForLogging=fromBlock,
      ~toBlockForLogging=toBlock,
      ~chainId,
    )

  events->EventProcessing.processEventBatch(~chainId)
}

let getAllEventFilters = (
  ~addressInterfaceMapping,
  ~chainConfig: Config.chainConfig,
  ~provider,
) => {
  let eventFilters = []

  chainConfig.contracts->Belt.Array.forEach(contract => {
    let contractEthers = Ethers.Contract.make(
      ~address=contract.address,
      ~abi=contract.abi,
      ~provider,
    )
    addressInterfaceMapping->Js.Dict.set(
      contract.address->Ethers.ethAddressToString,
      contractEthers->Ethers.Contract.getInterface,
    )

    contract.events->Belt.Array.forEach(eventName => {
      let eventFilter = contractEthers->Ethers.Contract.getEventFilter(~eventName=Types.eventNameToString(eventName))
      let _ = eventFilters->Js.Array2.push(eventFilter)
    })
  })
  eventFilters
}

let processAllEventsFromBlockNumber = async (
  ~fromBlock,
  ~blockInterval as maxBlockInterval,
  ~chainConfig: Config.chainConfig,
  ~provider,
) => {
  let addressInterfaceMapping: Js.Dict.t<Ethers.Interface.t> = Js.Dict.empty()

  let eventFilters = getAllEventFilters(~addressInterfaceMapping, ~chainConfig, ~provider)

  let fromBlock = ref(fromBlock)
  let currentBlock: ref<option<int>> = ref(None)
  let shouldContinueProcess = () =>
    currentBlock.contents->Belt.Option.mapWithDefault(true, blockNum =>
      fromBlock.contents < blockNum
    )

  while shouldContinueProcess() {
    let rec executeQuery = (~blockInterval) => {
      //If the query hangs for longer than this, reject this promise to reduce the block interval
      let queryTimoutPromise =
        Time.resolvePromiseAfterDelay(~delayMilliseconds=queryTimeoutMillis)->Promise.then(() =>
          Promise.reject(QueryTimout(`Query took longer than ${Belt.Int.toString(queryTimeoutMillis / 1000)} seconds`))
        )

      let queryPromise =
        queryEventsWithCombinedFilterAndExecuteHandlers(
          ~addressInterfaceMapping,
          ~eventFilters,
          ~fromBlock=fromBlock.contents,
          ~toBlock=fromBlock.contents + blockInterval - 1,
          ~provider,
          ~chainId=chainConfig.chainId,
        )->Promise.thenResolve(_ => blockInterval)

      [queryTimoutPromise, queryPromise]
      ->Promise.race
      ->Promise.catch(err => {
        Js.log2(`Error getting events, waiting ${(backoffMillis / 1000)->Belt.Int.toString} seconds before retrying`, err)

        Time.resolvePromiseAfterDelay(~delayMilliseconds=backoffMillis)->Promise.then(_ => {
          let nextBlockIntervalTry = (blockInterval->Belt.Int.toFloat *. backoffMultiplicative)->Belt.Int.fromFloat
          Js.log3("Retrying query fromBlock and toBlock:", fromBlock, nextBlockIntervalTry)
          executeQuery(~blockInterval={nextBlockIntervalTry})
        })
      })
    }

    let executedBlockInterval = await executeQuery(~blockInterval=maxBlockInterval)

    fromBlock := fromBlock.contents + executedBlockInterval
    let currentBlockFromRPC =
      await provider
      ->Ethers.JsonRpcProvider.getBlockNumber
      ->Promise.catch(_err => {
        Js.log("Error getting current block number")
        currentBlock.contents->Belt.Option.getWithDefault(0)->Promise.resolve
      })
    currentBlock := Some(currentBlockFromRPC)
    Js.log(
      `Finished processAllEventsFromBlockNumber ${fromBlock.contents->Belt.Int.toString} out of ${currentBlockFromRPC->Belt.Int.toString}`,
    )
  }
}

let processAllEvents = (chainConfig: Config.chainConfig) => {
  let startBlock = chainConfig.startBlock

  processAllEventsFromBlockNumber(
    ~fromBlock=startBlock,
    ~chainConfig,
    ~blockInterval=2000,
    ~provider=chainConfig.provider,
  )
}

let startSyncingAllEvents = () => {
  Config.config
  ->Js.Dict.values
  ->Belt.Array.map(chainConfig => {
    chainConfig->processAllEvents
  })
  ->Promise.all
  ->Promise.thenResolve(_ => ())
}
