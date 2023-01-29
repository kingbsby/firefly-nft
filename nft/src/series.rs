use crate::{Contract, StorageKey};
use crate::ContractExt;
use crate::metadata::TokenMetadata;
use crate::token::TokenId;
use crate::utils::refund_deposit;
use near_sdk::collections::UnorderedSet;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{AccountId, Balance, near_bindgen, env};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U128;
use serde_json::json;

const MAX_PRICE: Balance = 1_000_000_000 * 10u128.pow(24);

/// Note that token IDs for NFTs are strings on NEAR. It's still fine to use autoincrementing numbers as unique IDs if desired, but they should be stringified. This is to make IDs more future-proof as chain-agnostic conventions and standards arise, and allows for more flexibility with considerations like bridging NFTs across chains, etc.
pub type TokenSeriesId = String;

/// In this implementation, the Token struct takes two extensions standards (metadata and approval) as optional fields, as they are frequently used in modern NFTs.
#[derive(BorshDeserialize, BorshSerialize)]
pub struct TokenSeries {
	pub metadata: TokenMetadata,
	pub creator_id: AccountId,
	pub tokens: UnorderedSet<TokenId>,
    pub price: Option<Balance>,
    pub is_mintable: bool,
    // royalty: HashMap<AccountId, u32>
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct TokenSeriesJson {
    pub token_series_id: TokenSeriesId,
	pub metadata: TokenMetadata,
	pub creator_id: AccountId,
    // royalty: HashMap<AccountId, u32>,
    // transaction_fee: U128
}

#[near_bindgen]
impl Contract{
    #[payable]
    pub fn nft_create_series(
        &mut self,
        token_metadata: TokenMetadata,
        price: Option<U128>,
        // royalty: Option<HashMap<AccountId, u32>>,
    ) -> TokenSeriesJson {
        let initial_storage_usage = env::storage_usage();
        let caller_id = env::predecessor_account_id();

        let token_series_id = (self.token_series_by_id.len() + 1).to_string();

        assert!(
            self.token_series_by_id.get(&token_series_id).is_none(),
            "FireFly: duplicate token_series_id"
        );

        let title = token_metadata.title.clone();
        assert!(title.is_some(), "FireFly: token_metadata.title is required");
        
        // let mut total_perpetual = 0;
        // let mut total_accounts = 0;
        // let royalty_res: HashMap<AccountId, u32> = if let Some(royalty) = royalty {
        //     for (k , v) in royalty.iter() {
        //         if !is_valid_account_id(k.as_bytes()) {
        //             env::panic("Not valid account_id for royalty".as_bytes());
        //         };
        //         total_perpetual += *v;
        //         total_accounts += 1;
        //     }
        //     royalty
        // } else {
        //     HashMap::new()
        // };

        // assert!(total_accounts <= 50, "Paras: royalty exceeds 50 accounts");

        // assert!(
        //     total_perpetual <= 9000,
        //     "Paras Exceeds maximum royalty -> 9000",
        // );

        let price_res: Option<u128> = if price.is_some() {
            assert!(
                price.unwrap().0 < MAX_PRICE,
                "FireFly: price higher than {}",
                MAX_PRICE
            );
            Some(price.unwrap().0)
        } else {
            None
        };

        self.token_series_by_id.insert(&token_series_id, &TokenSeries{
            metadata: token_metadata.clone(),
            creator_id: caller_id.clone(),
            tokens: UnorderedSet::new(
                StorageKey::TokensBySeriesInner {
                    token_series: token_series_id.clone(),
                }
                .try_to_vec()
                .unwrap(),
            ),
            price: price_res,
            is_mintable: true,
            // royalty: royalty_res.clone(),
        });

        // set market data transaction fee (need to understand)
        // let current_transaction_fee = self.calculate_current_transaction_fee();
        // self.market_data_transaction_fee.insert(&token_series_id, &current_transaction_fee);

        env::log_str(
            json!({
                "type": "nft_create_series",
                "params": {
                    "token_series_id": token_series_id,
                    "token_metadata": token_metadata,
                    "creator_id": caller_id.clone(),
                    "price": price
                    // "royalty": royalty_res,
                    // "transaction_fee": &current_transaction_fee.to_string()
                }
            }).to_string().as_str()
        );

        refund_deposit(env::storage_usage() - initial_storage_usage);

		TokenSeriesJson{
            token_series_id,
			metadata: token_metadata,
			creator_id: caller_id.into(),
            // royalty: royalty_res,
            // transaction_fee: current_transaction_fee.into()
		}
    }

    /**
    Get list of all TokenSeries
    */
    pub fn nft_series_for_all(&self) -> Vec<TokenSeriesJson>{
        self.token_series_by_id
        .iter()
        .map(|se| {
            TokenSeriesJson{
                token_series_id: se.0,
                metadata: se.1.metadata,
                creator_id: se.1.creator_id,
            }
        }).collect()
        
    }

    // pub fn calculate_current_transaction_fee(&mut self) -> u128 {
    //     let transaction_fee: &TransactionFee = &self.transaction_fee;
    //     if transaction_fee.next_fee.is_some() {
    //         if to_sec(env::block_timestamp()) >= transaction_fee.start_time.unwrap() {
    //             self.transaction_fee.current_fee = transaction_fee.next_fee.unwrap();
    //             self.transaction_fee.next_fee = None;
    //             self.transaction_fee.start_time = None;
    //         }
    //     }
    //     self.transaction_fee.current_fee as u128
    // }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;
    use super::*;

    // const MINT_STORAGE_COST: u128 = 5870000000000000000000;
    const MINT_STORAGE_COST: u128 = 5910000000000000000000;

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
    fn test_create_series() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(1).into());

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(1))
            .build());

        let token_series = contract.nft_create_series(sample_token_metadata(), Some(U128::from(0u128)));
        assert_eq!(token_series.token_series_id, "1".to_string());
        assert_eq!(token_series.creator_id.to_string(), accounts(1).to_string());
        assert_eq!(token_series.metadata, sample_token_metadata());
    }


}