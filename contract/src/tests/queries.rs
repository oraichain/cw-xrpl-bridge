use cosmwasm_std::{coins, Addr, Uint128};

use crate::contract::{
    INITIAL_PROHIBITED_XRPL_ADDRESSES, XRP_CURRENCY, XRP_DEFAULT_MAX_HOLDING_AMOUNT,
    XRP_DEFAULT_SENDING_PRECISION, XRP_ISSUER, XRP_SUBUNIT,
};
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{
    ExecuteMsg, ProhibitedXRPLAddressesResponse, TransactionEvidence, TransactionEvidencesResponse,
    XRPLTokensResponse,
};
use crate::state::{BridgeState, Config, TokenState, XRPLToken};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{
    msg::{InstantiateMsg, QueryMsg},
    relayer::Relayer,
};

#[test]
fn queries() {
    let accounts_number = 4;
    let accounts: Vec<_> = (0..accounts_number)
        .into_iter()
        .map(|i| format!("account{i}"))
        .collect();

    let mut app = MockApp::new(&[
        (accounts[0].as_str(), &coins(100_000_000_000, FEE_DENOM)),
        (accounts[1].as_str(), &coins(100_000_000_000, FEE_DENOM)),
        (accounts[2].as_str(), &coins(100_000_000_000, FEE_DENOM)),
        (accounts[3].as_str(), &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[accounts_number - 1];
    let xrpl_addresses: Vec<String> = (0..3).map(|_| generate_xrpl_address()).collect();
    let xrpl_pub_keys: Vec<String> = (0..3).map(|_| generate_xrpl_pub_key()).collect();

    let mut relayer_accounts = vec![];
    let mut relayers = vec![];

    for i in 0..accounts_number - 1 {
        let account = format!("account{}", i);
        relayer_accounts.push(account.clone());
        relayers.push(Relayer {
            coreum_address: Addr::unchecked(account),
            xrpl_address: xrpl_addresses[i as usize].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i as usize].to_string(),
        });
    }

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    let bridge_xrpl_address = generate_xrpl_address();
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![
                    relayers[0].clone(),
                    relayers[1].clone(),
                    relayers[2].clone(),
                ],
                evidence_threshold: 3,
                used_ticket_sequence_threshold: 5,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: bridge_xrpl_address.clone(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
            },
        )
        .unwrap();

    // Query the config
    let query_config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(
        query_config,
        Config {
            relayers: vec![
                relayers[0].clone(),
                relayers[1].clone(),
                relayers[2].clone()
            ],
            evidence_threshold: 3,
            used_ticket_sequence_threshold: 5,
            trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
            bridge_xrpl_address: bridge_xrpl_address.clone(),
            bridge_state: BridgeState::Active,
            xrpl_base_fee: 10,
            token_factory_addr: token_factory_addr.clone()
        }
    );

    // Query XRPL tokens
    let query_xrpl_tokens: XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(
        query_xrpl_tokens.tokens[0],
        XRPLToken {
            issuer: XRP_ISSUER.to_string(),
            currency: XRP_CURRENCY.to_string(),
            coreum_denom: format!("{}/{}", XRP_SUBUNIT, token_factory_addr),
            sending_precision: XRP_DEFAULT_SENDING_PRECISION,
            max_holding_amount: Uint128::new(XRP_DEFAULT_MAX_HOLDING_AMOUNT),
            state: TokenState::Enabled,
            bridging_fee: Uint128::zero(),
        }
    );

    // Let's create a ticket operation
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(6),
        },
        &[],
    )
    .unwrap();

    // Two relayers will return the evidence as rejected and one as accepted
    let tx_hash = generate_hash();
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(tx_hash.clone()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..7).collect()),
                }),
            },
        },
        &[],
    )
    .unwrap();
    app.execute(
        Addr::unchecked(&relayer_accounts[1]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(tx_hash.clone()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..7).collect()),
                }),
            },
        },
        &[],
    )
    .unwrap();
    app.execute(
        Addr::unchecked(&relayer_accounts[2]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(tx_hash.clone()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Rejected,
                operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
            },
        },
        &[],
    )
    .unwrap();

    // Let's query all the transaction evidences (we should get two)
    let query_transaction_evidences: TransactionEvidencesResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::TransactionEvidences {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_transaction_evidences.transaction_evidences.len(), 2);

    // Let's query all the transaction evidences with pagination
    let query_transaction_evidences: TransactionEvidencesResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::TransactionEvidences {
                start_after_key: None,
                limit: Some(1),
            },
        )
        .unwrap();

    assert_eq!(query_transaction_evidences.transaction_evidences.len(), 1);

    let query_transaction_evidences: TransactionEvidencesResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::TransactionEvidences {
                start_after_key: query_transaction_evidences.last_key,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_transaction_evidences.transaction_evidences.len(), 1);

    // Let's query a transaction evidences by evidence hash and verify that we have an address that provided that evidence
    let query_transaction_evidence: TransactionEvidence = app
        .query(
            contract_addr.clone(),
            &QueryMsg::TransactionEvidence {
                hash: query_transaction_evidences.transaction_evidences[0]
                    .hash
                    .clone(),
            },
        )
        .unwrap();

    assert!(!query_transaction_evidence.relayer_addresses.is_empty());

    // Let's query the prohibited addresses
    let query_prohibited_addresses: ProhibitedXRPLAddressesResponse = app
        .query(contract_addr.clone(), &QueryMsg::ProhibitedXRPLAddresses {})
        .unwrap();

    assert_eq!(
        query_prohibited_addresses.prohibited_xrpl_addresses.len(),
        INITIAL_PROHIBITED_XRPL_ADDRESSES.len() + 1
    );
    assert!(query_prohibited_addresses
        .prohibited_xrpl_addresses
        .contains(&bridge_xrpl_address));

    // Let's try to update this by adding a new one and query again
    let new_prohibited_address = generate_xrpl_address();
    let mut prohibited_addresses = query_prohibited_addresses.prohibited_xrpl_addresses.clone();
    prohibited_addresses.push(new_prohibited_address.clone());
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateProhibitedXRPLAddresses {
            prohibited_xrpl_addresses: prohibited_addresses,
        },
        &[],
    )
    .unwrap();

    let query_prohibited_addresses: ProhibitedXRPLAddressesResponse = app
        .query(contract_addr.clone(), &QueryMsg::ProhibitedXRPLAddresses {})
        .unwrap();

    assert_eq!(
        query_prohibited_addresses.prohibited_xrpl_addresses.len(),
        INITIAL_PROHIBITED_XRPL_ADDRESSES.len() + 2
    );
    assert!(query_prohibited_addresses
        .prohibited_xrpl_addresses
        .contains(&bridge_xrpl_address));

    assert!(query_prohibited_addresses
        .prohibited_xrpl_addresses
        .contains(&new_prohibited_address));

    // If we try to update this from an account that is not the owner it will fail
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::UpdateProhibitedXRPLAddresses {
            prohibited_xrpl_addresses: vec![],
        },
        &[],
    )
    .unwrap_err();
}
