#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address, String, BytesN, Vec};
use soroban_sdk::xdr::ToXdr;

pub mod types;
pub mod errors;
pub mod storage;
pub mod events;

use crate::errors::CarbonChainError;
use crate::storage::{
    set_admin, get_admin, has_admin,
    set_credit, get_credit,
    get_verifiers, set_verifiers, is_verifier,
    add_credit_to_project, get_credits_by_project,
    set_retirement_contract, get_retirement_contract,
    set_paused, is_paused,
};
use crate::types::{CreditMetadata, CreditStatus, DataKey, ServiceType};
use crate::events::{
    credit_submitted, credit_minted, verifier_added, verifier_removed,
    contract_paused, contract_unpaused,
};

#[cfg(not(feature = "library"))]
#[contract]
pub struct CreditRegistry;

#[cfg(not(feature = "library"))]
#[contractimpl]
impl CreditRegistry {
    // ── Admin ────────────────────────────────────────────────────────────────

    /// Initialise the registry. Must be called exactly once.
    ///
    /// # Errors
    /// - [`CarbonChainError::AlreadyInitialized`] — contract has already been initialised.
    pub fn initialize(env: Env, admin: Address, retirement_contract: Address) -> Result<(), CarbonChainError> {
        if has_admin(&env) {
            return Err(CarbonChainError::AlreadyInitialized);
        }
        // Validate that admin is a legitimate, authorised address.
        // require_auth() will panic for zero/invalid addresses in the Soroban VM.
        admin.require_auth();
        set_admin(&env, &admin);
        set_retirement_contract(&env, &retirement_contract);
        Ok(())
    }

    // ── Pause / Unpause ──────────────────────────────────────────────────────

