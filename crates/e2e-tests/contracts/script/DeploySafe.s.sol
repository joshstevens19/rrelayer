// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Script.sol";
import "forge-std/console.sol";
import "../src/SafeDeployment.sol";

contract DeploySafe is Script {
    function run() external {
        vm.startBroadcast();
        
        // Use CREATE2 for deterministic singleton deployment
        bytes32 singletonSalt = keccak256("MockSafeSingleton_v1");
        MockSafeSingleton safeSingleton = new MockSafeSingleton{salt: singletonSalt}();
        console.log("Safe Singleton deployed to:", address(safeSingleton));
        
        // Use CREATE2 for deterministic factory deployment
        bytes32 factorySalt = keccak256("MockSafeProxyFactory_v1");
        MockSafeProxyFactory proxyFactory = new MockSafeProxyFactory{salt: factorySalt}(address(safeSingleton));
        console.log("Safe Proxy Factory deployed to:", address(proxyFactory));
        
        // Define the owner for the Safe (wallet index 80)
        address[] memory owners = new address[](1);
        owners[0] = 0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf; // Wallet index 80
        
        // Setup parameters for Safe initialization
        uint256 threshold = 1; // Single owner threshold
        address to = address(0); // No initial transaction
        bytes memory data = ""; // No initial data
        address fallbackHandler = address(0); // No fallback handler
        address paymentToken = address(0); // ETH
        uint256 payment = 0; // No payment
        address paymentReceiver = address(0); // No payment receiver
        
        // Encode the setup call
        bytes memory initializer = abi.encodeWithSelector(
            ISafe.setup.selector,
            owners,
            threshold,
            to,
            data,
            fallbackHandler,
            paymentToken,
            payment,
            paymentReceiver
        );
        
        // Create the Safe proxy with a deterministic salt
        // Using the same approach as TestToken to ensure consistent address  
        uint256 saltNonce = 42; // Simple deterministic salt
        address safeProxy = proxyFactory.createProxyWithNonce(
            address(safeSingleton),
            initializer,
            saltNonce
        );
        
        console.log("Safe Proxy deployed to:", safeProxy);
        console.log("Safe owner (wallet index 1000):", owners[0]);
        console.log("Safe threshold:", threshold);
        
        vm.stopBroadcast();
    }
}