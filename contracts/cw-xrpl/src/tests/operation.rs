use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{
    AvailableTicketsResponse, PendingOperationsResponse, PendingRefundsResponse, XRPLTokensResponse,
};

use crate::state::{BridgeState, Config};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    relayer::Relayer,
    state::TokenState,
};
use cosmwasm_std::{coin, coins, Addr, Uint128};

#[test]
fn cancel_pending_operation() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let not_owner = &accounts[1];

    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let new_relayer = Relayer {
        cosmos_address: Addr::unchecked(not_owner),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // Test with 1 relayer and 1 evidence threshold first
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 3,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: None,osor_entry_point: None,
            },
        )
        .unwrap();

    // Register COSMOS Token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: FEE_DENOM.to_string(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::new(1000000000000),
            bridging_fee: Uint128::zero(),
        },
        &[],
    )
    .unwrap();

    // Set up enough tickets
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(10),
        },
        &[],
    )
    .unwrap();

    // Check that the ticket operation is there and cancel it
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

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .account_sequence
                .unwrap(),
        },
        &[],
    )
    .unwrap();

    // Should be gone and no tickets allocated
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

    let query_available_tickets: AvailableTicketsResponse = app
        .query(contract_addr.clone(), &QueryMsg::AvailableTickets {})
        .unwrap();

    assert!(query_available_tickets.tickets.is_empty());

    // This time we set them up correctly without cancelling
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(10),
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..11).collect()),
                }),
            },
        },
        &[],
    )
    .unwrap();

    // Create 1 pending operation of each type
    // TrustSet pending operation
    let issuer = generate_xrpl_address();
    let currency = "USD".to_string();
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            sending_precision: 4,
            max_holding_amount: Uint128::new(50000),
            bridging_fee: Uint128::zero(),
        },
        &coins(10_000_000u128, FEE_DENOM),
    )
    .unwrap();

    // CosmosToXRPLTransfer pending operation
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

    // RotateKeys operation
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RotateKeys {
            new_relayers: vec![new_relayer.clone()],
            new_evidence_threshold: 1,
        },
        &[],
    )
    .unwrap();

    // Check that 3 tickets are currently being used
    let query_available_tickets: AvailableTicketsResponse = app
        .query(contract_addr.clone(), &QueryMsg::AvailableTickets {})
        .unwrap();

    assert_eq!(query_available_tickets.tickets.len(), 7); // 10 - 3

    // Check that we have one of each pending operation types
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 3);

    // If someone that is not the owner tries to cancel it should fail
    let cancel_error = app
        .execute(
            Addr::unchecked(not_owner),
            contract_addr.clone(),
            &ExecuteMsg::CancelPendingOperation {
                operation_id: query_pending_operations.operations[0]
                    .ticket_sequence
                    .unwrap(),
            },
            &[],
        )
        .unwrap_err();

    assert!(cancel_error
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // If owner tries to cancel a pending operation that does not exist it should fail
    let cancel_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::CancelPendingOperation { operation_id: 50 },
            &[],
        )
        .unwrap_err();

    assert!(cancel_error.root_cause().to_string().contains(
        ContractError::PendingOperationNotFound {}
            .to_string()
            .as_str()
    ));

    // Cancel the first pending operation (trust set) and check that ticket is returned and token is put in Inactive state
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .ticket_sequence
                .unwrap(),
        },
        &[],
    )
    .unwrap();

    let query_xrpl_tokens: XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let token = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.currency == currency && t.issuer == issuer)
        .unwrap();

    assert_eq!(token.state, TokenState::Inactive);

    // Check that 2 tickets are currently being used (1 has been returned)
    let query_available_tickets: AvailableTicketsResponse = app
        .query(contract_addr.clone(), &QueryMsg::AvailableTickets {})
        .unwrap();

    assert_eq!(query_available_tickets.tickets.len(), 8);

    // Check that we the cancelled operation was removed
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 2);

    // Cancel the second pending operation (CosmosToXRPLTransfer), which should create a pending refund for the sender
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .ticket_sequence
                .unwrap(),
        },
        &[],
    )
    .unwrap();

    let query_pending_refunds: PendingRefundsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingRefunds {
                address: Addr::unchecked(signer),
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
    assert_eq!(
        query_pending_refunds.pending_refunds[0].coin,
        coin(1, FEE_DENOM)
    );

    // Check that 1 tickets is currently being used (2 have been returned)
    let query_available_tickets: AvailableTicketsResponse = app
        .query(contract_addr.clone(), &QueryMsg::AvailableTickets {})
        .unwrap();

    assert_eq!(query_available_tickets.tickets.len(), 9);

    // Check that we the cancelled operation was removed
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

    // Cancel the RotateKeys operation, it should keep the bridge halted and not rotate the relayers
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .ticket_sequence
                .unwrap(),
        },
        &[],
    )
    .unwrap();

    let query_config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(query_config.bridge_state, BridgeState::Halted);
    assert_eq!(query_config.relayers, vec![relayer]);

    // This should have returned all tickets and removed all pending operations from the queue
    // Check that all tickets are available (the 10 that we initially allocated)
    let query_available_tickets: AvailableTicketsResponse = app
        .query(contract_addr.clone(), &QueryMsg::AvailableTickets {})
        .unwrap();

    assert_eq!(query_available_tickets.tickets.len(), 10);

    // Check that we the cancelled operation was removed
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
}

