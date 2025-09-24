// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Script.sol";
import "forge-std/console.sol";
import "../src/SafeDeployment.sol";

contract DeploySafe is Script {
    function run() external {
        vm.startBroadcast();

        bytes32 singletonSalt = keccak256("MockSafeSingleton_v1");
        MockSafeSingleton safeSingleton = new MockSafeSingleton{salt: singletonSalt}();
        console.log("Safe Singleton deployed to:", address(safeSingleton));

        bytes32 factorySalt = keccak256("MockSafeProxyFactory_v1");
        MockSafeProxyFactory proxyFactory = new MockSafeProxyFactory{salt: factorySalt}(address(safeSingleton));
        console.log("Safe Proxy Factory deployed to:", address(proxyFactory));

        address[] memory owners = new address[](1);

        try vm.envAddress("SAFE_OWNER_ADDRESS") returns (address envAddress) {
            owners[0] = envAddress;
        } catch {
            owners[0] = 0x1C073e63f70701BC545019D3c4f2a25A69eCA8Cf;
        }
        
        // Setup parameters for Safe initialization
        uint256 threshold = 1;
        address to = address(0);
        bytes memory data = "";
        address fallbackHandler = address(0);
        address paymentToken = address(0);
        uint256 payment = 0;
        address paymentReceiver = address(0);

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

        uint256 saltNonce = 42;
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