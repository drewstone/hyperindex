open RescriptMocha
open Mocha
open Belt
open TestHelpers
let {
  it: it_promise,
  it_only: it_promise_only,
  it_skip: it_skip_promise,
  before: before_promise,
} = module(RescriptMocha.Promise)

describe("Transfers", () => {
  it_promise(
    "Transfer subtracts the from account balance and adds to the to account balance",
    async () => {
      //Get mock addresses from helpers
      let userAddress1 = Ethers.Addresses.mockAddresses[0]->Option.getUnsafe
      let userAddress2 = Ethers.Addresses.mockAddresses[1]->Option.getUnsafe

      let account_id = userAddress1->Ethers.ethAddressToString
      //Make a mock entity to set the initial state of the mock db
      let mockAccountEntity: Entities.Account.t = {
        id: account_id,
      }

      let tokenAddress = Ethers.Addresses.defaultAddress->Ethers.ethAddressToString
      let mockAccountTokenEntity = EventHandlers.makeAccountToken(
        ~account_id,
        ~tokenAddress,
        ~balance=Ethers.BigInt.fromInt(5),
      )

      //Set an initial state for the user
      //Note: set and delete functions do not mutate the mockDb, they return a new
      //mockDb with with modified state
      let mockDb = MockDb.createMockDb().entities.account.set(
        mockAccountEntity,
      ).entities.accountToken.set(mockAccountTokenEntity)

      //Create a mock Transfer event from userAddress1 to userAddress2
      let mockTransfer = ERC20.Transfer.createMockEvent({
        from: userAddress1,
        to: userAddress2,
        value: Ethers.BigInt.fromInt(3),
      })

      //Process the mockEvent
      //Note: processEvent functions do not mutate the mockDb, they return a new
      //mockDb with with modified state
      let mockDbAfterTransfer = await ERC20.Transfer.processEvent({
        event: mockTransfer,
        mockDb,
      })

      //Get the balance of userAddress1 after the transfer
      let account1Balance =
        mockDbAfterTransfer.entities.accountToken.get(
          EventHandlers.makeAccountTokenId(~account_id, ~tokenAddress),
        )->Option.map(a => a.balance)

      //Assert the expected balance
      Assert.equal(
        account1Balance,
        Some(Ethers.BigInt.fromInt(2)),
        ~message="Should have subtracted transfer amount 3 from userAddress1 balance 5",
      )

      //Get the balance of userAddress2 after the transfer
      let account2Balance =
        mockDbAfterTransfer.entities.accountToken.get(
          EventHandlers.makeAccountTokenId(
            ~account_id=userAddress2->Ethers.ethAddressToString,
            ~tokenAddress,
          ),
        )->Option.map(a => a.balance)
      //Assert the expected balance
      Assert.equal(
        Some(Ethers.BigInt.fromInt(3)),
        account2Balance,
        ~message="Should have added transfer amount 3 to userAddress2 balance 0",
      )
    },
  )

  it_promise("Deletes Account", async () => {
    //Get mock addresses from helpers
    let userAddress1 = Ethers.Addresses.mockAddresses[0]->Option.getUnsafe

    let account_id = userAddress1->Ethers.ethAddressToString
    //Make a mock entity to set the initial state of the mock db
    let mockAccountEntity: Types.accountEntity = {
      id: account_id,
    }

    //Set an initial state for the user
    //Note: set and delete functions do not mutate the mockDb, they return a new
    //mockDb with with modified state
    let mockDb = MockDb.createMockDb().entities.account.set(mockAccountEntity)

    let mockDeleteUser = ERC20Factory.DeleteUser.createMockEvent({user: userAddress1})

    //Process the mockEvent
    //Note: processEvent functions do not mutate the mockDb, they return a new
    //mockDb with with modified state
    let mockDbAfterDelete = await ERC20Factory.DeleteUser.processEvent({
      event: mockDeleteUser,
      mockDb,
    })

    //Get the balance of userAddress1 after the transfer
    let accountsInDb = mockDbAfterDelete.entities.account.getAll()
    //Assert the expected balance
    Assert.equal(accountsInDb->Array.length, 0, ~message="Should have delete account 1")
  })
})
