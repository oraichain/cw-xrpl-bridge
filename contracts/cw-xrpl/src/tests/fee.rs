use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{ExecuteMsg, PendingOperationsResponse, QueryMsg};
use crate::state::Config;
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{msg::InstantiateMsg, relayer::Relayer};
use cosmwasm_std::{coins, Addr, Uint128};

#[test]
fn updating_xrpl_base_fee() {
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
        relayer_accounts.push(accounts[i].to_string());
        relayers.push(Relayer {
            cosmos_address: Addr::unchecked(&accounts[i]),
            xrpl_address: xrpl_addresses[i as usize].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i as usize].to_string(),
        });
    }

    let xrpl_base_fee = 10;

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: relayers.clone(),
                evidence_threshold: 3,
                used_ticket_sequence_threshold: 9,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
            },
        )
        .unwrap();

    // Add enough tickets for all our tests
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(250),
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
                        tickets: Some((1..251).collect()),
                    }),
                },
            },
            &[],
        )
        .unwrap();
    }

    // We are going to create the max number of pending operations and add signatures to them to verify that we can update all of them at once
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: generate_xrpl_address(),
            currency: "USD".to_string(),
            sending_precision: 15,
            max_holding_amount: Uint128::new(100000),
            bridging_fee: Uint128::zero(),
        },
        &[],
    )
    .unwrap();

    // Register COSMOS to send some
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: FEE_DENOM.to_string(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::new(100000),
            bridging_fee: Uint128::zero(),
        },
        &[],
    )
    .unwrap();

    // Let's create 247 more so that we get up to 250 in the end
    for _ in 0..247 {
        app.execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1, FEE_DENOM.to_string()),
        )
        .unwrap();
    }

    // Query pending operations with limit and start_after_key to verify it works
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: Some(100),
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 100);

    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: query_pending_operations.last_key,
                limit: Some(200),
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 148);

    // Query all pending operations
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 248);

    // Halt the bridge to verify that we can't send signatures of pending operations that are not allowed
    let correct_signature_example = "3045022100DFA01DA5D6C9877F9DAA59A06032247F3D7ED6444EAD5C90A3AC33CCB7F19B3F02204D8D50E4D085BB1BC9DFB8281B8F35BDAEB7C74AE4B825F8CAE1217CFBDF4EA1".to_string();
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::HaltBridge {},
        &[],
    )
    .unwrap();

    let signature_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[0]),
            contract_addr.clone(),
            &ExecuteMsg::SaveSignature {
                operation_id: query_pending_operations.operations[0]
                    .ticket_sequence
                    .unwrap(),
                operation_version: 1,
                signature: correct_signature_example.clone(),
            },
            &[],
        )
        .unwrap_err();

    assert!(signature_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    // Resume the bridge to add signatures again
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::ResumeBridge {},
        &[],
    )
    .unwrap();

    // Add some signatures to each pending operation
    for pending_operation in query_pending_operations.operations.iter() {
        for relayer in &relayer_accounts {
            app.execute(
                Addr::unchecked(relayer),
                contract_addr.clone(),
                &ExecuteMsg::SaveSignature {
                    operation_id: pending_operation.ticket_sequence.unwrap(),
                    operation_version: 1,
                    signature: correct_signature_example.clone(),
                },
                &[],
            )
            .unwrap();
        }
    }

    // Add a Key Rotation, which will verify that we can update the base fee while the bridge is halted
    // and to check that we can add signatures for key rotations while bridge is halted
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

    // Verify that we have 249 pending operations
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 249);

    // Sign this last operation with the 3 relayers

    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveSignature {
                operation_id: query_pending_operations.operations[248]
                    .ticket_sequence
                    .unwrap(),
                operation_version: 1,
                signature: correct_signature_example.clone(),
            },
            &[],
        )
        .unwrap();
    }

    // Verify that all pending operations are in version 1 and have three signatures each
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    for pending_operation in query_pending_operations.operations.iter() {
        assert_eq!(pending_operation.version, 1);
        assert_eq!(pending_operation.signatures.len(), 3);
    }

    // If we trigger an XRPL base fee by some who is not the owner, it should fail.
    let unauthorized_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[0]),
            contract_addr.clone(),
            &ExecuteMsg::UpdateXRPLBaseFee { xrpl_base_fee: 600 },
            &[],
        )
        .unwrap_err();

    assert!(unauthorized_error
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    let new_xrpl_base_fee = 20;
    // If we trigger an XRPL base fee update, all signatures must be gone, and pending operations must be in version 2, and pending operations base fee must be the new one
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLBaseFee {
            xrpl_base_fee: new_xrpl_base_fee,
        },
        &[],
    )
    .unwrap();

    // Let's query all pending operations again to verify
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    for pending_operation in query_pending_operations.operations.iter() {
        assert_eq!(pending_operation.version, 2);
        assert_eq!(pending_operation.xrpl_base_fee, new_xrpl_base_fee);
        assert!(pending_operation.signatures.is_empty());
    }

    // Let's also verify that the XRPL base fee has been updated
    let query_config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(query_config.xrpl_base_fee, new_xrpl_base_fee);
}
