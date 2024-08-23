use crate::contract::{MAX_RELAYERS, XRP_SYMBOL};
use crate::error::ContractError;
use crate::tests::helper::{
    generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM, TRUST_SET_LIMIT_AMOUNT,
};
use crate::{msg::InstantiateMsg, relayer::Relayer};
use cosmwasm_std::{coins, Addr, Uint128};
use cosmwasm_testing_util::{MockAppExtensions, MockTokenExtensions};
use token_bindings::{FullDenomResponse, MetadataResponse};

#[test]
fn contract_instantiation() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let relayer_addr = &accounts[1];

    let xrpl_address = generate_xrpl_address();
    let xrpl_pub_key = generate_xrpl_pub_key();

    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: xrpl_address.clone(),
        xrpl_pub_key: xrpl_pub_key.clone(),
    };

    let relayer_duplicated_xrpl_address = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address,
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let relayer_duplicated_xrpl_pub_key = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key,
    };

    let relayer_duplicated_cosmos_address = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let relayer_prohibited_xrpl_address = Relayer {
        cosmos_address: Addr::unchecked(relayer_addr),
        xrpl_address: "rHb9CJAWyB4rj91VRWn96DkukG4bwdtyTh".to_string(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let relayer_correct = Relayer {
        cosmos_address: Addr::unchecked(relayer_addr),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // We check that we can store and instantiate
    app.create_bridge(
        Addr::unchecked(signer),
        &InstantiateMsg {
            owner: Addr::unchecked(signer),
            relayers: vec![relayer.clone(), relayer_correct.clone()],
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

    // We check that trying to instantiate with relayers with the same xrpl address fails
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone(), relayer_duplicated_xrpl_address.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();

    assert!(error
        .root_cause()
        .to_string()
        .contains(ContractError::DuplicatedRelayer {}.to_string().as_str()));

    // We check that trying to instantiate with relayers with the same xrpl public key fails
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone(), relayer_duplicated_xrpl_pub_key.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();

    assert!(error
        .root_cause()
        .to_string()
        .contains(ContractError::DuplicatedRelayer {}.to_string().as_str()));

    // We check that trying to instantiate with relayers with the same cosmos address fails
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone(), relayer_duplicated_cosmos_address.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();
    assert!(error
        .root_cause()
        .to_string()
        .contains(ContractError::DuplicatedRelayer {}.to_string().as_str()));

    // We check that trying to use a relayer with a prohibited address fails
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone(), relayer_prohibited_xrpl_address.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();

    assert!(error
        .root_cause()
        .to_string()
        .contains(ContractError::ProhibitedAddress {}.to_string().as_str()));

    // We check that trying to instantiate with invalid bridge_xrpl_address fails
    let invalid_address = "rf0BiGeXwwQoi8Z2ueFYTEXSwuJYfV2Jpn".to_string(); //invalid because contains a 0
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: invalid_address.clone(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();

    assert!(error.root_cause().to_string().contains(
        ContractError::InvalidXRPLAddress {
            address: invalid_address
        }
        .to_string()
        .as_str()
    ));

    // We check that trying to instantiate with invalid max allowed ticket fails.
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 1,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();
    assert!(error.root_cause().to_string().contains(
        ContractError::InvalidUsedTicketSequenceThreshold {}
            .to_string()
            .as_str()
    ));

    // Instantiating with threshold 0 will fail
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![],
                evidence_threshold: 0,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();

    assert!(error
        .root_cause()
        .to_string()
        .contains(ContractError::InvalidThreshold {}.to_string().as_str()));

    // Instantiating with too many relayers (> 32) should fail
    let mut too_many_relayers = vec![];
    for i in 0..MAX_RELAYERS + 1 {
        too_many_relayers.push(Relayer {
            cosmos_address: Addr::unchecked(format!("cosmos_address_{}", i)),
            xrpl_address: generate_xrpl_address(),
            xrpl_pub_key: generate_xrpl_pub_key(),
        });
    }

    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: too_many_relayers,
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();

    assert!(error
        .root_cause()
        .to_string()
        .contains(ContractError::TooManyRelayers {}.to_string().as_str()));

    // We check that trying to instantiate with an invalid trust set amount will fail
    let error = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer, relayer_correct],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 50,
                trust_set_limit_amount: Uint128::new(10000000000000001),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: false,
            },
        )
        .unwrap_err();

    assert!(error
        .root_cause()
        .to_string()
        .contains(ContractError::InvalidXRPLAmount {}.to_string().as_str()));

    let FullDenomResponse { denom } = app
        .query(
            token_factory_addr.clone(),
            &tokenfactory::msg::QueryMsg::GetDenom {
                creator_address: token_factory_addr.to_string(),
                subdenom: XRP_SYMBOL.to_string(),
            },
        )
        .unwrap();

    // We query the issued token by the contract instantiation (XRP)
    let MetadataResponse { metadata } = app
        .query::<MetadataResponse, _>(
            token_factory_addr.clone(),
            &tokenfactory::msg::QueryMsg::GetMetadata {
                denom: denom.to_string(),
            },
        )
        .unwrap_or(MetadataResponse { metadata: None });

    assert_eq!(metadata, None);
}
