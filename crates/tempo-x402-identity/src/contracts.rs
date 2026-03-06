//! Solidity ABI bindings for ERC-8004 registries.
//!
//! Uses alloy `sol!` macro following the same pattern as `TIP20` in `x402/lib.rs`.

use alloy::sol;

sol! {
    /// ERC-8004 Agent Identity Registry (ERC-721 Enumerable).
    #[sol(rpc)]
    interface IAgentIdentity {
        function mint(address owner, string metadataURI) external returns (uint256);
        function ownerOf(uint256 tokenId) external view returns (address);
        function setRecoveryAddress(uint256 tokenId, address recovery) external;
        function recoverAgent(uint256 tokenId, address newOwner) external;
        function updateMetadata(uint256 tokenId, string uri) external;
        function getMetadataURI(uint256 tokenId) external view returns (string);
        function totalSupply() external view returns (uint256);
        function tokenByIndex(uint256 index) external view returns (uint256);
        function tokenOfOwnerByIndex(address owner, uint256 index) external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);
    }

    /// ERC-8004 Agent Reputation Registry.
    #[sol(rpc)]
    interface IAgentReputation {
        function submitFeedback(uint256 agentId, bool isPositive, string metadataURI) external;
        function getReputation(uint256 agentId) external view returns (uint256 positive, uint256 negative, uint256 neutral);
    }

    /// ERC-8004 Agent Validation Registry.
    #[sol(rpc)]
    interface IAgentValidation {
        function registerValidator(uint256 agentId, address validator) external;
        function removeValidator(uint256 agentId, address validator) external;
        function executeWithValidation(uint256 agentId, address target, bytes data, uint256 value) external returns (bytes);
    }
}
