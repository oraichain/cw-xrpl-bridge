use cosmwasm_std::{Addr, Coin, HexBinary, Uint128};
use derive_more::{Deref, DerefMut};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use ripple_keypairs::Seed;
use sha2::{Digest, Sha256};
// use coreum_test_tube::{Account, AssetFT, Bank, CoreumTestApp, Module, SigningAccount, Wasm};
// use coreum_wasm_sdk::types::coreum::asset::ft::v1::{MsgFreeze, MsgUnfreeze};
// use coreum_wasm_sdk::types::cosmos::bank::v1beta1::QueryTotalSupplyRequest;
// use coreum_wasm_sdk::types::cosmos::base::v1beta1::Coin as BaseCoin;
// use coreum_wasm_sdk::{
//     assetft::{FREEZING, IBC, MINTING},
//     types::{
//         coreum::asset::ft::v1::{
//             MsgIssue, QueryBalanceRequest, QueryParamsRequest, QueryTokensRequest, Token,
//         },
//         cosmos::bank::v1beta1::MsgSend,
//     },
// };

use cw_multi_test::ContractWrapper;

pub const FEE_DENOM: &str = "ucore";
pub const XRP_SYMBOL: &str = "XRP";
pub const XRP_SUBUNIT: &str = "drop";
pub const XRPL_DENOM_PREFIX: &str = "xrpl";
pub const TRUST_SET_LIMIT_AMOUNT: u128 = 1000000000000000000; // 1e18
pub const XRP_DECIMALS: u32 = 6;
pub const XRP_DEFAULT_SENDING_PRECISION: i32 = 6;
pub const XRP_DEFAULT_MAX_HOLDING_AMOUNT: u128 =
    10u128.pow(16 - XRP_DEFAULT_SENDING_PRECISION as u32 + XRP_DECIMALS);

#[derive(Clone)]
pub struct XRPLToken {
    pub issuer: String,
    pub currency: String,
    pub sending_precision: i32,
    pub max_holding_amount: Uint128,
    pub bridging_fee: Uint128,
}

#[derive(Clone)]
pub struct CoreumToken {
    pub denom: String,
    pub decimals: u32,
    pub sending_precision: i32,
    pub max_holding_amount: Uint128,
    pub bridging_fee: Uint128,
}

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
