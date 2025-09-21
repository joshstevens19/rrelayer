// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Script.sol";
import "../src/TestToken.sol";

contract DeployMyCustomToken is Script {
    function run() external {
        vm.startBroadcast();
        
        // Deploy normally but ensure deterministic nonce by resetting chain state
        // The contract will deploy to a predictable address based on deployer + nonce
        TestToken token = new TestToken();
        
        console.log("TestToken deployed to:", address(token));
        console.log("Total supply:", token.totalSupply());
        console.log("Deployer balance:", token.balanceOf(msg.sender));
        
        vm.stopBroadcast();
    }
}