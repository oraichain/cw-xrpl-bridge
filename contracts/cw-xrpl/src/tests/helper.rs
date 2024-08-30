use cosmwasm_std::{coins, Addr, Coin};
use cosmwasm_testing_util::MockResult;
use derive_more::{Deref, DerefMut};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use ripple_keypairs::Seed;

pub const FEE_DENOM: &str = "orai";
pub const TRUST_SET_LIMIT_AMOUNT: u128 = 1000000000000000000; // 1e18

#[cfg(not(feature = "test-tube"))]
pub type TestMockApp = cosmwasm_testing_util::MultiTestMockApp;
#[cfg(feature = "test-tube")]
pub type TestMockApp = cosmwasm_testing_util::TestTubeMockApp;

#[derive(Deref, DerefMut)]
pub struct MockApp {
    #[deref]
    #[deref_mut]
    app: TestMockApp,
    bridge_id: u64,
}

#[allow(dead_code)]
impl MockApp {
    pub fn new(init_balances: &[(&str, &[Coin])]) -> (Self, Vec<String>) {
        let (mut app, accounts) = TestMockApp::new(init_balances);
        let bridge_id;
        #[cfg(feature = "test-tube")]
        {
            bridge_id = app.upload(include_bytes!("./testdata/cw-xrpl.wasm"));
        }
        #[cfg(not(feature = "test-tube"))]
        {
            bridge_id = app.upload(Box::new(
                cosmwasm_testing_util::ContractWrapper::new_with_empty(
                    crate::contract::execute,
                    crate::contract::instantiate,
                    crate::contract::query,
                ),
            ));
        }
        (Self { app, bridge_id }, accounts)
    }

    /// external method
    pub fn create_bridge(
        &mut self,
        sender: Addr,
        init_msg: &crate::msg::InstantiateMsg,
    ) -> MockResult<Addr> {
        let code_id = self.bridge_id;
        self.instantiate(
            code_id,
            sender,
            init_msg,
            &coins(10_000_000u128, FEE_DENOM), // denom creation fee
            "cw-xrpl-bridge",
        )
    }
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
