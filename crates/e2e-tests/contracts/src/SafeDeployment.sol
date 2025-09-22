// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

/// @title Safe Factory Interface
/// @dev Interface for deploying Safe contracts using a proxy factory
interface ISafeProxyFactory {
    function createProxyWithNonce(
        address _singleton,
        bytes memory initializer,
        uint256 saltNonce
    ) external returns (address proxy);
}

/// @title Safe Interface
/// @dev Interface for Safe contract initialization
interface ISafe {
    function setup(
        address[] calldata _owners,
        uint256 _threshold,
        address to,
        bytes calldata data,
        address fallbackHandler,
        address paymentToken,
        uint256 payment,
        address paymentReceiver
    ) external;
}

/// @title Mock Safe Singleton
/// @dev A minimal Safe implementation for testing purposes
contract MockSafeSingleton {
    uint256 public nonce;
    mapping(address => bool) public isOwner;
    uint256 public threshold;
    
    event SafeSetup(address indexed initiator, address[] owners, uint256 threshold);
    event ExecutionSuccess(bytes32 txHash);
    
    function setup(
        address[] calldata _owners,
        uint256 _threshold,
        address to,
        bytes calldata data,
        address fallbackHandler,
        address paymentToken,
        uint256 payment,
        address paymentReceiver
    ) external {
        require(_owners.length > 0, "Safe: owners required");
        require(_threshold > 0 && _threshold <= _owners.length, "Safe: invalid threshold");
        
        threshold = _threshold;
        
        for (uint256 i = 0; i < _owners.length; i++) {
            address owner = _owners[i];
            require(owner != address(0), "Safe: invalid owner");
            require(!isOwner[owner], "Safe: duplicate owner");
            isOwner[owner] = true;
        }
        
        emit SafeSetup(msg.sender, _owners, _threshold);
    }
    
    function execTransaction(
        address to,
        uint256 value,
        bytes calldata data,
        uint8 operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        bytes memory signatures
    ) external payable returns (bool success) {
        bytes32 txHash = keccak256(abi.encode(
            to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, nonce
        ));
        
        nonce++;
        
        if (to != address(0)) {
            if (operation == 0) {
                // CALL
                (success, ) = to.call{value: value}(data);
            } else {
                // DELEGATECALL  
                (success, ) = to.delegatecall(data);
            }
        } else {
            success = true;
        }
        
        if (success) {
            emit ExecutionSuccess(txHash);
        }
        
        return success;
    }
    
    function getTransactionHash(
        address to,
        uint256 value,
        bytes calldata data,
        uint8 operation,
        uint256 safeTxGas,
        uint256 baseGas,
        uint256 gasPrice,
        address gasToken,
        address refundReceiver,
        uint256 _nonce
    ) external view returns (bytes32) {
        return keccak256(abi.encode(
            to, value, data, operation, safeTxGas, baseGas, gasPrice, gasToken, refundReceiver, _nonce
        ));
    }
}

/// @title Mock Safe Proxy Factory
/// @dev A minimal factory for creating Safe proxies
contract MockSafeProxyFactory {
    address public immutable safeSingleton;
    
    event ProxyCreation(address proxy, address singleton);
    
    constructor(address _safeSingleton) {
        safeSingleton = _safeSingleton;
    }
    
    function createProxyWithNonce(
        address _singleton,
        bytes memory initializer,
        uint256 saltNonce
    ) external returns (address proxy) {
        require(_singleton == safeSingleton, "Invalid singleton");
        
        bytes memory deploymentData = abi.encodePacked(
            type(SafeProxy).creationCode,
            abi.encode(_singleton)
        );
        
        bytes32 salt = keccak256(abi.encodePacked(initializer, saltNonce));
        
        assembly {
            proxy := create2(0x0, add(0x20, deploymentData), mload(deploymentData), salt)
        }
        
        require(proxy != address(0), "Create2 failed");
        
        if (initializer.length > 0) {
            (bool success, ) = proxy.call(initializer);
            require(success, "Initialization failed");
        }
        
        emit ProxyCreation(proxy, _singleton);
    }
}

/// @title Safe Proxy
/// @dev Minimal proxy contract that delegates all calls to the Safe singleton
contract SafeProxy {
    address private immutable singleton;
    
    constructor(address _singleton) {
        singleton = _singleton;
    }
    
    fallback() external payable {
        address _singleton = singleton;
        assembly {
            calldatacopy(0, 0, calldatasize())
            let result := delegatecall(gas(), _singleton, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            
            switch result
            case 0 { revert(0, returndatasize()) }
            default { return(0, returndatasize()) }
        }
    }
    
    receive() external payable {}
}