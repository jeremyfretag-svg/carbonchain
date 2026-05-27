use soroban_sdk::{contracttype, Address, String, BytesN};

/// Unit convention: all `tonnes` fields are stored as fixed-point integers
/// where 1 tonne = 1_000_000 units (0.1 tonne resolution = 100_000 units).
pub const TONNES_SCALE: i128 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
#[contracttype]
pub enum CreditStatus {
    Pending = 0,
    Active = 1,
    Retired = 2,
    Flagged = 3,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct CreditMetadata {
    pub project_id: String,
    pub issuer: Address,
    pub vintage_year: u32,
    pub methodology: String,
    pub geography: String,
    /// Carbon volume in scaled units. 1 tonne = [`TONNES_SCALE`] (1_000_000).
    pub tonnes: i128,
    pub ipfs_hash: String,
    pub status: CreditStatus,
    pub issued_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    VerifierSet,
    Credit(BytesN<32>),
    ProjectCredits(String),
    RetirementContract,
}
