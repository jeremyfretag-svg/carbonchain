#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contracterror, symbol_short, Env, Address, BytesN, Symbol, Vec, IntoVal};

// ── TTL constants ─────────────────────────────────────────────────────────────
/// Minimum TTL in ledgers (~1 year at 5s/ledger).
const MIN_TTL: u32 = 6_307_200;
/// Threshold below which TTL is extended.
const TTL_THRESHOLD: u32 = MIN_TTL / 2;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Offer {
    pub seller: Address,
    pub credit_id: BytesN<32>,
    pub price_xlm: i128,   // in stroops
    pub tonnes: i128,
    pub active: bool,
    pub created_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Offer(u64),
    OfferCount,
    SellerOffers(Address),
    Nonce(Address),
    Admin,
    PendingAdmin,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketplaceError {
    OfferNotFound  = 115,
    Unauthorized   = 116,
    InvalidPrice   = 117,
    AlreadyClosed  = 118,
    CreditNotActive = 119,
    InvalidNonce   = 120,
    NotInitialized = 121,
    AlreadyInitialized = 122,
    NoPendingAdmin = 123,
}

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct Marketplace;

#[contractimpl]
impl Marketplace {
    fn read_nonce(env: &Env, addr: &Address) -> u64 {
        env.storage().persistent().get(&DataKey::Nonce(addr.clone())).unwrap_or(0u64)
    }

    fn consume_nonce(env: &Env, addr: &Address, expected: u64) -> bool {
        let current = Self::read_nonce(env, addr);
        if current != expected { return false; }
        let key = DataKey::Nonce(addr.clone());
        env.storage().persistent().set(&key, &(current + 1));
        env.storage().persistent().extend_ttl(&key, TTL_THRESHOLD, MIN_TTL);
        true
    }

    pub fn nonce(env: Env, address: Address) -> u64 {
        Self::read_nonce(&env, &address)
    }

    /// List a credit for sale. Returns the new offer ID.
    pub fn create_offer(
        env: Env,
        seller: Address,
        credit_id: BytesN<32>,
        price_xlm: i128,
        tonnes: i128,
        registry_id: Address,
        nonce: u64,
    ) -> Result<u64, MarketplaceError> {
        seller.require_auth();
        if !Self::consume_nonce(&env, &seller, nonce) {
            return Err(MarketplaceError::InvalidNonce);
        }
        if price_xlm <= 0 || tonnes <= 0 {
            return Err(MarketplaceError::InvalidPrice);
        }

        // Validate credit exists and is Active in the registry
        let credit: carbonchain_credit_registry::types::CreditMetadata = env.invoke_contract(
            &registry_id,
            &Symbol::new(&env, "get_credit"),
            (credit_id.clone(),).into_val(&env),
        );
        if credit.status != carbonchain_credit_registry::types::CreditStatus::Active {
            return Err(MarketplaceError::CreditNotActive);
        }

        let offer_id = Self::next_id(&env);
        let offer = Offer {
            seller: seller.clone(),
            credit_id,
            price_xlm,
            tonnes,
            active: true,
            created_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&DataKey::Offer(offer_id), &offer);
        env.storage().persistent().extend_ttl(&DataKey::Offer(offer_id), TTL_THRESHOLD, MIN_TTL);

        // Index under seller
        let key = DataKey::SellerOffers(seller.clone());
        let mut ids: Vec<u64> = env.storage().persistent().get(&key).unwrap_or_else(|| Vec::new(&env));
        ids.push_back(offer_id);
        env.storage().persistent().set(&key, &ids);
        env.storage().persistent().extend_ttl(&key, TTL_THRESHOLD, MIN_TTL);

        env.events().publish((symbol_short!("offer_new"), seller), offer_id);
        Ok(offer_id)
    }

    /// Cancel an open offer. Only the original seller may cancel.
    pub fn cancel_offer(env: Env, seller: Address, offer_id: u64, nonce: u64) -> Result<(), MarketplaceError> {
        seller.require_auth();
        if !Self::consume_nonce(&env, &seller, nonce) {
            return Err(MarketplaceError::InvalidNonce);
        }
        let mut offer: Offer = env
            .storage()
            .persistent()
            .get(&DataKey::Offer(offer_id))
            .ok_or(MarketplaceError::OfferNotFound)?;

        if offer.seller != seller {
            return Err(MarketplaceError::Unauthorized);
        }
        if !offer.active {
            return Err(MarketplaceError::AlreadyClosed);
        }

        offer.active = false;
        env.storage().persistent().set(&DataKey::Offer(offer_id), &offer);
        env.storage().persistent().extend_ttl(&DataKey::Offer(offer_id), TTL_THRESHOLD, MIN_TTL);
        env.events().publish((symbol_short!("offer_cxl"), seller), offer_id);
        Ok(())
    }

    pub fn get_offer(env: Env, offer_id: u64) -> Result<Offer, MarketplaceError> {
        env.storage()
            .persistent()
            .get(&DataKey::Offer(offer_id))
            .ok_or(MarketplaceError::OfferNotFound)
    }

    pub fn get_offers_by_seller(env: Env, seller: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::SellerOffers(seller))
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn offer_count(env: Env) -> u64 {
        env.storage().persistent().get(&DataKey::OfferCount).unwrap_or(0u64)
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn next_id(env: &Env) -> u64 {
        let id: u64 = env.storage().persistent().get(&DataKey::OfferCount).unwrap_or(0u64);
        env.storage().persistent().set(&DataKey::OfferCount, &(id + 1));
        env.storage().persistent().extend_ttl(&DataKey::OfferCount, TTL_THRESHOLD, MIN_TTL);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Env, BytesN, String};
    use carbonchain_credit_registry::CreditRegistry;

    fn setup_with_registry(env: &Env) -> (MarketplaceClient<'static>, Address, Address, BytesN<32>) {
        let registry_id = env.register(CreditRegistry, ());
        let registry_client = carbonchain_credit_registry::CreditRegistryClient::new(env, &registry_id);

        let admin = Address::generate(env);
        let verifier = Address::generate(env);
        let issuer = Address::generate(env);
        let retirement = Address::generate(env);
        registry_client.initialize(&admin, &retirement);
        let nonce = registry_client.nonce(&admin);
        registry_client.register_verifier(&admin, &verifier, &nonce);

        let inonce = registry_client.nonce(&issuer);
        let credit_id = registry_client.submit_credit(
            &issuer,
            &String::from_str(env, "PROJ-001"),
            &2024,
            &String::from_str(env, "VCS"),
            &String::from_str(env, "NG"),
            &1_000_000,
            &String::from_str(env, "bafybei123"),
            &inonce,
        );
        let vnonce = registry_client.nonce(&verifier);
        registry_client.approve_and_mint(&verifier, &credit_id, &vnonce);

        let marketplace_id = env.register(Marketplace, ());
        let client = MarketplaceClient::new(env, &marketplace_id);
        let seller = Address::generate(env);
        (client, seller, registry_id, credit_id)
    }

    #[test]
    fn test_create_offer() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        let offer_id = client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        assert_eq!(offer_id, 0);
        let offer = client.get_offer(&offer_id);
        assert!(offer.active);
        assert_eq!(offer.price_xlm, 10_000_000);
    }

    #[test]
    fn test_cancel_offer() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        let offer_id = client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        let nonce2 = client.nonce(&seller);
        client.cancel_offer(&seller, &offer_id, &nonce2);
        assert!(!client.get_offer(&offer_id).active);
    }

    #[test]
    fn test_cancel_already_closed_emits_no_event() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        let offer_id = client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        let nonce2 = client.nonce(&seller);
        client.cancel_offer(&seller, &offer_id, &nonce2);
        let count_before = env.events().all().len();
        let nonce3 = client.nonce(&seller);
        let _ = client.try_cancel_offer(&seller, &offer_id, &nonce3);
        assert_eq!(env.events().all().len(), count_before);
    }

    #[test]
    fn test_cancel_already_closed_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        let offer_id = client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        let nonce2 = client.nonce(&seller);
        client.cancel_offer(&seller, &offer_id, &nonce2);
        let nonce3 = client.nonce(&seller);
        assert!(client.try_cancel_offer(&seller, &offer_id, &nonce3).is_err());
    }

    #[test]
    fn test_invalid_price_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        assert!(client.try_create_offer(&seller, &credit_id, &0, &500_000, &registry_id, &nonce).is_err());
    }

    #[test]
    fn test_get_offers_by_seller() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        let nonce2 = client.nonce(&seller);
        client.create_offer(&seller, &credit_id, &20_000_000, &250_000, &registry_id, &nonce2);
        assert_eq!(client.get_offers_by_seller(&seller).len(), 2);
    }

    #[test]
    fn test_offer_count() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        let nonce2 = client.nonce(&seller);
        client.create_offer(&seller, &credit_id, &20_000_000, &250_000, &registry_id, &nonce2);
        assert_eq!(client.offer_count(), 2);
    }

    #[test]
    fn test_unauthorized_cancel_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, seller, registry_id, credit_id) = setup_with_registry(&env);
        let nonce = client.nonce(&seller);
        let offer_id = client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        let other = Address::generate(&env);
        let ononce = client.nonce(&other);
        assert!(client.try_cancel_offer(&other, &offer_id, &ononce).is_err());
    }

    #[test]
    fn test_offer_count_survives_contract_reinstantiation() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Marketplace, ());
        let client = MarketplaceClient::new(&env, &contract_id);
        let seller = Address::generate(&env);
        let credit_id = BytesN::from_array(&env, &[1u8; 32]);
        let registry_id = Address::generate(&env);

        let nonce = client.nonce(&seller);
        client.create_offer(&seller, &credit_id, &10_000_000, &500_000, &registry_id, &nonce);
        let nonce2 = client.nonce(&seller);
        client.create_offer(&seller, &credit_id, &20_000_000, &250_000, &registry_id, &nonce2);
        assert_eq!(client.offer_count(), 2);

        env.register_at(&contract_id, Marketplace, ());
        assert_eq!(client.offer_count(), 2);

        let nonce3 = client.nonce(&seller);
        let new_offer_id = client.create_offer(&seller, &credit_id, &5_000_000, &100_000, &registry_id, &nonce3);
        assert_eq!(new_offer_id, 2);
    }
}
