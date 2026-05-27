#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Env, Address, String, Vec, Symbol, IntoVal,
};

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MrvDataPoint {
    pub oracle: Address,
    pub project_id: String,
    /// Carbon sequestration in scaled units. 1 tonne = 1_000_000 units.
    pub tonnes: i128,
    pub recorded_at: u64,
    /// Flagged when the reading deviates >20% from the previous reading.
    pub anomaly: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Admin address allowed to register oracles.
    Admin,
    /// Set of authorised oracle addresses.
    OracleSet,
    /// Latest reading per project.
    Latest(String),
    /// Full history per project (Vec<MrvDataPoint>).
    History(String),
    /// Pause flag.
    Paused,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    NotInitialized     = 119,
    Unauthorized       = 120,
    AlreadyInitialized = 121,
    Overflow           = 122,
    ContractPaused     = 123,
    ProjectNotFound    = 124,
}

// Maximum MRV history entries retained per project (ring-buffer eviction).
const MAX_HISTORY: u32 = 100;

/// Minimum TTL in ledgers (~1 year at 5s/ledger).
const MIN_TTL: u32 = 6_307_200;
/// Threshold below which TTL is extended.
const TTL_THRESHOLD: u32 = MIN_TTL / 2;

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct MrvOracle;

#[contractimpl]
impl MrvOracle {
    /// Initialise the oracle contract. Must be called exactly once.
    ///
    /// # Errors
    /// - [`OracleError::AlreadyInitialized`] — contract has already been initialised.
    pub fn initialize(env: Env, admin: Address) -> Result<(), OracleError> {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.events().publish((symbol_short!("mrv_init"),), admin);
        Ok(())
    }

    // ── Pause / Unpause ──────────────────────────────────────────────────────

    /// Pause all state-mutating operations. Only the admin may call this.
    ///
    /// # Errors
    /// - [`OracleError::NotInitialized`] — contract has not been initialised.
    /// - [`OracleError::Unauthorized`] — caller is not the admin.
    pub fn pause(env: Env, admin: Address) -> Result<(), OracleError> {
        env.events().publish((symbol_short!("paused"),), admin);
        Ok(())
    }

    /// Resume all state-mutating operations. Only the admin may call this.
    ///
    /// # Errors
    /// - [`OracleError::NotInitialized`] — contract has not been initialised.
    /// - [`OracleError::Unauthorized`] — caller is not the admin.
    pub fn unpause(env: Env, admin: Address) -> Result<(), OracleError> {
        env.events().publish((symbol_short!("unpaused"),), admin);
        Ok(())
    }

