#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Env, Address, String, Vec,
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
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum OracleError {
    NotInitialized   = 119,
    Unauthorized     = 120,
    AlreadyInitialized = 121,
    Overflow         = 122,
}

// Maximum MRV history entries retained per project (ring-buffer eviction).
const MAX_HISTORY: u32 = 100;

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct MrvOracle;

#[contractimpl]
impl MrvOracle {
    pub fn initialize(env: Env, admin: Address) -> Result<(), OracleError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(OracleError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.events().publish((symbol_short!("mrv_init"),), admin);
        Ok(())
    }

    pub fn register_oracle(env: Env, admin: Address, oracle: Address) -> Result<(), OracleError> {
        Self::require_admin(&env, &admin)?;
        let mut set: Vec<Address> = env
            .storage().instance()
            .get(&DataKey::OracleSet)
            .unwrap_or_else(|| Vec::new(&env));
        if !set.contains(&oracle) {
            set.push_back(oracle);
            env.storage().instance().set(&DataKey::OracleSet, &set);
        }
        Ok(())
    }

    /// Submit a new MRV reading for a project.
    /// Anomaly flag is set when the new reading deviates >20% from the previous one.
    pub fn update_mrv_data(
        env: Env,
        oracle: Address,
        project_id: String,
        tonnes: i128,
    ) -> Result<bool, OracleError> {
        oracle.require_auth();
        if !Self::is_oracle(&env, &oracle) {
            return Err(OracleError::Unauthorized);
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

        env.events().publish(
            (symbol_short!("mrv_upd"), oracle),
            (project_id, tonnes, anomaly),
        );

        Ok(anomaly)
    }

    pub fn get_latest(env: Env, project_id: String) -> Option<MrvDataPoint> {
        env.storage().persistent().get(&DataKey::Latest(project_id))
    }

    pub fn get_history(env: Env, project_id: String) -> Vec<MrvDataPoint> {
        env.storage()
            .persistent()
            .get(&DataKey::History(project_id))
            .unwrap_or_else(|| Vec::new(&env))
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

    fn is_oracle(env: &Env, oracle: &Address) -> bool {
        let set: Vec<Address> = env
            .storage().instance()
            .get(&DataKey::OracleSet)
            .unwrap_or_else(|| Vec::new(env));
        set.contains(oracle)
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

    fn setup() -> (Env, MrvOracleClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(MrvOracle, ());
        let client = MrvOracleClient::new(&env, &id);
        let admin = Address::generate(&env);
        let oracle = Address::generate(&env);
        client.initialize(&admin);
        client.register_oracle(&admin, &oracle);
        (env, client, admin, oracle)
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
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        client.update_mrv_data(&oracle, &proj, &1_000_000);
        let latest = client.get_latest(&proj).unwrap();
        assert_eq!(latest.tonnes, 1_000_000);
        assert!(!latest.anomaly);
    }

    #[test]
    fn test_history_accumulates() {
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        client.update_mrv_data(&oracle, &proj, &1_000_000);
        client.update_mrv_data(&oracle, &proj, &1_050_000);
        assert_eq!(client.get_history(&proj).len(), 2);
    }

    #[test]
    fn test_anomaly_flagged_on_large_deviation() {
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        client.update_mrv_data(&oracle, &proj, &1_000_000);
        // 50% jump — should flag anomaly
        let anomaly = client.update_mrv_data(&oracle, &proj, &1_500_000);
        assert!(anomaly);
        assert!(client.get_latest(&proj).unwrap().anomaly);
    }

    #[test]
    fn test_no_anomaly_on_small_deviation() {
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        client.update_mrv_data(&oracle, &proj, &1_000_000);
        // 10% jump — within threshold
        let anomaly = client.update_mrv_data(&oracle, &proj, &1_100_000);
        assert!(!anomaly);
    }

    #[test]
    fn test_unauthorized_oracle_rejected() {
        let (env, client, _admin, _oracle) = setup();
        let proj = String::from_str(&env, "PROJ-001");
        let rogue = Address::generate(&env);
        assert!(client.try_update_mrv_data(&rogue, &proj, &1_000_000).is_err());
    }

    #[test]
    fn test_history_cap_evicts_oldest() {
        let (env, client, _admin, oracle) = setup();
        let proj = String::from_str(&env, "PROJ-CAP");
        // Submit MAX_HISTORY + 1 entries; history should stay at MAX_HISTORY.
        for i in 0..=MAX_HISTORY {
            client.update_mrv_data(&oracle, &proj, &(i as i128 * 1_000));
        }
        let history = client.get_history(&proj);
        assert_eq!(history.len(), MAX_HISTORY);
        // Oldest entry (tonnes=0) should have been evicted; first entry is now tonnes=1_000.
        assert_eq!(history.get(0).unwrap().tonnes, 1_000);
    }
}
