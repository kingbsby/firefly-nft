use crate::Contract;
use crate::approval::ext_nft_approval_receiver;
/// Common implementation of the [approval management standard](https://nomicon.io/Standards/NonFungibleToken/ApprovalManagement.html) for NFTs.
/// on the contract/account that has just been approved. This is not required to implement.
use crate::approval::NonFungibleTokenApproval;
use crate::token::TokenId;
use crate::utils::{
    assert_at_least_one_yocto, bytes_for_approved_account_id, refund_approved_account_ids,
    refund_approved_account_ids_iter, refund_deposit,
};
use near_sdk::{assert_one_yocto, env, require, AccountId, Gas, Promise};

const GAS_FOR_NFT_APPROVE: Gas = Gas(10_000_000_000_000);

fn expect_token_found<T>(option: Option<T>) -> T {
    option.unwrap_or_else(|| env::panic_str("Token not found"))
}

fn expect_approval<T>(option: Option<T>) -> T {
    option.unwrap_or_else(|| env::panic_str("next_approval_by_id must be set for approval ext"))
}

impl NonFungibleTokenApproval for Contract {
    fn nft_approve(
        &mut self,
        token_id: TokenId,
        account_id: AccountId,
        msg: Option<String>,
    ) -> Option<Promise> {
        assert_at_least_one_yocto();
        let approvals_by_id = self
            .tokens
            .approvals_by_id
            .as_mut()
            .unwrap_or_else(|| env::panic_str("NFT does not support Approval Management"));

        let owner_id = expect_token_found(self.tokens.owner_by_id.get(&token_id));

        require!(env::predecessor_account_id() == owner_id, "Predecessor must be token owner.");

        let next_approval_id_by_id = expect_approval(self.tokens.next_approval_id_by_id.as_mut());
        // update HashMap of approvals for this token
        let approved_account_ids = &mut approvals_by_id.get(&token_id).unwrap_or_default();
        let approval_id: u64 = next_approval_id_by_id.get(&token_id).unwrap_or(1u64);
        let old_approval_id = approved_account_ids.insert(account_id.clone(), approval_id);

        // save updated approvals HashMap to contract's LookupMap
        approvals_by_id.insert(&token_id, approved_account_ids);

        // increment next_approval_id for this token
        next_approval_id_by_id.insert(&token_id, &(approval_id + 1));

        // If this approval replaced existing for same account, no storage was used.
        // Otherwise, require that enough deposit was attached to pay for storage, and refund
        // excess.
        let storage_used =
            if old_approval_id.is_none() { bytes_for_approved_account_id(&account_id) } else { 0 };
        refund_deposit(storage_used);

        // if given `msg`, schedule call to `nft_on_approve` and return it. Else, return None.
        msg.map(|msg| {
            ext_nft_approval_receiver::ext(account_id)
                .with_static_gas(env::prepaid_gas() - GAS_FOR_NFT_APPROVE)
                .nft_on_approve(token_id, owner_id, approval_id, msg)
        })
    }

    fn nft_revoke(&mut self, token_id: TokenId, account_id: AccountId) {
        assert_one_yocto();
        let approvals_by_id = self.tokens.approvals_by_id.as_mut().unwrap_or_else(|| {
            env::panic_str("NFT does not support Approval Management");
        });

        let owner_id = expect_token_found(self.tokens.owner_by_id.get(&token_id));
        let predecessor_account_id = env::predecessor_account_id();

        require!(predecessor_account_id == owner_id, "Predecessor must be token owner.");

        // if token has no approvals, do nothing
        if let Some(approved_account_ids) = &mut approvals_by_id.get(&token_id) {
            // if account_id was already not approved, do nothing
            if approved_account_ids.remove(&account_id).is_some() {
                refund_approved_account_ids_iter(
                    predecessor_account_id,
                    core::iter::once(&account_id),
                );
                // if this was the last approval, remove the whole HashMap to save space.
                if approved_account_ids.is_empty() {
                    approvals_by_id.remove(&token_id);
                } else {
                    // otherwise, update approvals_by_id with updated HashMap
                    approvals_by_id.insert(&token_id, approved_account_ids);
                }
            }
        }
    }

