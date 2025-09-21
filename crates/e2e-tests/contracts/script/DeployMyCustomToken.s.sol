// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Script.sol";
import "../src/TestToken.sol";

contract DeployMyCustomToken is Script {
    function run() external {
        vm.startBroadcast();
        
        TestToken token = new TestToken();
        
        console.log("TestToken deployed to:", address(token));
        console.log("Total supply:", token.totalSupply());
        console.log("Deployer balance:", token.balanceOf(msg.sender));
        
        vm.stopBroadcast();
    }
}