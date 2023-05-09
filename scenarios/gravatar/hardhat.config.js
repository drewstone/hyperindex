require("@nomiclabs/hardhat-ethers");

module.exports = {
  solidity: "0.8.18",
  paths: {
    sources: "./src/__tests__/helpers/contracts"
  },
  networks: {
    localhost: {
      url: "http://127.0.0.1:8545",
      accounts: {
        mnemonic: "test test test test test test test test test test test junk",
      },
    },
  },
};