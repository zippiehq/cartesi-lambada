// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract AppChainState {
    event StateUpdated(bytes32 indexed appchainCID, uint256 indexed blockHeight, bytes stateCID, bytes32 outputHash);

    function postBlock(bytes32 appchainCID, uint256 blockHeight, bytes calldata stateCID, bytes32 outputHash) external {
        emit StateUpdated(appchainCID, blockHeight, stateCID, outputHash);
    }
}
