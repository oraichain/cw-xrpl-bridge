use crate::contract::{MAX_COSMOS_TOKEN_DECIMALS, XRPL_DENOM_PREFIX};
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{PendingOperationsResponse, XRPLTokensResponse};
use crate::operation::{Operation, OperationType};
use crate::state::{Config, CosmosToken, XRPLToken};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::{
    contract::XRP_CURRENCY,
    msg::{CosmosTokensResponse, ExecuteMsg, InstantiateMsg, QueryMsg},
    relayer::Relayer,
    state::TokenState,
};
use cosmwasm_std::{coins, Addr, Uint128};
use token_bindings::DenomsByCreatorResponse;

#[test]
fn cancel_pending_operation() {
    let app = CosmosTestApp::new();
    let signer = app
        .init_account(&coins(100_000_000_000, FEE_DENOM))
        .unwrap();
    let not_owner = app
        .init_account(&coins(100_000_000_000, FEE_DENOM))
        .unwrap();

    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let new_relayer = Relayer {
        cosmos_address: Addr::unchecked(not_owner.address()),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let contract_addr = store_and_instantiate(
        &wasm,
        Addr::unchecked(signer),
        Addr::unchecked(signer),
        vec![relayer.clone()],
        1,
        3,
        Uint128::new(TRUST_SET_LIMIT_AMOUNT),
        query_issue_fee(&asset_ft),
        generate_xrpl_address(),
        10,
    );

    // Register COSMOS Token
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: FEE_DENOM.to_string(),
            decimals: 6,
            sending_precision: 6,
            max_holding_amount: Uint128::new(1000000000000),
            bridging_fee: Uint128::zero(),
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    // Set up enough tickets
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(10),
        },
        &[],
        Addr::unchecked(signer),
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
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .account_sequence
                .unwrap(),
        },
        &[],
        Addr::unchecked(signer),
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
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(10),
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    app.execute(
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
        Addr::unchecked(signer),
    )
    .unwrap();

    // Create 1 pending operation of each type
    // TrustSet pending operation
    let issuer = generate_xrpl_address();
    let currency = "USD".to_string();
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            sending_precision: 4,
            max_holding_amount: Uint128::new(50000),
            bridging_fee: Uint128::zero(),
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    // CosmosToXRPLTransfer pending operation
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: generate_xrpl_address(),
            deliver_amount: None,
        },
        &coins(1, FEE_DENOM.to_string()),
        Addr::unchecked(signer),
    )
    .unwrap();

    // RotateKeys operation
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::RotateKeys {
            new_relayers: vec![new_relayer.clone()],
            new_evidence_threshold: 1,
        },
        &[],
        Addr::unchecked(signer),
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
    let cancel_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::CancelPendingOperation {
                operation_id: query_pending_operations.operations[0]
                    .ticket_sequence
                    .unwrap(),
            },
            &[],
            &not_owner,
        )
        .unwrap_err();

    assert!(cancel_error
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // If owner tries to cancel a pending operation that does not exist it should fail
    let cancel_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::CancelPendingOperation { operation_id: 50 },
            &[],
            Addr::unchecked(signer),
        )
        .unwrap_err();

    assert!(cancel_error.root_cause().to_string().contains(
        ContractError::PendingOperationNotFound {}
            .to_string()
            .as_str()
    ));

    // Cancel the first pending operation (trust set) and check that ticket is returned and token is put in Inactive state
    app.execute(
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .ticket_sequence
                .unwrap(),
        },
        &[],
        Addr::unchecked(signer),
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
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .ticket_sequence
                .unwrap(),
        },
        &[],
        Addr::unchecked(signer),
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
        contract_addr.clone(),
        &ExecuteMsg::CancelPendingOperation {
            operation_id: query_pending_operations.operations[0]
                .ticket_sequence
                .unwrap(),
        },
        &[],
        Addr::unchecked(signer),
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
    let app = CosmosTestApp::new();
    let signer = app
        .init_account(&coins(100_000_000_000, FEE_DENOM))
        .unwrap();

    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let contract_addr = store_and_instantiate(
        &wasm,
        Addr::unchecked(signer),
        Addr::unchecked(signer),
        vec![relayer],
        1,
        4,
        Uint128::new(TRUST_SET_LIMIT_AMOUNT),
        query_issue_fee(&asset_ft),
        generate_xrpl_address(),
        10,
    );

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
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence,
            number_of_tickets: Some(5),
        },
        &[],
        Addr::unchecked(signer),
    )
    .unwrap();

    for (index, evidence) in invalid_evidences_input.iter().enumerate() {
        let invalid_evidence = wasm
            .execute(
                contract_addr.clone(),
                &ExecuteMsg::SaveEvidence {
                    evidence: evidence.clone(),
                },
                &[],
                Addr::unchecked(signer),
            )
            .unwrap_err();

        assert!(invalid_evidence
            .to_string()
            .contains(expected_errors[index].to_string().as_str()));
    }
}

#[test]
fn unauthorized_access() {
    let app = CosmosTestApp::new();
    let signer = app
        .init_account(&coins(100_000_000_000, FEE_DENOM))
        .unwrap();

    let not_owner = app
        .init_account(&coins(100_000_000_000, FEE_DENOM))
        .unwrap();

    let relayer = Relayer {
        cosmos_address: Addr::unchecked(signer),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let contract_addr = store_and_instantiate(
        &wasm,
        Addr::unchecked(signer),
        Addr::unchecked(signer),
        vec![relayer],
        1,
        50,
        Uint128::new(TRUST_SET_LIMIT_AMOUNT),
        query_issue_fee(&asset_ft),
        generate_xrpl_address(),
        10,
    );

    // Try transfering from user that is not owner, should fail
    let transfer_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
                new_owner: not_owner.address(),
                expiry: None,
            }),
            &[],
            &not_owner,
        )
        .unwrap_err();

    assert!(transfer_error.root_cause().to_string().contains(
        ContractError::Ownership(cw_ownable::OwnershipError::NotOwner)
            .to_string()
            .as_str()
    ));

    // Try registering a cosmos token as not_owner, should fail
    let register_cosmos_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::RegisterCosmosToken {
                denom: "any_denom".to_string(),
                decimals: 6,
                sending_precision: 1,
                max_holding_amount: Uint128::one(),
                bridging_fee: Uint128::zero(),
            },
            &[],
            &not_owner,
        )
        .unwrap_err();

    assert!(register_cosmos_error
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // Try registering an XRPL token as not_owner, should fail
    let register_xrpl_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::RegisterXRPLToken {
                issuer: generate_xrpl_address(),
                currency: "USD".to_string(),
                sending_precision: 4,
                max_holding_amount: Uint128::new(50000),
                bridging_fee: Uint128::zero(),
            },
            &[],
            &not_owner,
        )
        .unwrap_err();

    assert!(register_xrpl_error
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // Trying to send from an address that is not a relayer should fail
    let relayer_error = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: generate_hash(),
                    issuer: generate_xrpl_address(),
                    currency: "USD".to_string(),
                    amount: Uint128::new(100),
                    recipient: Addr::unchecked(signer),
                },
            },
            &[],
            &not_owner,
        )
        .unwrap_err();

    assert!(relayer_error
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // Try recovering tickets as not_owner, should fail
    let recover_tickets = wasm
        .execute(
            contract_addr.clone(),
            &ExecuteMsg::RecoverTickets {
                account_sequence: 1,
                number_of_tickets: Some(5),
            },
            &[],
            &not_owner,
        )
        .unwrap_err();

    assert!(recover_tickets
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));
}
