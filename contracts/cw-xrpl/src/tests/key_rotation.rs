use crate::contract::XRP_ISSUER;
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::PendingOperationsResponse;
use crate::operation::{Operation, OperationType};
use crate::state::{BridgeState, Config};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{
    contract::XRP_CURRENCY,
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    relayer::Relayer,
};
use cosmwasm_std::{coins, Addr, Uint128};

#[test]
fn key_rotation() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
        ("account3", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let accounts_number = accounts.len();

    let signer = &accounts[accounts_number - 1];
    let xrpl_addresses: Vec<String> = (0..3).map(|_| generate_xrpl_address()).collect();
    let xrpl_pub_keys: Vec<String> = (0..3).map(|_| generate_xrpl_pub_key()).collect();

    let mut relayer_accounts = vec![];
    let mut relayers = vec![];

    for i in 0..accounts_number - 1 {
        relayer_accounts.push(accounts[i].to_string());
        relayers.push(Relayer {
            cosmos_address: Addr::unchecked(&accounts[i]),
            xrpl_address: xrpl_addresses[i as usize].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i as usize].to_string(),
        });
    }

    let xrpl_base_fee = 10;

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // Test with 1 relayer and 1 evidence threshold first
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
                used_ticket_sequence_threshold: 4,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: None,
                osor_entry_point: None,
            },
        )
        .unwrap();

    // Recover enough tickets for testing
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(5),
        },
        &[],
    )
    .unwrap();

    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: Some(1),
                    ticket_sequence: None,
                    transaction_result: TransactionResult::Accepted,
                    operation_result: Some(OperationResult::TicketsAllocation {
                        tickets: Some((1..6).collect()),
                    }),
                },
            },
            &[],
        )
        .unwrap();
    }

    // Let's send a random evidence from 1 relayer that will stay after key rotation to confirm that it will be cleared after key rotation confirmation
    let tx_hash_old_evidence = generate_hash();
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash_old_evidence.clone(),
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                amount: Uint128::one(),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // If we send it again it should by same relayer it should fail because it's duplicated
    let error_duplicated_evidence = app
        .execute(
            Addr::unchecked(&relayer_accounts[0]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash_old_evidence.clone(),
                    issuer: XRP_ISSUER.to_string(),
                    currency: XRP_CURRENCY.to_string(),
                    amount: Uint128::one(),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(error_duplicated_evidence.root_cause().to_string().contains(
        ContractError::EvidenceAlreadyProvided {}
            .to_string()
            .as_str()
    ));

    // We are going to perform a key rotation, for that we are going to remove a malicious relayer
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RotateKeys {
            new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
            new_evidence_threshold: 2,
        },
        &[],
    )
    .unwrap();

    // If we try to perform another key rotation, it should fail because we have one pending ongoing
    let pending_rotation_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::RotateKeys {
                new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
                new_evidence_threshold: 2,
            },
            &[],
        )
        .unwrap_err();

    assert!(pending_rotation_error
        .root_cause()
        .to_string()
        .contains(ContractError::RotateKeysOngoing {}.to_string().as_str()));

    // Let's confirm that a pending operation is created
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 1);
    assert_eq!(
        query_pending_operations.operations[0],
        Operation {
            id: query_pending_operations.operations[0].id.clone(),
            version: 1,
            ticket_sequence: Some(1),
            account_sequence: None,
            signatures: vec![],
            operation_type: OperationType::RotateKeys {
                new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
                new_evidence_threshold: 2
            },
            xrpl_base_fee,
        }
    );

    // Any evidence we send now that is not a RotateKeys evidence should fail
    let error_no_key_rotation_evidence = app
        .execute(
            Addr::unchecked(&relayer_accounts[1]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash_old_evidence.clone(),
                    issuer: XRP_ISSUER.to_string(),
                    currency: XRP_CURRENCY.to_string(),
                    amount: Uint128::one(),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(error_no_key_rotation_evidence
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    // We are going to confirm the RotateKeys as rejected and check that nothing is changed and bridge is still halted
    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: None,
                    ticket_sequence: Some(1),
                    transaction_result: TransactionResult::Rejected,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    // Pending operation should have been removed
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert!(query_pending_operations.operations.is_empty());

    // Check config and see that it's the same as before and bridge is still halted
    let query_config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(query_config.relayers, relayers);
    assert_eq!(query_config.evidence_threshold, 3);
    assert_eq!(query_config.bridge_state, BridgeState::Halted);

    // Let's try to perform a key rotation again and check that it works
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RotateKeys {
            new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
            new_evidence_threshold: 2,
        },
        &[],
    )
    .unwrap();

    // Let's confirm that a pending operation is created
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 1);
    assert_eq!(
        query_pending_operations.operations[0],
        Operation {
            id: query_pending_operations.operations[0].id.clone(),
            version: 1,
            ticket_sequence: Some(2),
            account_sequence: None,
            signatures: vec![],
            operation_type: OperationType::RotateKeys {
                new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
                new_evidence_threshold: 2
            },
            xrpl_base_fee,
        }
    );

    // We are going to confirm the RotateKeys as accepted and check that config has been updated correctly
    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: None,
                    ticket_sequence: Some(2),
                    transaction_result: TransactionResult::Accepted,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    let query_config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(
        query_config.relayers,
        vec![relayers[0].clone(), relayers[1].clone()]
    );
    assert_eq!(query_config.evidence_threshold, 2);
    assert_eq!(query_config.bridge_state, BridgeState::Halted);

    // Owner can now resume the bridge
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::ResumeBridge {},
        &[],
    )
    .unwrap();

    // Let's check that evidences have been cleared by sending again the old evidence and it succeeds
    // If evidences were cleared, this message will succeed because the evidence is not stored
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: tx_hash_old_evidence.clone(),
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                amount: Uint128::one(),
                recipient: Addr::unchecked(signer),
            },
        },
        &[],
    )
    .unwrap();

    // Finally, let's check that the old relayer can not send evidences anymore
    let error_not_relayer = app
        .execute(
            Addr::unchecked(&relayer_accounts[2]),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash_old_evidence.clone(),
                    issuer: XRP_ISSUER.to_string(),
                    currency: XRP_CURRENCY.to_string(),
                    amount: Uint128::one(),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(error_not_relayer
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));
}