    fn nft_revoke_all(&mut self, token_id: TokenId) {
        assert_one_yocto();
        let approvals_by_id = self.tokens.approvals_by_id.as_mut().unwrap_or_else(|| {
            env::panic_str("NFT does not support Approval Management");
        });

        let owner_id = expect_token_found(self.tokens.owner_by_id.get(&token_id));
        let predecessor_account_id = env::predecessor_account_id();

        require!(predecessor_account_id == owner_id, "Predecessor must be token owner.");

        // if token has no approvals, do nothing
        if let Some(approved_account_ids) = &mut approvals_by_id.get(&token_id) {
            // otherwise, refund owner for storage costs of all approvals...
            refund_approved_account_ids(predecessor_account_id, approved_account_ids);
            // ...and remove whole HashMap of approvals
            approvals_by_id.remove(&token_id);
        }
    }

    fn nft_is_approved(
        &self,
        token_id: TokenId,
        approved_account_id: AccountId,
        approval_id: Option<u64>,
    ) -> bool {
        expect_token_found(self.tokens.owner_by_id.get(&token_id));

        let approvals_by_id = if let Some(a) = self.tokens.approvals_by_id.as_ref() {
            a
        } else {
            // contract does not support approval management
            return false;
        };

        let approved_account_ids = if let Some(ids) = approvals_by_id.get(&token_id) {
            ids
        } else {
            // token has no approvals
            return false;
        };

        let actual_approval_id = if let Some(id) = approved_account_ids.get(&approved_account_id) {
            id
        } else {
            // account not in approvals HashMap
            return false;
        };

        if let Some(given_approval_id) = approval_id {
            &given_approval_id == actual_approval_id
        } else {
            // account approved, no approval_id given
            true
        }
    }
}


#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use near_sdk::json_types::U128;
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::testing_env;
    use crate::TokenMetadata;

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
    fn test_approve() {
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

        // alice approves bob
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(170000000000000000000)
            .predecessor_account_id(accounts(2))
            .build());
        contract.nft_approve(token.token_id.clone(), accounts(3), None);

        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        assert!(contract.nft_is_approved(token.token_id.clone(), accounts(3), Some(1)));
    }

    #[test]
    fn test_revoke() {
        let mut context = get_context(accounts(0));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(0).into());

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(0))
            .build());
        let token_id = "0".to_string();
        contract.nft_mint(token_id.clone(), accounts(0));

        // alice approves bob
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(150000000000000000000)
            .predecessor_account_id(accounts(0))
            .build());
        contract.nft_approve(token_id.clone(), accounts(1), None);

        // alice revokes bob
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(1)
            .predecessor_account_id(accounts(0))
            .build());
        contract.nft_revoke(token_id.clone(), accounts(1));
        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        assert!(!contract.nft_is_approved(token_id.clone(), accounts(1), None));
    }

    #[test]
    fn test_revoke_all() {
        let mut context = get_context(accounts(0));
        testing_env!(context.build());
        let mut contract = Contract::new_default_meta(accounts(0).into());

        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(MINT_STORAGE_COST)
            .predecessor_account_id(accounts(0))
            .build());
        let token_id = "0".to_string();
        contract.nft_mint(token_id.clone(), accounts(0));

        // alice approves bob
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(150000000000000000000)
            .predecessor_account_id(accounts(0))
            .build());
        contract.nft_approve(token_id.clone(), accounts(1), None);

        // alice revokes bob
        testing_env!(context
            .storage_usage(env::storage_usage())
            .attached_deposit(1)
            .predecessor_account_id(accounts(0))
            .build());
        contract.nft_revoke_all(token_id.clone());
        testing_env!(context
            .storage_usage(env::storage_usage())
            .account_balance(env::account_balance())
            .is_view(true)
            .attached_deposit(0)
            .build());
        assert!(!contract.nft_is_approved(token_id.clone(), accounts(1), Some(1)));
    }
}