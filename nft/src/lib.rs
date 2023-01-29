pub mod approval;
pub mod token;
pub mod nft_core;
pub mod enumeration;
/// NFT utility functions
mod utils;
pub mod metadata;
pub mod events;
pub mod event;
pub mod series;

use enumeration::NonFungibleTokenEnumeration;
use metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, TokenMetadata, NFT_METADATA_SPEC,
};
use series::{TokenSeriesId, TokenSeries};
// use utils::*;
use token::{Token, TokenId};
use nft_core::{NonFungibleToken, NonFungibleTokenCore};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LazyOption, UnorderedMap};
use near_sdk::{
    env, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault, PromiseOrValue
};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    tokens: NonFungibleToken,
    metadata: LazyOption<NFTContractMetadata>,
    token_series_by_id: UnorderedMap<TokenSeriesId, TokenSeries>,
    market_data_transaction_fee: UnorderedMap<TokenSeriesId, u128>,
}

const DATA_IMAGE_SVG_NEAR_ICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 288 288'%3E%3Cg id='l' data-name='l'%3E%3Cpath d='M187.58,79.81l-30.1,44.69a3.2,3.2,0,0,0,4.75,4.2L191.86,103a1.2,1.2,0,0,1,2,.91v80.46a1.2,1.2,0,0,1-2.12.77L102.18,77.93A15.35,15.35,0,0,0,90.47,72.5H87.34A15.34,15.34,0,0,0,72,87.84V201.16A15.34,15.34,0,0,0,87.34,216.5h0a15.35,15.35,0,0,0,13.08-7.31l30.1-44.69a3.2,3.2,0,0,0-4.75-4.2L96.14,186a1.2,1.2,0,0,1-2-.91V104.61a1.2,1.2,0,0,1,2.12-.77l89.55,107.23a15.35,15.35,0,0,0,11.71,5.43h3.13A15.34,15.34,0,0,0,216,201.16V87.84A15.34,15.34,0,0,0,200.66,72.5h0A15.35,15.35,0,0,0,187.58,79.81Z'/%3E%3C/g%3E%3C/svg%3E";

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    NonFungibleToken,
    Metadata,
    TokenMetadata,
    Enumeration,
    Approval,
    TokenSeriesById,
    TokensBySeriesInner { token_series: String },
    MarketDataTransactionFee,
}

#[near_bindgen]
impl Contract {
    /// Initializes the contract owned by `owner_id` with
    /// default metadata (for example purposes only).
    #[init]
    pub fn new_default_meta(owner_id: AccountId) -> Self {
        Self::new(
            owner_id,
            NFTContractMetadata {
                spec: NFT_METADATA_SPEC.to_string(),
                name: "Example NEAR non-fungible token".to_string(),
                symbol: "EXAMPLE".to_string(),
                icon: Some(DATA_IMAGE_SVG_NEAR_ICON.to_string()),
                base_uri: None,
                reference: None,
                reference_hash: None,
            },
        )
    }

    #[init]
    pub fn new(owner_id: AccountId, metadata: NFTContractMetadata) -> Self {
        assert!(!env::state_exists(), "Already initialized");
        metadata.assert_valid();
        Self {
            tokens: NonFungibleToken::new(
                StorageKey::NonFungibleToken,
                owner_id,
                StorageKey::TokenMetadata,
                Some(StorageKey::Enumeration),
                Some(StorageKey::Approval),
            ),
            metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),
            token_series_by_id: UnorderedMap::new(StorageKey::TokenSeriesById),
            market_data_transaction_fee: UnorderedMap::new(StorageKey::MarketDataTransactionFee),
        }
    }

    /// Mint a new token with ID=`token_id` belonging to `receiver_id`.
    ///
    /// Since this example implements metadata, it also requires per-token metadata to be provided
    /// in this call. `self.tokens.mint` will also require it to be Some, since
    /// `StorageKey::TokenMetadata` was provided at initialization.
    ///
    /// `self.tokens.mint` will enforce `predecessor_account_id` to equal the `owner_id` given in
    /// initialization call to `new`.
    #[payable]
    pub fn nft_mint(
        &mut self,
        token_series_id: TokenSeriesId,
        receiver_id: AccountId,
    ) -> Token {
        self.tokens.internal_mint(&mut self.token_series_by_id, token_series_id, receiver_id)
    }

    pub fn nft_tokens_owner(&self, owner_id: AccountId) -> Vec<Token> {
        self.tokens.nft_tokens_for_owner(owner_id, None, None)
    }
}