#[test]
fn invalid_transaction_evidences() {
    let (mut app, accounts) = MockApp::new(&[("signer", &coins(100_000_000_000, FEE_DENOM))]);
    let signer = &accounts[0];
    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // Test with 1 relayer and 1 evidence threshold first
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 4,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: generate_xrpl_address(),
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: None,osor_entry_point: None,
            },
        )
        .unwrap();

    let tx_hash = generate_hash();
    let account_sequence = 1;
    let tickets: Vec<u64> = (1..6).collect();

    let invalid_evidences_input = vec![
        Evidence::XRPLTransactionResult {
            tx_hash: Some(tx_hash.clone()),
            account_sequence: None,
            ticket_sequence: None,
            transaction_result: TransactionResult::Rejected,
            operation_result: Some(OperationResult::TicketsAllocation {
                tickets: Some(tickets.clone()),
            }),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: Some(tx_hash.clone()),
            account_sequence: Some(account_sequence),
            ticket_sequence: Some(2),
            transaction_result: TransactionResult::Rejected,
            operation_result: Some(OperationResult::TicketsAllocation {
                tickets: Some(tickets.clone()),
            }),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: None,
            account_sequence: Some(account_sequence),
            ticket_sequence: None,
            transaction_result: TransactionResult::Rejected,
            operation_result: Some(OperationResult::TicketsAllocation {
                tickets: Some(tickets.clone()),
            }),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: Some(tx_hash.clone()),
            account_sequence: Some(account_sequence),
            ticket_sequence: None,
            transaction_result: TransactionResult::Rejected,
            operation_result: Some(OperationResult::TicketsAllocation {
                tickets: Some(tickets.clone()),
            }),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: Some(tx_hash.clone()),
            account_sequence: Some(account_sequence),
            ticket_sequence: None,
            transaction_result: TransactionResult::Invalid,
            operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: None,
            account_sequence: Some(account_sequence),
            ticket_sequence: None,
            transaction_result: TransactionResult::Invalid,
            operation_result: Some(OperationResult::TicketsAllocation {
                tickets: Some(tickets),
            }),
        },
    ];

    let expected_errors = vec![
        ContractError::InvalidTransactionResultEvidence {},
        ContractError::InvalidTransactionResultEvidence {},
        ContractError::InvalidSuccessfulTransactionResultEvidence {},
        ContractError::InvalidTicketAllocationEvidence {},
        ContractError::InvalidFailedTransactionResultEvidence {},
        ContractError::InvalidTicketAllocationEvidence {},
    ];

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence,
            number_of_tickets: Some(5),
        },
        &[],
    )
    .unwrap();

    for (index, evidence) in invalid_evidences_input.iter().enumerate() {
        let invalid_evidence = app
            .execute(
                Addr::unchecked(signer),
                contract_addr.clone(),
                &ExecuteMsg::SaveEvidence {
                    evidence: evidence.clone(),
                },
                &[],
            )
            .unwrap_err();

        assert!(invalid_evidence
            .root_cause()
            .to_string()
            .contains(expected_errors[index].to_string().as_str()));
    }
}

#[test]
fn unauthorized_access() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let not_owner = &accounts[1];

    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    // Test with 1 relayer and 1 evidence threshold first
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
                rate_limit_addr: None,osor_entry_point: None,
            },
        )
        .unwrap();

    // Try transfering from user that is not owner, should fail
    let transfer_error = app
        .execute(
            Addr::unchecked(not_owner),
            contract_addr.clone(),
            &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
                new_owner: not_owner.to_string(),
                expiry: None,
            }),
            &[],
        )
        .unwrap_err();

    assert!(transfer_error.root_cause().to_string().contains(
        ContractError::Ownership(cw_ownable::OwnershipError::NotOwner)
            .to_string()
            .as_str()
    ));

    // Try registering a cosmos token as not_owner, should fail
    let register_cosmos_error = app
        .execute(
            Addr::unchecked(not_owner),
            contract_addr.clone(),
            &ExecuteMsg::RegisterCosmosToken {
                denom: "any_denom".to_string(),
                decimals: 6,
                sending_precision: 1,
                max_holding_amount: Uint128::one(),
                bridging_fee: Uint128::zero(),
            },
            &[],
        )
        .unwrap_err();

    assert!(register_cosmos_error
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // Try registering an XRPL token as not_owner, should fail
    let register_xrpl_error = app
        .execute(
            Addr::unchecked(not_owner),
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: generate_xrpl_address(),
                currency: "USD".to_string(),
                sending_precision: 4,
                max_holding_amount: Uint128::new(50000),
                bridging_fee: Uint128::zero(),
            },
            &coins(10_000_000u128, FEE_DENOM),
        )
        .unwrap_err();

    assert!(register_xrpl_error
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // Trying to send from an address that is not a relayer should fail
    let relayer_error = app
        .execute(
            Addr::unchecked(not_owner),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: generate_xrpl_address(),
                    currency: "USD".to_string(),
                    amount: Uint128::new(100),
                    recipient: Addr::unchecked(signer),memo: None
                },
            },
            &[],
        )
        .unwrap_err();

    assert!(relayer_error
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // Try recovering tickets as not_owner, should fail
    let recover_tickets = app
        .execute(
            Addr::unchecked(not_owner),
            contract_addr.clone(),
            &ExecuteMsg::RecoverTickets {
                account_sequence: 1,
                number_of_tickets: Some(5),
            },
            &[],
        )
        .unwrap_err();

    assert!(recover_tickets
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));
}
