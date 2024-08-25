use cosmwasm_std::{coins, Addr, Uint128};
use cw_ownable::Ownership;

use crate::msg::ExecuteMsg;
use crate::tests::helper::{
    generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM, TRUST_SET_LIMIT_AMOUNT,
};
use crate::{
    msg::{InstantiateMsg, QueryMsg},
    relayer::Relayer,
};


#[test]
fn transfer_ownership() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let new_owner = &accounts[1];

    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();

    // Query current owner
    let query_owner: Ownership<Addr> = app
        .query(contract_addr.clone(), &QueryMsg::Ownership {})
        .unwrap();

    assert_eq!(query_owner.owner, Some(Addr::unchecked(signer)));

    // Current owner is going to transfer ownership to another address (new_owner)
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
            new_owner: new_owner.to_string(),
            expiry: None,
        }),
        &[],
    )
    .unwrap();

    // New owner is going to accept the ownership
    app.execute(
        Addr::unchecked(new_owner),
        contract_addr.clone(),
        &ExecuteMsg::UpdateOwnership(cw_ownable::Action::AcceptOwnership {}),
        &[],
    )
    .unwrap();

    let query_owner: Ownership<Addr> = app
        .query(contract_addr.clone(), &QueryMsg::Ownership {})
        .unwrap();

    assert_eq!(query_owner.owner, Some(Addr::unchecked(new_owner)));

    // Try transfering from old owner again, should fail
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
            new_owner: "new_owner".to_string(),
            expiry: None,
        }),
        &[],
    )
    .unwrap_err();
}