    /// Returns `true` if the contract is currently paused.
    pub fn paused(env: Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    // ── Oracle management ────────────────────────────────────────────────────

    /// Register an oracle address. Returns `true` if newly added, `false` if already registered.
    ///
    /// Emits `orc_new` on first registration and `orc_dup` on a duplicate, so callers can
    /// distinguish the two cases from on-chain events.
    ///
    /// # Errors
    /// - [`OracleError::NotInitialized`] — contract has not been initialised.
    /// - [`OracleError::Unauthorized`] — caller is not the admin.
    /// - [`OracleError::InvalidNonce`] — `nonce` does not match the current admin nonce.
    pub fn register_oracle(env: Env, admin: Address, oracle: Address) -> Result<bool, OracleError> {
        Self::require_admin(&env, &admin)?;
        if !Self::consume_nonce(&env, &admin, nonce) {
            return Err(OracleError::InvalidNonce);
        }
        let mut set: Vec<Address> = env
            .storage().instance()
            .get(&DataKey::OracleSet)
            .unwrap_or_else(|| Vec::new(&env));
        if set.contains(&oracle) {
            // Already registered — emit a distinct event so callers know.
            env.events().publish((symbol_short!("orc_dup"),), oracle);
            return Ok(false);
        }
        set.push_back(oracle.clone());
        env.storage().instance().set(&DataKey::OracleSet, &set);
        env.events().publish((symbol_short!("orc_new"),), oracle);
        Ok(true)
    }

    /// Submit a new MRV reading for a project. Returns `true` if an anomaly was detected.
    ///
    /// An anomaly is flagged when the new reading deviates more than 20% from the previous one.
    /// The reading is stored as the latest value and appended to the project's history
    /// (capped at 100 entries; oldest entry is evicted when the cap is reached).
    ///
    /// # Errors
    /// - [`OracleError::ContractPaused`] — contract is paused.
    /// - [`OracleError::Unauthorized`] — `oracle` is not a registered oracle address.
    /// - [`OracleError::InvalidNonce`] — `nonce` does not match the current oracle nonce.
    /// - [`OracleError::Overflow`] — anomaly calculation overflowed (extremely large `tonnes` value).
    pub fn update_mrv_data(
        env: Env,
        oracle: Address,
        project_id: String,
        tonnes: i128,
        registry_id: Address,
        nonce: u64,
    ) -> Result<bool, OracleError> {
        if Self::is_paused(&env) {
            return Err(OracleError::ContractPaused);
        }
        oracle.require_auth();
        if !Self::is_oracle(&env, &oracle) {
            return Err(OracleError::Unauthorized);
        }
        if !Self::consume_nonce(&env, &oracle, nonce) {
            return Err(OracleError::InvalidNonce);
        }

        // Validate project exists in registry
        let credits: soroban_sdk::Vec<soroban_sdk::BytesN<32>> = env.invoke_contract(
            &registry_id,
            &soroban_sdk::Symbol::new(&env, "list_credits_by_project"),
            (project_id.clone(),).into_val(&env),
        );
        if credits.is_empty() {
            return Err(OracleError::InvalidProject);
        }

        let anomaly = Self::detect_anomaly(&env, &project_id, tonnes)?;

        let point = MrvDataPoint {
            oracle: oracle.clone(),
            project_id: project_id.clone(),
            tonnes,
            recorded_at: env.ledger().timestamp(),
            anomaly,
        };

        env.storage().persistent().set(&DataKey::Latest(project_id.clone()), &point);
        env.storage().persistent().extend_ttl(&DataKey::Latest(project_id.clone()), TTL_THRESHOLD, MIN_TTL);

        let hist_key = DataKey::History(project_id.clone());
        let mut history: Vec<MrvDataPoint> = env
            .storage().persistent()
            .get(&hist_key)
            .unwrap_or_else(|| Vec::new(&env));
        if history.len() >= MAX_HISTORY {
            // Evict oldest entry (index 0) to keep the ring buffer bounded.
            history.remove(0);
        }
        history.push_back(point);
        env.storage().persistent().set(&hist_key, &history);
        env.storage().persistent().extend_ttl(&hist_key, TTL_THRESHOLD, MIN_TTL);

        env.events().publish(
            (symbol_short!("mrv_upd"), oracle),
            (project_id, tonnes, anomaly),
        );

        Ok(anomaly)
    }

    pub fn get_latest(env: Env, project_id: String) -> Result<Option<MrvDataPoint>, OracleError> {
        // Check if project exists by looking for any history
        let has_history = env.storage().persistent().has(&DataKey::History(project_id.clone()));
        let has_latest = env.storage().persistent().has(&DataKey::Latest(project_id.clone()));
        
        if !has_history && !has_latest {
            return Err(OracleError::ProjectNotFound);
        }
        
        Ok(env.storage().persistent().get(&DataKey::Latest(project_id)))
    }

    /// Returns the current replay-protection nonce for `address`.
    pub fn get_nonce(env: Env, address: Address) -> u64 {
        env.storage().persistent().get(&DataKey::Nonce(address)).unwrap_or(0u64)
    }

    /// Propose a new admin. The candidate must call [`accept_admin`] to complete the transfer.
    ///
    /// # Errors
    /// - [`OracleError::NotInitialized`] — contract has not been initialised.
    /// - [`OracleError::Unauthorized`] — caller is not the current admin.
    pub fn propose_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), OracleError> {
        Self::require_admin(&env, &admin)?;
        env.storage().instance().set(&DataKey::PendingAdmin, &new_admin);
        Ok(())
    }