    /// Pause all state-mutating operations. Only the admin may call this.
    ///
    /// # Errors
    /// - [`CarbonChainError::NotInitialized`] — contract has not been initialised.
    /// - [`CarbonChainError::Unauthorized`] — caller is not the admin.
    pub fn pause(env: Env, admin: Address) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        set_paused(&env, true);
        contract_paused(&env, admin);
        Ok(())
    }

    /// Resume all state-mutating operations. Only the admin may call this.
    ///
    /// # Errors
    /// - [`CarbonChainError::NotInitialized`] — contract has not been initialised.
    /// - [`CarbonChainError::Unauthorized`] — caller is not the admin.
    pub fn unpause(env: Env, admin: Address) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        set_paused(&env, false);
        contract_unpaused(&env, admin);
        Ok(())
    }

    /// Returns `true` if the contract is currently paused.
    pub fn paused(env: Env) -> bool {
        is_paused(&env)
    }

    // ── Verifier management ──────────────────────────────────────────────────

    /// Add a verifier to the authorised set. Requires a valid admin nonce for replay protection.
    ///
    /// # Errors
    /// - [`CarbonChainError::NotInitialized`] — contract has not been initialised.
    /// - [`CarbonChainError::Unauthorized`] — caller is not the admin.
    /// - [`CarbonChainError::InvalidNonce`] — `nonce` does not match the current admin nonce.
    /// - [`CarbonChainError::VerifierAlreadyExists`] — `verifier` is already registered.
    pub fn register_verifier(env: Env, admin: Address, verifier: Address, nonce: u64) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        if !consume_nonce(&env, &admin, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        if is_verifier(&env, &verifier) {
            return Err(CarbonChainError::VerifierAlreadyExists);
        }
        let mut verifiers = get_verifiers(&env);
        verifiers.push_back(verifier.clone());
        set_verifiers(&env, &verifiers);
        verifier_added(&env, admin, verifier);
        Ok(())
    }

    /// Remove a verifier from the authorised set. Requires a valid admin nonce.
    ///
    /// # Errors
    /// - [`CarbonChainError::NotInitialized`] — contract has not been initialised.
    /// - [`CarbonChainError::Unauthorized`] — caller is not the admin.
    /// - [`CarbonChainError::InvalidNonce`] — `nonce` does not match the current admin nonce.
    /// - [`CarbonChainError::VerifierNotFound`] — `verifier` is not in the registered set.
    pub fn remove_verifier(env: Env, admin: Address, verifier: Address, nonce: u64) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        if !consume_nonce(&env, &admin, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        if !is_verifier(&env, &verifier) {
            return Err(CarbonChainError::VerifierNotFound);
        }
        let old = get_verifiers(&env);
        let mut new_list: Vec<Address> = Vec::new(&env);
        for v in old.iter() {
            if v != verifier {
                new_list.push_back(v);
            }
        }
        set_verifiers(&env, &new_list);
        verifier_removed(&env, admin, verifier);
        Ok(())
    }

    /// Returns up to the first 50 verifiers. Use `list_verifiers_paginated` for larger sets.
    pub fn list_verifiers(env: Env) -> Vec<Address> {
        let all = get_verifiers(&env);
        let cap: u32 = 50;
        if all.len() <= cap {
            return all;
        }
        let mut out: Vec<Address> = Vec::new(&env);
        for i in 0..cap {
            out.push_back(all.get(i).unwrap());
        }
        out
    }

    /// Returns one page of verifiers. `page` is 0-indexed; `page_size` must be 1–50.
    pub fn list_verifiers_paginated(env: Env, page: u32, page_size: u32) -> Vec<Address> {
        let page_size = if page_size == 0 || page_size > 50 { 50 } else { page_size };
        let all = get_verifiers(&env);
        let start = page * page_size;
        let mut out: Vec<Address> = Vec::new(&env);
        let mut i = start;
        while i < start + page_size && i < all.len() {
            out.push_back(all.get(i).unwrap());
            i += 1;
        }
        out
    }

    // ── Credit lifecycle ─────────────────────────────────────────────────────

    /// Submit a new carbon credit for verifier approval.
    ///
    /// Stores the credit with [`CreditStatus::Pending`] and returns its deterministic ID
    /// (SHA-256 of `project_id || internal_nonce`). The credit cannot be traded or retired
    /// until a registered verifier calls [`approve_and_mint`].
    ///
    /// `tonnes` is expressed in kg units (1 tonne = 1 000 000). Valid range: `1..=1_000_000_000_000_000`.
    ///
    /// # Errors
    /// - [`CarbonChainError::NotInitialized`] — contract has not been initialised.
    /// - [`CarbonChainError::ContractPaused`] — contract is paused.
    /// - [`CarbonChainError::InvalidNonce`] — `nonce` does not match the current issuer nonce.
    /// - [`CarbonChainError::InvalidTonnes`] — `tonnes` is zero, negative, or exceeds the upper bound.
    pub fn submit_credit(
        env: Env,
        issuer: Address,
        project_id: String,
        vintage_year: u32,
        methodology: String,
        geography: String,
        tonnes: i128,
        ipfs_hash: String,
        nonce: u64,
    ) -> Result<BytesN<32>, CarbonChainError> {
        if !has_admin(&env) {
            return Err(CarbonChainError::NotInitialized);
        }
        if is_paused(&env) {
            return Err(CarbonChainError::ContractPaused);
        }
        issuer.require_auth();
        if !consume_nonce(&env, &issuer, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        if tonnes <= 0 {
            return Err(CarbonChainError::InvalidTonnes);
        }
        // 1 billion tonnes upper bound (1_000_000_000 * TONNES_SCALE = 1e15)
        if tonnes > 1_000_000_000_000_000 {
            return Err(CarbonChainError::InvalidTonnes);
        }
        // Validate vintage_year: 1990 to current_year + 1
        let current_year = (env.ledger().timestamp() / 31_536_000) as u32 + 1970;
        if vintage_year < 1990 || vintage_year > current_year + 1 {
            return Err(CarbonChainError::InvalidMetadata);
        }
        // Validate geography: minimum 2 characters (ISO 3166-1 alpha-2)
        if geography.len() < 2 {
            return Err(CarbonChainError::InvalidMetadata);
        }

        // Include a per-contract nonce so two credits for the same project get distinct IDs.
        let nonce: u64 = env.storage().instance().get(&DataKey::CreditNonce).unwrap_or(0u64);
        env.storage().instance().set(&DataKey::CreditNonce, &(nonce + 1));
        let mut preimage = project_id.clone().to_xdr(&env);
        preimage.append(&nonce.to_xdr(&env));
        let id: BytesN<32> = env.crypto().sha256(&preimage).into();
        let metadata = CreditMetadata {
            project_id: project_id.clone(),
            issuer: issuer.clone(),
            owner: issuer.clone(),
            vintage_year,
            methodology,
            geography,
            tonnes,
            ipfs_hash,
            status: CreditStatus::Pending,
            issued_at: env.ledger().timestamp(),
        };

        set_credit(&env, &id, &metadata);
        add_credit_to_project(&env, &project_id, &id);
        credit_submitted(&env, issuer, project_id, tonnes);

        Ok(id)
    }

    pub fn approve_and_mint(env: Env, verifier: Address, credit_id: BytesN<32>, nonce: u64) -> Result<(), CarbonChainError> {
        if is_paused(&env) {
            return Err(CarbonChainError::ContractPaused);
        }
        verifier.require_auth();
        if !is_verifier(&env, &verifier) {
            return Err(CarbonChainError::Unauthorized);
        }
        if !consume_nonce(&env, &verifier, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        let mut credit = get_credit(&env, &credit_id).ok_or(CarbonChainError::CreditNotFound)?;
        if credit.status != CreditStatus::Pending {
            return Err(CarbonChainError::InvalidStatusTransition);
        }
        credit.status = CreditStatus::Active;
        set_credit(&env, &credit_id, &credit);
        credit_minted(&env, verifier, credit_id);
        Ok(())
    }

    /// Flag a credit for re-review, transitioning it to [`CreditStatus::Flagged`].
    ///
    /// Only a registered verifier may flag a credit. Credits that are already
    /// [`CreditStatus::Retired`] or [`CreditStatus::Flagged`] cannot be flagged again.
    ///
    /// # Errors
    /// - [`CarbonChainError::ContractPaused`] — contract is paused.
    /// - [`CarbonChainError::Unauthorized`] — `verifier` is not in the registered verifier set.
    /// - [`CarbonChainError::InvalidNonce`] — `nonce` does not match the current verifier nonce.
    /// - [`CarbonChainError::CreditNotFound`] — no credit exists for `credit_id`.
    /// - [`CarbonChainError::InvalidStatusTransition`] — credit is already `Retired` or `Flagged`.
    pub fn flag_credit(env: Env, verifier: Address, credit_id: BytesN<32>, reason: String) -> Result<(), CarbonChainError> {
        if is_paused(&env) {
            return Err(CarbonChainError::ContractPaused);
        }
        verifier.require_auth();
        if !is_verifier(&env, &verifier) {
            return Err(CarbonChainError::Unauthorized);
        }
        if !consume_nonce(&env, &verifier, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        let mut credit = get_credit(&env, &credit_id).ok_or(CarbonChainError::CreditNotFound)?;
        if credit.status == CreditStatus::Retired || credit.status == CreditStatus::Flagged {
            return Err(CarbonChainError::InvalidStatusTransition);
        }
        credit.status = CreditStatus::Flagged;
        set_credit(&env, &credit_id, &credit);
        crate::events::credit_flagged(&env, credit_id, reason);
        Ok(())
    }

    /// Mark a credit as retired. Only callable by the registered retirement contract.
    ///
    /// This is an internal cross-contract call made by the retirement contract after
    /// recording the retirement receipt. The credit must be [`CreditStatus::Active`].
    ///
    /// # Errors
    /// - [`CarbonChainError::ContractPaused`] — contract is paused.
    /// - [`CarbonChainError::NotInitialized`] — no retirement contract has been registered.
    /// - [`CarbonChainError::CreditNotFound`] — no credit exists for `credit_id`.
    /// - [`CarbonChainError::InvalidStatusTransition`] — credit is not in `Active` status.
    pub fn mark_retired(env: Env, credit_id: BytesN<32>) -> Result<(), CarbonChainError> {
        if is_paused(&env) {
            return Err(CarbonChainError::ContractPaused);
        }
        // Only the registered retirement contract may call this.
        let retirement_contract = get_retirement_contract(&env)
            .ok_or(CarbonChainError::NotInitialized)?;
        retirement_contract.require_auth();
        let mut credit = get_credit(&env, &credit_id).ok_or(CarbonChainError::CreditNotFound)?;
        if credit.status != CreditStatus::Active {
            return Err(CarbonChainError::InvalidStatusTransition);
        }
        credit.status = CreditStatus::Retired;
        set_credit(&env, &credit_id, &credit);
        Ok(())
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    /// Fetch full metadata for a credit by its ID.
    ///
    /// # Errors
    /// - [`CarbonChainError::CreditNotFound`] — no credit exists for `credit_id`.
    pub fn get_credit(env: Env, credit_id: BytesN<32>) -> Result<CreditMetadata, CarbonChainError> {
        get_credit(&env, &credit_id).ok_or(CarbonChainError::CreditNotFound)
    }

    /// Returns all credit IDs registered under `project_id`.
    pub fn list_credits_by_project(env: Env, project_id: String) -> Vec<BytesN<32>> {
        get_credits_by_project(&env, &project_id)
    }

    /// Returns the current replay-protection nonce for `address`.
    /// Pass this value as the `nonce` argument to the next state-mutating call.
    pub fn get_nonce(env: Env, address: Address) -> u64 {
        get_nonce(&env, &address)
    }

    /// Propose a new admin. The current admin initiates the transfer; the
    /// candidate must call [`accept_admin`] to complete it.
    ///
    /// # Errors
    /// - [`CarbonChainError::NotInitialized`] — contract has not been initialised.
    /// - [`CarbonChainError::Unauthorized`] — caller is not the current admin.
    pub fn propose_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        env.storage().instance().set(&crate::types::DataKey::PendingAdmin, &new_admin);
        Ok(())
    }

    /// Complete an admin transfer initiated by [`propose_admin`].
    /// `new_admin` must match the pending candidate.
    ///
    /// # Errors
    /// - [`CarbonChainError::NoPendingAdmin`] — no transfer has been proposed.
    /// - [`CarbonChainError::Unauthorized`] — `new_admin` does not match the pending candidate.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), CarbonChainError> {
        let pending: Address = env
            .storage().instance()
            .get(&crate::types::DataKey::PendingAdmin)
            .ok_or(CarbonChainError::NoPendingAdmin)?;
        if new_admin != pending {
            return Err(CarbonChainError::Unauthorized);
        }
        new_admin.require_auth();
        set_admin(&env, &new_admin);
        env.storage().instance().remove(&crate::types::DataKey::PendingAdmin);
        Ok(())
    }

    /// Returns `true` if `address` is a registered verifier.
    pub fn is_verifier(env: Env, address: Address) -> bool {
        is_verifier(&env, &address)
    }

    // ── Verifier Services ────────────────────────────────────────────────────

    /// Replace all capabilities for a verifier. This overwrites any existing services.
    pub fn configure_verifier_services(env: Env, admin: Address, verifier: Address, services: Vec<ServiceType>, nonce: u64) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        if !consume_nonce(&env, &admin, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        if !is_verifier(&env, &verifier) {
            return Err(CarbonChainError::VerifierNotFound);
        }
        env.storage().persistent().set(&DataKey::VerifierServices(verifier), &services);
        Ok(())
    }

    /// Add a single service to a verifier's capabilities.
    pub fn add_verifier_service(env: Env, admin: Address, verifier: Address, service: ServiceType, nonce: u64) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        if !consume_nonce(&env, &admin, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        if !is_verifier(&env, &verifier) {
            return Err(CarbonChainError::VerifierNotFound);
        }
        
        let mut services: Vec<ServiceType> = env.storage().persistent()
            .get(&DataKey::VerifierServices(verifier.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        
        if !services.contains(&service) {
            services.push_back(service);
            env.storage().persistent().set(&DataKey::VerifierServices(verifier), &services);
        }
        Ok(())
    }

    /// Remove a single service from a verifier's capabilities.
    pub fn remove_verifier_service(env: Env, admin: Address, verifier: Address, service: ServiceType, nonce: u64) -> Result<(), CarbonChainError> {
        let stored_admin = get_admin(&env).ok_or(CarbonChainError::NotInitialized)?;
        admin.require_auth();
        if admin != stored_admin {
            return Err(CarbonChainError::Unauthorized);
        }
        if !consume_nonce(&env, &admin, nonce) {
            return Err(CarbonChainError::InvalidNonce);
        }
        if !is_verifier(&env, &verifier) {
            return Err(CarbonChainError::VerifierNotFound);
        }
        
        let old_services: Vec<ServiceType> = env.storage().persistent()
            .get(&DataKey::VerifierServices(verifier.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        
        let mut new_services: Vec<ServiceType> = Vec::new(&env);
        for s in old_services.iter() {
            if s != service {
                new_services.push_back(s);
            }
        }
        env.storage().persistent().set(&DataKey::VerifierServices(verifier), &new_services);
        Ok(())
    }

    pub fn get_verifier_services(env: Env, verifier: Address) -> Vec<ServiceType> {
        env.storage().persistent()
            .get(&DataKey::VerifierServices(verifier))
            .unwrap_or_else(|| Vec::new(&env))
    }
}

// ── Helper functions ─────────────────────────────────────────────────────────

fn get_nonce(env: &Env, addr: &Address) -> u64 {
    env.storage().persistent().get(&DataKey::Nonce(addr.clone())).unwrap_or(0u64)
}

fn consume_nonce(env: &Env, addr: &Address, expected: u64) -> bool {
    let current = get_nonce(env, addr);
    if current != expected { return false; }
    let key = DataKey::Nonce(addr.clone());
    env.storage().persistent().set(&key, &(current + 1));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Env, String};

    fn setup() -> (Env, CreditRegistryClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreditRegistry, ());
        let client = CreditRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let verifier = Address::generate(&env);
        let retirement = Address::generate(&env);
        client.initialize(&admin, &retirement);
        (env, client, admin, verifier)
    }

    fn submit_test_credit(env: &Env, client: &CreditRegistryClient, issuer: &Address) -> BytesN<32> {
        let nonce = client.get_nonce(issuer);
        client.submit_credit(
            issuer,
            &String::from_str(env, "PROJ-001"),
            &2024,
            &String::from_str(env, "VCS"),
            &String::from_str(env, "NG"),
            &1_000_000,
            &String::from_str(env, "bafybei123"),
            &nonce,
        )
    }

    #[test]
    fn test_initialize_twice_fails() {
        let (env, client, admin, _) = setup();
        let retirement = Address::generate(&env);
        let result = client.try_initialize(&admin, &retirement);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_and_list_verifier() {
        let (_env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let list = client.list_verifiers();
        assert_eq!(list.len(), 1);
        assert_eq!(list.get(0).unwrap(), verifier);
    }

    #[test]
    fn test_register_verifier_twice_fails() {
        let (_env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let nonce2 = client.get_nonce(&admin);
        let result = client.try_register_verifier(&admin, &verifier, &nonce2);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_verifier() {
        let (_env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let nonce2 = client.get_nonce(&admin);
        client.remove_verifier(&admin, &verifier, &nonce2);
        assert_eq!(client.list_verifiers().len(), 0);
    }

    #[test]
    fn test_submit_credit() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let credit = client.get_credit(&id);
        assert_eq!(credit.status, CreditStatus::Pending);
        assert_eq!(credit.tonnes, 1_000_000);
    }

    #[test]
    fn test_approve_and_mint() {
        let (env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let vnonce = client.get_nonce(&verifier);
        client.approve_and_mint(&verifier, &id, &vnonce);
        assert_eq!(client.get_credit(&id).status, CreditStatus::Active);
    }

    #[test]
    fn test_approve_non_pending_fails() {
        let (env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let vnonce = client.get_nonce(&verifier);
        client.approve_and_mint(&verifier, &id, &vnonce);
        // second approval should fail
        let vnonce2 = client.get_nonce(&verifier);
        let result = client.try_approve_and_mint(&verifier, &id, &vnonce2);
        assert!(result.is_err());
    }

    #[test]
    fn test_flag_credit() {
        let (env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let vnonce = client.get_nonce(&verifier);
        client.flag_credit(&verifier, &id, &String::from_str(&env, "suspicious data"), &vnonce);
        assert_eq!(client.get_credit(&id).status, CreditStatus::Flagged);
    }

    #[test]
    fn test_double_flag_fails() {
        let (env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let vnonce = client.get_nonce(&verifier);
        client.flag_credit(&verifier, &id, &String::from_str(&env, "first flag"), &vnonce);
        let vnonce2 = client.get_nonce(&verifier);
        let result = client.try_flag_credit(&verifier, &id, &String::from_str(&env, "second flag"), &vnonce2);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_verifiers_paginated() {
        let (env, client, admin, _) = setup();
        let mut addrs = soroban_sdk::Vec::new(&env);
        for _ in 0..5u32 {
            let v = Address::generate(&env);
            let nonce = client.get_nonce(&admin);
            client.register_verifier(&admin, &v, &nonce);
            addrs.push_back(v);
        }
        // page 0, size 2 → first 2
        let p0 = client.list_verifiers_paginated(&0, &2);
        assert_eq!(p0.len(), 2);
        assert_eq!(p0.get(0).unwrap(), addrs.get(0).unwrap());
        // page 1, size 2 → next 2
        let p1 = client.list_verifiers_paginated(&1, &2);
        assert_eq!(p1.len(), 2);
        assert_eq!(p1.get(0).unwrap(), addrs.get(2).unwrap());
        // page 2, size 2 → last 1
        let p2 = client.list_verifiers_paginated(&2, &2);
        assert_eq!(p2.len(), 1);
    }

    #[test]
    fn test_mark_retired() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreditRegistry, ());
        let client = CreditRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let verifier = Address::generate(&env);
        let retirement = Address::generate(&env);
        client.initialize(&admin, &retirement);
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let vnonce = client.get_nonce(&verifier);
        client.approve_and_mint(&verifier, &id, &vnonce);
        client.mark_retired(&id);
        assert_eq!(client.get_credit(&id).status, CreditStatus::Retired);
    }

    #[test]
    fn test_unauthorized_mark_retired_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(CreditRegistry, ());
        let client = CreditRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let verifier = Address::generate(&env);
        let retirement = Address::generate(&env);

        client.initialize(&admin, &retirement);
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let vnonce = client.get_nonce(&verifier);
        client.approve_and_mint(&verifier, &id, &vnonce);

        env.set_auths(&[]);
        let result = client.try_mark_retired(&id);
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_credit_zero_tonnes_fails() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &0,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_credit_negative_tonnes_fails() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &-1,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_credit_over_upper_bound_fails() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000_000_000_001,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_credit_at_upper_bound_succeeds() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000_000_000_000,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_credit_returns_error_for_missing_credit() {
        let (env, client, _, _) = setup();
        let fake_id = BytesN::from_array(&env, &[0u8; 32]);
        let result = client.try_get_credit(&fake_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_credit_returns_credit_metadata() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let credit = client.get_credit(&id);
        assert_eq!(credit.tonnes, 1_000_000);
        assert_eq!(credit.status, CreditStatus::Pending);
    }

    #[test]
    fn test_list_credits_by_project() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        submit_test_credit(&env, &client, &issuer);
        let ids = client.list_credits_by_project(&String::from_str(&env, "PROJ-001"));
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn test_non_verifier_cannot_approve() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        let fake = Address::generate(&env);
        let nonce = client.get_nonce(&fake);
        let result = client.try_approve_and_mint(&fake, &id, &nonce);
        assert!(result.is_err());
    }

    // ── Pause tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_pause_blocks_submit_credit() {
        let (env, client, admin, _) = setup();
        client.pause(&admin);
        assert!(client.paused());
        let issuer = Address::generate(&env);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_unpause_restores_submit_credit() {
        let (env, client, admin, _) = setup();
        client.pause(&admin);
        client.unpause(&admin);
        assert!(!client.paused());
        let issuer = Address::generate(&env);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_pause_blocks_approve_and_mint() {
        let (env, client, admin, verifier) = setup();
        let nonce = client.get_nonce(&admin);
        client.register_verifier(&admin, &verifier, &nonce);
        let issuer = Address::generate(&env);
        let id = submit_test_credit(&env, &client, &issuer);
        client.pause(&admin);
        let vnonce = client.get_nonce(&verifier);
        assert!(client.try_approve_and_mint(&verifier, &id, &vnonce).is_err());
    }

    #[test]
    fn test_non_admin_cannot_pause() {
        let (env, client, _, _) = setup();
        let rando = Address::generate(&env);
        assert!(client.try_pause(&rando).is_err());
    }

    #[test]
    fn test_submit_credit_vintage_year_too_old_fails() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &1989,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_credit_vintage_year_valid_1990_succeeds() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &1990,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_submit_credit_empty_geography_fails() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, ""),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_credit_geography_single_char_fails() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "N"),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_credit_geography_valid_iso_succeeds() {
        let (env, client, _, _) = setup();
        let issuer = Address::generate(&env);
        let nonce = client.get_nonce(&issuer);
        let result = client.try_submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
            &nonce,
        );
        assert!(result.is_ok());
    }
}
