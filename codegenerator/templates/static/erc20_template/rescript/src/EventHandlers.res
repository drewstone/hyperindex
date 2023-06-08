open Types

Handlers.ERC20Contract.registerApprovalLoadEntities((~event, ~context) => {
  // loading the required accountEntity
  context.account.ownerAccountChangesLoad(event.params.owner->Ethers.ethAddressToString)
})

Handlers.ERC20Contract.registerApprovalHandler((~event, ~context) => {
  //  getting the owner accountEntity
  let ownerAccount = context.account.ownerAccountChanges()

  switch ownerAccount {
  | Some(existingAccount) => {
      // updating accountEntity object
      let accountObject: accountEntity = {
        id: existingAccount.id,
        approval: event.params.value,
        balance: existingAccount.balance,
      }

      // updating the accountEntity with the new transfer field value
      context.account.update(accountObject)
    }

  | None => {
      // updating accountEntity object
      let accountObject: accountEntity = {
        id: event.params.owner->Ethers.ethAddressToString,
        approval: event.params.value,
        balance: Ethers.BigInt.fromInt(0),
      }

      // inserting the accountEntity with the new transfer field value
      context.account.insert(accountObject)
    }
  }
})

Handlers.ERC20Contract.registerTransferLoadEntities((~event, ~context) => {
  // loading the required accountEntity
  context.account.senderAccountChangesLoad(event.params.from->Ethers.ethAddressToString)
  context.account.receiverAccountChangesLoad(event.params.to->Ethers.ethAddressToString)
})

Handlers.ERC20Contract.registerTransferHandler((~event, ~context) => {
  // getting the sender accountEntity
  let senderAccount = context.account.senderAccountChanges()

  switch senderAccount {
  | Some(existingSenderAccount) => {
      // updating accountEntity object
      let accountObject: accountEntity = {
        id: existingSenderAccount.id,
        approval: existingSenderAccount.approval,
        balance: existingSenderAccount.balance->Ethers.BigInt.sub(event.params.value),
      }

      // updating the accountEntity with the new transfer field value
      context.account.update(accountObject)
    }

  | None => {
      // updating accountEntity object
        let accountObject: accountEntity = {
          id: event.params.from->Ethers.ethAddressToString,
          approval: Ethers.BigInt.fromInt(0),
          balance: Ethers.BigInt.fromInt(0) ->Ethers.BigInt.sub(event.params.value),
        }

        // inserting the accountEntity with the new transfer field value
        context.account.insert(accountObject)
    }
  }

  // getting the sender accountEntity
  let receiverAccount = context.account.receiverAccountChanges()

  switch receiverAccount {
  | Some(existingReceiverAccount) => {
      // updating accountEntity object
      let accountObject: accountEntity = {
        id: existingReceiverAccount.id,
        approval: existingReceiverAccount.approval,
        balance: existingReceiverAccount.balance->Ethers.BigInt.add(event.params.value),
      }

      // updating the accountEntity with the new transfer field value
      context.account.update(accountObject)
    }

  | None => {
      // updating accountEntity object
          let accountObject: accountEntity = {
            id: event.params.to->Ethers.ethAddressToString,
            approval: Ethers.BigInt.fromInt(0),
            balance: event.params.value,
          }

          // inserting the accountEntity with the new transfer field value
          context.account.insert(accountObject)
    }
  }
})