    /// Complete an admin transfer initiated by [`propose_admin`].
    ///
    /// # Errors
    /// - [`OracleError::NoPendingAdmin`] — no transfer has been proposed.
    /// - [`OracleError::Unauthorized`] — `new_admin` does not match the pending candidate.
    pub fn accept_admin(env: Env, new_admin: Address) -> Result<(), OracleError> {
        if new_admin != pending {
            return Err(OracleError::Unauthorized);
        }
        new_admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.storage().instance().remove(&DataKey::PendingAdmin);
        Ok(())
    }

    /// Returns the full MRV history for `project_id` (up to 100 entries).
    pub fn get_history(env: Env, project_id: String) -> Vec<MrvDataPoint> {
        env.storage()
            .persistent()
            .get(&DataKey::History(project_id))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Clear the anomaly flag on the latest MRV reading for a project.
    /// Only authorized verifiers or admin may call this after reviewing the data.
    pub fn clear_anomaly_flag(env: Env, verifier: Address, project_id: String, nonce: u64) -> Result<(), OracleError> {
        if Self::is_paused(&env) {
            return Err(OracleError::ContractPaused);
        }
        verifier.require_auth();
        if !Self::is_oracle(&env, &verifier) {
            return Err(OracleError::Unauthorized);
        }
        if !Self::consume_nonce(&env, &verifier, nonce) {
            return Err(OracleError::InvalidNonce);
        }

        let mut point: MrvDataPoint = env
            .storage().persistent()
            .get(&DataKey::Latest(project_id.clone()))
            .ok_or(OracleError::NotInitialized)?;

        point.anomaly = false;
        env.storage().persistent().set(&DataKey::Latest(project_id.clone()), &point);
        env.storage().persistent().extend_ttl(&DataKey::Latest(project_id.clone()), TTL_THRESHOLD, MIN_TTL);

        env.events().publish((symbol_short!("anom_clr"), verifier), project_id);
        Ok(())
    }

    // ── Internal ─────────────────────────────────────────────────────────────

    fn require_admin(env: &Env, caller: &Address) -> Result<(), OracleError> {
        let admin: Address = env
            .storage().instance()
            .get(&DataKey::Admin)
            .ok_or(OracleError::NotInitialized)?;
        caller.require_auth();
        if *caller != admin {
            return Err(OracleError::Unauthorized);
        }
        Ok(())
    }

    fn consume_nonce(env: &Env, addr: &Address, expected: u64) -> bool {
        let current: u64 = env.storage().persistent()
            .get(&DataKey::Nonce(addr.clone())).unwrap_or(0u64);
        if current != expected { return false; }
        let key = DataKey::Nonce(addr.clone());
        env.storage().persistent().set(&key, &(current + 1));
        env.storage().persistent().extend_ttl(&key, TTL_THRESHOLD, MIN_TTL);
        true
    }

    fn is_oracle(env: &Env, oracle: &Address) -> bool {
        let set: Vec<Address> = env
            .storage().instance()
            .get(&DataKey::OracleSet)
            .unwrap_or_else(|| Vec::new(env));
        set.contains(oracle)
    }

    fn is_paused(env: &Env) -> bool {
        env.storage().instance().get(&DataKey::Paused).unwrap_or(false)
    }

    /// Returns true if `new_tonnes` deviates more than 20% from the last reading.
    fn detect_anomaly(env: &Env, project_id: &String, new_tonnes: i128) -> Result<bool, OracleError> {
        let prev: Option<MrvDataPoint> = env
            .storage().persistent()
            .get(&DataKey::Latest(project_id.clone()));
        match prev {
            None => Ok(false),
            Some(p) if p.tonnes == 0 => Ok(false),
            Some(p) => {
                let diff = (new_tonnes - p.tonnes).abs();
                // diff / prev > 0.20  ⟺  diff * 5 > prev
                let diff_times_5 = diff.checked_mul(5).ok_or(OracleError::Overflow)?;
                Ok(diff_times_5 > p.tonnes.abs())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Env, String};

    fn setup() -> (Env, MrvOracleClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        
        // Setup credit registry
        let registry_id = env.register(carbonchain_credit_registry::CreditRegistry, ());
        let registry_client = carbonchain_credit_registry::CreditRegistryClient::new(&env, &registry_id);
        let admin = Address::generate(&env);
        let verifier = Address::generate(&env);
        let retirement = Address::generate(&env);
        registry_client.initialize(&admin, &retirement);
        let nonce = registry_client.get_nonce(&admin);
        registry_client.register_verifier(&admin, &verifier, &nonce);
        
        // Create a test credit
        let issuer = Address::generate(&env);
        let inonce = registry_client.get_nonce(&issuer);
        registry_client.submit_credit(
            &issuer,
            &String::from_str(&env, "PROJ-001"),
            &2024,
            &String::from_str(&env, "VCS"),
            &String::from_str(&env, "NG"),
            &1_000_000,
            &String::from_str(&env, "bafybei123"),
            &inonce,
        );
        
        // Setup MRV oracle
        let id = env.register(MrvOracle, ());
        let client = MrvOracleClient::new(&env, &id);
        let oracle = Address::generate(&env);
        client.initialize(&admin);
        let ononce = client.get_nonce(&admin);
        client.register_oracle(&admin, &oracle, &ononce);
        (env, client, oracle, registry_id, admin)
    }

    #[test]
    fn test_initialize_emits_event() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(MrvOracle, ());
        let client = MrvOracleClient::new(&env, &id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        let events = env.events().all();
        // Exactly one event must be emitted: the mrv_init event.
        assert_eq!(events.len(), 1);
        let (_, topics, _data): (_, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val) =
            events.get(0).unwrap();
        // First topic is the symbol "mrv_init".
        let expected: soroban_sdk::Val = symbol_short!("mrv_init").into();
        assert_eq!(topics.get(0).unwrap(), expected);
    }

    #[test]
    fn test_update_and_get_latest() {
        let (env, client, oracle, registry_id, _admin) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let nonce = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_000_000, &nonce);
        let latest = client.get_latest(&proj).unwrap().unwrap();
        assert_eq!(latest.tonnes, 1_000_000);
        assert!(!latest.anomaly);
    }

    #[test]
    fn test_history_accumulates() {
        let (env, client, oracle, registry_id, _admin) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let nonce = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_000_000, &registry_id, &nonce);
        let nonce2 = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_050_000, &registry_id, &nonce2);
        assert_eq!(client.get_history(&proj).len(), 2);
    }