#[near_bindgen]
impl NonFungibleTokenCore for Contract {
    #[payable]
    fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        self.tokens.nft_transfer(receiver_id, token_id, approval_id, memo)
    }

    #[payable]
    fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<bool> {
        self.tokens.nft_transfer_call(receiver_id, token_id, approval_id, memo, msg)
    }

    fn nft_token(&self, token_id: TokenId) -> Option<Token> {
        self.tokens.nft_token(token_id)
    }
}

#[near_bindgen]
impl NonFungibleTokenMetadataProvider for Contract {
    fn nft_metadata(&self) -> NFTContractMetadata {
        self.metadata.get().unwrap()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::json_types::U128;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;
    use std::collections::HashMap;

    use super::*;

    // const MINT_STORAGE_COST: u128 = 5870000000000000000000;
    // const MINT_STORAGE_COST: u128 = 5910000000000000000000;

    fn get_context(predecessor_account_id: AccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    fn sample_token_metadata() -> TokenMetadata {
        TokenMetadata {
            title: Some("Olympus Mons".into()),
            description: Some("The tallest mountain in the charted solar system".into()),
            media: None,
            media_hash: None,
            copies: Some(1u64),
            issued_at: None,
            expires_at: None,
            starts_at: None,
            updated_at: None,
            extra: None,
            reference: None,
            reference_hash: None,
        }
    }

    #[test]
    fn test_new() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let contract = Contract::new_default_meta(accounts(1));
        testing_env!(context.is_view(true).build());
        assert_eq!(contract.nft_token("1".to_string()), None);
    }

    #[test]
    #[should_panic(expected = "The contract is not initialized")]
    fn test_default() {
        let context = get_context(accounts(1));
        testing_env!(context.build());
        let _contract = Contract::default();
    }

    #[test]
    fn test_mint() {
        const MINT_STORAGE_COST: u128 = 6010000000000000000000;
        let mut context = get_context(accounts(0));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(0).into());

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(0))
            .build());
        let token_series = contract.nft_create_series(sample_token_metadata(), Some(U128::from(0u128)));
        let token = contract.nft_mint(token_series.token_series_id, accounts(2));
        assert_eq!(token.token_id, "1:1".to_string());
        assert_eq!(token.owner_id.to_string(), accounts(2).to_string());
        assert_eq!(token.metadata, sample_token_metadata());
        assert_eq!(token.approved_account_ids.unwrap(), HashMap::new());
    }

    #[test]
    fn test_transfer() {
        const MINT_STORAGE_COST: u128 = 6010000000000000000000;
        let mut context = get_context(accounts(0));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(0).into());

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(0))
            .build());
        let token_series = contract.nft_create_series(sample_token_metadata(), Some(U128::from(0u128)));
        let token = contract.nft_mint(token_series.token_series_id, accounts(1));

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(1)
            .predecessor_account_id(accounts(1))
            .build());
        contract.nft_transfer(accounts(3), token.token_id.clone(), None, None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        if let Some(token) = contract.nft_token(token.token_id.clone()) {
            assert_eq!(token.token_id, "1:1".to_string());
            assert_eq!(token.owner_id.to_string(), accounts(3).to_string());
            assert_eq!(token.metadata, sample_token_metadata());
            assert_eq!(token.approved_account_ids.unwrap(), HashMap::new());
        } else {
            panic!("token not correctly created, or not found by nft_token");
        }
    }
}