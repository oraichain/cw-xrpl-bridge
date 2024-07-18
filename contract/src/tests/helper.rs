use cosmwasm_std::{Addr, Coin, HexBinary};
use derive_more::{Deref, DerefMut};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use ripple_keypairs::Seed;
use sha2::{Digest, Sha256};

use cw_multi_test::ContractWrapper;

pub const FEE_DENOM: &str = "ucore";
pub const TRUST_SET_LIMIT_AMOUNT: u128 = 1000000000000000000; // 1e18

#[derive(Deref, DerefMut)]
pub struct MockApp {
    #[deref]
    #[deref_mut]
    app: cosmwasm_testing_util::MockApp,
    bridge_id: u64,
}

#[allow(dead_code)]
impl MockApp {
    pub fn new(init_balances: &[(&str, &[Coin])]) -> Self {
        let mut app = cosmwasm_testing_util::MockApp::new(init_balances);

        let bridge_id = app.upload(Box::new(ContractWrapper::new_with_empty(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        )));

        Self { app, bridge_id }
    }

    /// external method
    pub fn create_bridge(
        &mut self,
        sender: Addr,
        init_msg: &crate::msg::InstantiateMsg,
    ) -> Result<Addr, String> {
        let code_id = self.bridge_id;
        let addr = self.instantiate(code_id, sender, init_msg, &[], "cw-xrpl-bridge")?;
        Ok(addr)
    }
}

pub fn hash_bytes(bytes: Vec<u8>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let output = hasher.finalize();
    HexBinary::from(output.as_slice()).to_hex()
}

pub fn generate_hash() -> String {
    String::from_utf8(
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(20)
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

pub fn generate_xrpl_address() -> String {
    let seed = Seed::random();
    let (_, public_key) = seed.derive_keypair().unwrap();
    let address = public_key.derive_address();
    address
}

pub fn generate_invalid_xrpl_address() -> String {
    let mut address = 'r'.to_string();
    let mut rand = String::from_utf8(
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .collect::<Vec<_>>(),
    )
    .unwrap();

    rand = rand.replace("0", "1");
    rand = rand.replace("O", "o");
    rand = rand.replace("I", "i");
    rand = rand.replace("l", "L");

    address.push_str(rand.as_str());
    address
}

pub fn generate_xrpl_pub_key() -> String {
    String::from_utf8(
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(52)
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
