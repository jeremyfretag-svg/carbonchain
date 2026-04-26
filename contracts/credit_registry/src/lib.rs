#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Address, String, BytesN};

pub mod types;
pub mod errors;

#[contract]
pub struct CreditRegistry;

#[contractimpl]
impl CreditRegistry {
    pub fn initialize(_env: Env, _admin: Address) {
        // TODO: Implementation
    }

    pub fn submit_credit(env: Env, _issuer: Address, _project_id: String, _tonnes: i128) -> BytesN<32> {
        // TODO: Implementation
        BytesN::from_array(&env, &[0u8; 32])
    }
}