    #[test]
    fn test_anomaly_flagged_on_large_deviation() {
        let (env, client, oracle, registry_id, _admin) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let nonce = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_000_000, &registry_id, &nonce);
        let nonce2 = client.get_nonce(&oracle);
        let anomaly = client.update_mrv_data(&oracle, &proj, &1_500_000, &registry_id, &nonce2);
        assert!(anomaly);
        assert!(client.get_latest(&proj).unwrap().unwrap().anomaly);
    }

    #[test]
    fn test_no_anomaly_on_small_deviation() {
        let (env, client, oracle, registry_id, _admin) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let nonce = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_000_000, &registry_id, &nonce);
        let nonce2 = client.get_nonce(&oracle);
        let anomaly = client.update_mrv_data(&oracle, &proj, &1_100_000, &registry_id, &nonce2);
        assert!(!anomaly);
    }

    #[test]
    fn test_unauthorized_oracle_rejected() {
        let (env, client, _oracle, registry_id, _admin) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let rogue = Address::generate(&env);
        let nonce = client.get_nonce(&rogue);
        assert!(client.try_update_mrv_data(&rogue, &proj, &1_000_000, &registry_id, &nonce).is_err());
    }

    #[test]
    fn test_unregistered_project_rejected() {
        let (env, client, oracle, registry_id, _admin) = setup();
        let proj = String::from_str(&env, "PROJ-NONEXISTENT");
        let nonce = client.get_nonce(&oracle);
        assert!(client.try_update_mrv_data(&oracle, &proj, &1_000_000, &registry_id, &nonce).is_err());
    }

    #[test]
    fn test_history_cap_evicts_oldest() {
        let (env, client, oracle, registry_id, _admin) = setup();
        let proj = String::from_str(&env, "PROJ-CAP");
        for i in 0..=MAX_HISTORY {
            let nonce = client.get_nonce(&oracle);
            client.update_mrv_data(&oracle, &proj, &(i as i128 * 1_000), &registry_id, &nonce);
        }
        let history = client.get_history(&proj);
        assert_eq!(history.len(), MAX_HISTORY);
        assert_eq!(history.get(0).unwrap().tonnes, 1_000);
    }

    // ── register_oracle duplicate tests ─────────────────────────────────────

    #[test]
    fn test_register_oracle_returns_true_for_new() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(MrvOracle, ());
        let client = MrvOracleClient::new(&env, &id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        client.initialize(&admin);
        let newly_added = client.register_oracle(&admin, &oracle);
        assert!(newly_added);
    }

    #[test]
    fn test_register_oracle_returns_false_for_duplicate() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(MrvOracle, ());
        let client = MrvOracleClient::new(&env, &id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        client.initialize(&admin);
        client.register_oracle(&admin, &oracle);
        // Second registration of the same oracle must return false.
        let newly_added = client.register_oracle(&admin, &oracle);
        assert!(!newly_added);
    }

    #[test]
    fn test_register_oracle_duplicate_emits_oracle_dup_event() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(MrvOracle, ());
        let client = MrvOracleClient::new(&env, &id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        client.initialize(&admin);
        client.register_oracle(&admin, &oracle);
        // Clear events so we only see the duplicate-registration event.
        let events_before = env.events().all().len();
        client.register_oracle(&admin, &oracle);
        let events_after = env.events().all();
        // One new event must have been emitted.
        assert_eq!(events_after.len(), events_before + 1);
        let (_, topics, _): (_, soroban_sdk::Vec<soroban_sdk::Val>, soroban_sdk::Val) =
            events_after.get(events_before).unwrap();
        let expected: soroban_sdk::Val = symbol_short!("orc_dup").into();
        assert_eq!(topics.get(0).unwrap(), expected);
    }

    // ── Pause tests ──────────────────────────────────────────────────────────

    #[test]
    #[test]
    fn test_pause_blocks_update_mrv_data() {
        let (env, client, oracle, registry_id, admin) = setup();
        client.pause(&admin);
        assert!(client.paused());
        let proj = String::from_str(&env, "PROJ-001");
        assert!(client.try_update_mrv_data(&oracle, &proj, &1_000_000, &registry_id).is_err());
    }

    #[test]
    fn test_unpause_restores_update_mrv_data() {
        let (env, client, oracle, registry_id, admin) = setup();
        client.pause(&admin);
        client.unpause(&admin);
        assert!(!client.paused());
        let proj = String::from_str(&env, "PROJ-001");
        assert!(client.try_update_mrv_data(&oracle, &proj, &1_000_000, &registry_id).is_ok());
    }

    #[test]
    fn test_non_admin_cannot_pause() {
        let (env, client, _, _, _) = setup();
        let rando = Address::generate(&env);
        assert!(client.try_pause(&rando).is_err());
    }

    #[test]
    fn test_clear_anomaly_flag_after_review() {
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let nonce = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_000_000, &nonce);
        let nonce2 = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_500_000, &nonce2);
        assert!(client.get_latest(&proj).unwrap().anomaly);
        let nonce3 = client.get_nonce(&oracle);
        client.clear_anomaly_flag(&oracle, &proj, &nonce3);
        assert!(!client.get_latest(&proj).unwrap().anomaly);
    }

    #[test]
    fn test_clear_anomaly_flag_full_lifecycle() {
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-LIFECYCLE");
        let nonce = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_000_000, &nonce);
        let nonce2 = client.get_nonce(&oracle);
        let anomaly_detected = client.update_mrv_data(&oracle, &proj, &1_600_000, &nonce2);
        assert!(anomaly_detected);
        assert!(client.get_latest(&proj).unwrap().anomaly);
        let nonce3 = client.get_nonce(&oracle);
        client.clear_anomaly_flag(&oracle, &proj, &nonce3);
        assert!(!client.get_latest(&proj).unwrap().anomaly);
    }

    #[test]
    fn test_unauthorized_cannot_clear_anomaly_flag() {
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let nonce = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_000_000, &nonce);
        let nonce2 = client.get_nonce(&oracle);
        client.update_mrv_data(&oracle, &proj, &1_500_000, &nonce2);
        let rogue = Address::generate(&env);
        let nonce3 = client.get_nonce(&rogue);
        assert!(client.try_clear_anomaly_flag(&rogue, &proj, &nonce3).is_err());
    }
}
