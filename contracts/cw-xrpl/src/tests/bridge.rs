use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{
    BridgeStateResponse, CosmosTokensResponse, ExecuteMsg, FeesCollectedResponse,
    PendingOperationsResponse, PendingRefundsResponse, QueryMsg, XRPLTokensResponse,
};
use crate::operation::{Operation, OperationType};
use crate::state::{BridgeState, Config, CosmosToken, TokenState};
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::token::full_denom;
use crate::{contract::XRP_CURRENCY, msg::InstantiateMsg, relayer::Relayer};
use cosmwasm_std::{coin, coins, Addr, Uint128};
use cw20::Cw20Coin;

#[test]
fn bridge_fee_collection_and_claiming() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
        ("account3", &coins(100_000_000_000, FEE_DENOM)),
        ("account4", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let accounts_number = accounts.len();

    let signer = &accounts[accounts_number - 1];
    let receiver = &accounts[accounts_number - 2];
    let xrpl_addresses: Vec<String> = (0..3).map(|_| generate_xrpl_address()).collect();
    let xrpl_pub_keys: Vec<String> = (0..3).map(|_| generate_xrpl_pub_key()).collect();

    let mut relayer_accounts = vec![];
    let mut relayers = vec![];

    for i in 0..accounts_number - 2 {
        relayer_accounts.push(accounts[i].to_string());
        relayers.push(Relayer {
            cosmos_address: Addr::unchecked(&accounts[i]),
            xrpl_address: xrpl_addresses[i as usize].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i as usize].to_string(),
        });
    }

    let xrpl_base_fee = 10;

    let bridge_xrpl_address = generate_xrpl_address();

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

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
                used_ticket_sequence_threshold: 14,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: bridge_xrpl_address.clone(),
                xrpl_base_fee,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: None,
                osor_entry_point: None,
            },
        )
        .unwrap();

    // Recover enough tickets
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(15),
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
                        tickets: Some((1..16).collect()),
                    }),
                },
            },
            &[],
        )
        .unwrap();
    }

    // We are going to issue 2 tokens, one XRPL originated and one Cosmos originated, with different fees.

    let issuer = generate_xrpl_address(); // Valid issuer
    let currency = "USD".to_string(); // Valid standard currency code
    let sending_precision = 10;
    let max_holding_amount = Uint128::new(5000000000000000); // 5e15
    let bridging_fee = Uint128::new(50000); // 5e4

    let subunit = "utest".to_string();
    let decimals = 6;
    let initial_amount = Uint128::new(100000000);

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CreateCosmosToken {
            subdenom: subunit.to_uppercase(),
            initial_balances: vec![Cw20Coin {
                address: receiver.to_string(),
                amount: initial_amount,
            }],
        },
        &coins(10_000_000u128, FEE_DENOM),
    )
    .unwrap();

    let denom = full_denom(&token_factory_addr, &subunit.to_uppercase());

    let test_token_cosmos = CosmosToken {
        denom: denom.clone(),
        decimals,
        sending_precision: 4,
        max_holding_amount: Uint128::new(10000000000), // 1e10
        bridging_fee: Uint128::new(300000),            // 3e5
        xrpl_currency: XRP_CURRENCY.to_string(),
        state: TokenState::Enabled,
    };

    // Register XRPL originated token and confirm trust set
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            sending_precision,
            max_holding_amount,
            bridging_fee,
        },
        &coins(10_000_000u128, FEE_DENOM),
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
                    account_sequence: None,
                    ticket_sequence: Some(1),
                    transaction_result: TransactionResult::Accepted,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    let query_xrpl_tokens: XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let xrpl_token = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == issuer && t.currency == currency)
        .unwrap();

    // Register Cosmos originated token
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: test_token_cosmos.denom,
            decimals: test_token_cosmos.decimals,
            sending_precision: test_token_cosmos.sending_precision,
            max_holding_amount: test_token_cosmos.max_holding_amount,
            bridging_fee: test_token_cosmos.bridging_fee,
        },
        &[],
    )
    .unwrap();

    let query_cosmos_tokens: CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let cosmos_token = query_cosmos_tokens
        .tokens
        .iter()
        .find(|t| t.denom == denom)
        .unwrap();

    // Let's bridge some tokens from XRPL to Cosmos multiple times and verify that the fees are collected correctly in each step
    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::new(1000000000050000), // 1e15 + 5e4 --> This should take the bridging fee (5e4) and truncate nothing
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap();
    }

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), xrpl_token.cosmos_denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "1000000000000000".to_string());

    // If we query fees for any random address that has no fees collected, it should return an empty array
    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked("any_address"),
            },
        )
        .unwrap();

    assert_eq!(query_fees_collected.fees_collected, vec![]);

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // 50000 / 3 = 16666.67 ---> Which means each relayer will have 16666 to claim and 2 tokens will stay in the fee remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![coin(16666, xrpl_token.cosmos_denom.clone())]
    );

    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::new(1000000000040000), // 1e15 + 4e4 --> This should take the bridging fee -> 1999999999990000 and truncate -> 1999999999900000
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap();
    }

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), xrpl_token.cosmos_denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "1999999999900000".to_string());

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // Each relayer is getting 140000 (+2 that were in the remainder) / 3 -> 140002 / 3 = 46667 and 1 token will stay in the remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![coin(63333, xrpl_token.cosmos_denom.clone())] // 16666 from before + 46667
    );

    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: issuer.clone(),
                    currency: currency.clone(),
                    amount: Uint128::new(1000000000000000), // 1e15 --> This should charge bridging fee -> 1999999999950000 and truncate -> 1999999999900000
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap();
    }

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), xrpl_token.cosmos_denom.clone())
        .unwrap();

    assert_eq!(request_balance.to_string(), "2999999999800000".to_string());

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // Each relayer is getting 100000 (+1 from remainder) / 3 -> 100001 / 3 = 33333 and 2 token will stay in the remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![coin(96666, xrpl_token.cosmos_denom.clone())] // 63333 from before + 33333
    );

    // Check that contract holds those tokens.
    let query_contract_balance = app
        .query_balance(contract_addr.clone(), xrpl_token.cosmos_denom.clone())
        .unwrap();
    assert_eq!(query_contract_balance.to_string(), "290000".to_string()); // 96666 * 3 + 2 in the remainder

    // Let's try to bridge some tokens back from Cosmos to XRPL and verify that the fees are also collected correctly
    let xrpl_receiver_address = generate_xrpl_address();
    app.execute(
        Addr::unchecked(receiver),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(1000000000020000, xrpl_token.cosmos_denom.clone()), // This should charge the bridging fee -> 999999999970000 and then truncate the rest -> 999999999900000
    )
    .unwrap();

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
            operation_type: OperationType::CosmosToXRPLTransfer {
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(999999999900000),
                max_amount: Some(Uint128::new(999999999900000)),
                sender: Addr::unchecked(receiver),
                recipient: xrpl_receiver_address.clone(),
            },
            xrpl_base_fee,
        }
    );

    // Confirm operation to clear tokens from contract
    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: query_pending_operations.operations[0].account_sequence,
                    ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                    transaction_result: TransactionResult::Accepted,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // Each relayer is getting 120000 (+2 from remainder) / 3 -> 120002 / 3 = 40000 and 2 token will stay in the remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![coin(136666, xrpl_token.cosmos_denom.clone())] // 96666 from before + 40000
    );

    // Let's bridge some tokens again but this time with the optional amount, to check that bridge fees are collected correctly and
    // when rejected, full amount without bridge fees is available to be claimed back by user.
    let deliver_amount = Some(Uint128::new(700000000020000));

    // If we send an amount, that after truncation and bridge fees is higher than max amount, it should fail
    let max_amount_error = app
        .execute(
            Addr::unchecked(receiver),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: xrpl_receiver_address.clone(),
                deliver_amount: Some(Uint128::new(1000000000010000)),
            },
            &coins(1000000000020000, xrpl_token.cosmos_denom.clone()), // After fees and truncation -> 1000000000000000 > 999999999900000
        )
        .unwrap_err();

    assert!(max_amount_error
        .root_cause()
        .to_string()
        .contains(ContractError::InvalidDeliverAmount {}.to_string().as_str()));

    app.execute(
        Addr::unchecked(receiver),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount, // This will be truncated to 700000000000000
        },
        &coins(1000000000020000, xrpl_token.cosmos_denom.clone()), // This should charge the bridging fee -> 999999999970000 and then truncate the rest -> 999999999900000
    )
    .unwrap();

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
            ticket_sequence: Some(3),
            account_sequence: None,
            signatures: vec![],
            operation_type: OperationType::CosmosToXRPLTransfer {
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: Uint128::new(700000000000000),
                max_amount: Some(Uint128::new(999999999900000)),
                sender: Addr::unchecked(receiver),
                recipient: xrpl_receiver_address.clone(),
            },
            xrpl_base_fee
        }
    );

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // Each relayer is getting 120000 (+2 from remainder) / 3 -> 120002 / 3 = 40000 and 2 token will stay in the remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![coin(176666, xrpl_token.cosmos_denom.clone())] // 136666 from before + 40000
    );

    // If we reject the operation, 999999999900000 (max_amount after bridge fees and truncation) should be able to be claimed back by the user
    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: query_pending_operations.operations[0].account_sequence,
                    ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                    transaction_result: TransactionResult::Rejected,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    let query_pending_refunds: PendingRefundsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingRefunds {
                address: Addr::unchecked(receiver),
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
    assert_eq!(
        query_pending_refunds.pending_refunds[0].coin,
        coin(999999999900000, xrpl_token.cosmos_denom.clone())
    );

    // Let's claim it back
    app.execute(
        Addr::unchecked(receiver),
        contract_addr.clone(),
        &ExecuteMsg::ClaimRefund {
            pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
        },
        &[],
    )
    .unwrap();

    // Now let's bridge tokens from Cosmos to XRPL and verify that the fees are collected correctly in each step and accumulated with the previous ones

    // Trying to send less than the bridging fees should fail
    let bridging_error = app
        .execute(
            Addr::unchecked(receiver),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: xrpl_receiver_address.clone(),
                deliver_amount: None,
            },
            &coins(100, denom.clone()),
        )
        .unwrap_err();

    assert!(bridging_error.root_cause().to_string().contains(
        ContractError::CannotCoverBridgingFees {}
            .to_string()
            .as_str()
    ));

    app.execute(
        Addr::unchecked(receiver),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(600010, denom.clone()), // This should charge briding fee -> 300010 and then truncate the rest -> 300000
    )
    .unwrap();

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
            ticket_sequence: Some(4),
            account_sequence: None,
            signatures: vec![],
            operation_type: OperationType::CosmosToXRPLTransfer {
                issuer: bridge_xrpl_address.clone(),
                currency: cosmos_token.xrpl_currency.clone(),
                amount: Uint128::new(300000000000000),
                max_amount: Some(Uint128::new(300000000000000)),
                sender: Addr::unchecked(receiver),
                recipient: xrpl_receiver_address.clone(),
            },
            xrpl_base_fee
        }
    );

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // Each relayer is getting 300010 / 3 -> 100003 and 1 token will stay in the remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![
            coin(176666, xrpl_token.cosmos_denom.clone()),
            coin(100003, denom.clone())
        ]
    );

    // Confirm operation
    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: query_pending_operations.operations[0].account_sequence,
                    ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                    transaction_result: TransactionResult::Accepted,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    app.execute(
        Addr::unchecked(receiver),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(900000, denom.clone()), // This charge the entire bridging fee (300000) and truncate nothing
    )
    .unwrap();

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
            ticket_sequence: Some(5),
            account_sequence: None,
            signatures: vec![],
            operation_type: OperationType::CosmosToXRPLTransfer {
                issuer: bridge_xrpl_address.clone(),
                currency: cosmos_token.xrpl_currency.clone(),
                amount: Uint128::new(600000000000000),
                max_amount: Some(Uint128::new(600000000000000)),
                sender: Addr::unchecked(receiver),
                recipient: xrpl_receiver_address.clone(),
            },
            xrpl_base_fee,
        }
    );

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // Each relayer is getting 300000 (+1 from remainder) / 3 -> 100000 and 1 token will stay in the remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![
            coin(176666, xrpl_token.cosmos_denom.clone()),
            coin(200003, denom.clone()) // 100003 + 100000
        ]
    );

    // Confirm operation
    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLTransactionResult {
                    tx_hash: Some(tx_hash.clone()),
                    account_sequence: query_pending_operations.operations[0].account_sequence,
                    ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                    transaction_result: TransactionResult::Accepted,
                    operation_result: None,
                },
            },
            &[],
        )
        .unwrap();
    }

    // Let's try to send the Cosmos originated token in the opposite direction (from XRPL to Cosmos) and see that fees are also accumulated correctly.
    let previous_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    let tx_hash = generate_hash();
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::SaveEvidence {
                evidence: Evidence::XRPLToCosmosTransfer {
                    tx_hash: tx_hash.clone(),
                    issuer: bridge_xrpl_address.clone(),
                    currency: cosmos_token.xrpl_currency.clone(),
                    amount: Uint128::new(650010000000000), // 650010000000000 will convert to 650010, which after charging bridging fees (300000) and truncating (10) will send 350000 to the receiver
                    recipient: Addr::unchecked(receiver),
                },
            },
            &[],
        )
        .unwrap();
    }

    let new_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(
        new_balance.u128(),
        previous_balance.u128().checked_add(350000u128).unwrap()
    );

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // Each relayer will be getting 300010 (+1 from the remainder) / 3 -> 300011 / 3 = 100003 and 2 tokens will stay in the remainders for next collection
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![
            coin(176666, xrpl_token.cosmos_denom.clone()),
            coin(300006, denom.clone()) // 200003 from before + 100003
        ]
    );

    // Let's test the claiming

    // If we claim more than available, it should fail
    let claim_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[0]),
            contract_addr.clone(),
            &ExecuteMsg::ClaimRelayerFees {
                amounts: vec![
                    coin(176666, xrpl_token.cosmos_denom.clone()),
                    coin(300007, denom.clone()), // +1
                ],
            },
            &[],
        )
        .unwrap_err();

    assert!(claim_error.root_cause().to_string().contains(
        ContractError::NotEnoughFeesToClaim {
            denom: denom.clone(),
            amount: Uint128::new(300007)
        }
        .to_string()
        .as_str()
    ));

    // If we separate token claim into two coins but ask for too much it should also fail
    let claim_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[0]),
            contract_addr.clone(),
            &ExecuteMsg::ClaimRelayerFees {
                amounts: vec![
                    coin(176666, xrpl_token.cosmos_denom.clone()),
                    coin(300006, denom.clone()),
                    coin(1, denom.clone()), // Extra token claim that is too much
                ],
            },
            &[],
        )
        .unwrap_err();

    assert!(claim_error.root_cause().to_string().contains(
        ContractError::NotEnoughFeesToClaim {
            denom: denom.clone(),
            amount: Uint128::new(1)
        }
        .to_string()
        .as_str()
    ));

    // If we claim everything except 1 token, it should work
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::ClaimRelayerFees {
                amounts: vec![
                    coin(176666, xrpl_token.cosmos_denom.clone()),
                    coin(300005, denom.clone()),
                ],
            },
            &[],
        )
        .unwrap();
    }

    let query_fees_collected: FeesCollectedResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::FeesCollected {
                relayer_address: Addr::unchecked(&relayer_accounts[0]),
            },
        )
        .unwrap();

    // There should be only 1 token left in the remainders
    assert_eq!(
        query_fees_collected.fees_collected,
        vec![coin(1, denom.clone())]
    );

    // If we try to claim a token that is not in the claimable array, it should fail
    let claim_error = app
        .execute(
            Addr::unchecked(&relayer_accounts[0]),
            contract_addr.clone(),
            &ExecuteMsg::ClaimRelayerFees {
                amounts: vec![coin(1, xrpl_token.cosmos_denom.clone())],
            },
            &[],
        )
        .unwrap_err();

    assert!(claim_error.root_cause().to_string().contains(
        ContractError::NotEnoughFeesToClaim {
            denom: xrpl_token.cosmos_denom.clone(),
            amount: Uint128::new(1)
        }
        .to_string()
        .as_str()
    ));

    // Claim the token that is left to claim
    for relayer in &relayer_accounts {
        app.execute(
            Addr::unchecked(relayer),
            contract_addr.clone(),
            &ExecuteMsg::ClaimRelayerFees {
                amounts: vec![coin(1, denom.clone())],
            },
            &[],
        )
        .unwrap();
    }

    // Let's check the balances of the relayers
    for relayer in &relayer_accounts {
        let request_balance_token1 = app
            .query_balance(Addr::unchecked(relayer), xrpl_token.cosmos_denom.clone())
            .unwrap();
        let request_balance_token2 = app
            .query_balance(Addr::unchecked(relayer), denom.clone())
            .unwrap();

        assert_eq!(request_balance_token1.to_string(), "176666".to_string()); // 530000 / 3 = 183333
        assert_eq!(request_balance_token2.to_string(), "300006".to_string()); // 900020 / 3 = 300006
    }

    // We check that everything has been claimed
    for relayer in &relayer_accounts {
        let query_fees_collected: FeesCollectedResponse = app
            .query(
                contract_addr.clone(),
                &QueryMsg::FeesCollected {
                    relayer_address: Addr::unchecked(relayer),
                },
            )
            .unwrap();

        assert_eq!(query_fees_collected.fees_collected, vec![]);
    }

    // Check that final balance in the contract matches with those fees
    let query_contract_balance = app
        .query_balance(contract_addr.clone(), xrpl_token.cosmos_denom.clone())
        .unwrap();
    assert_eq!(query_contract_balance.to_string(), "2".to_string()); // What is stored in the remainder

    let query_contract_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    // Amount that the user can still bridge back (he has on XRPL) from the token he has sent
    // Sent: 300000 + 600000 (after applying fees and truncating)
    // Sent back: 650010
    // Result: 300000 + 600000 - 650010 = 249990
    // + 2 tokens that have not been claimed yet because the relayers can't claim them = 249992
    assert_eq!(query_contract_balance.to_string(), "249992".to_string());
}

#[test]
fn bridge_halting_and_resuming() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let relayer_account = &accounts[1];
    let new_relayer_account = &accounts[2];
    let relayer = Relayer {
        cosmos_address: Addr::unchecked(relayer_account),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let bridge_xrpl_address = generate_xrpl_address();

    let xrpl_base_fee = 10;

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();

    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 9,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: bridge_xrpl_address.clone(),
                xrpl_base_fee,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: None,
                osor_entry_point: None,
            },
        )
        .unwrap();

    // Halt the bridge and check that we can't send any operations except allowed ones
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::HaltBridge {},
        &[],
    )
    .unwrap();

    // Query bridge state to confirm it's halted
    let query_bridge_state: BridgeStateResponse = app
        .query(contract_addr.clone(), &QueryMsg::BridgeState {})
        .unwrap();

    assert_eq!(query_bridge_state.state, BridgeState::Halted);

    // Setting up some tickets should be allowed
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
        Addr::unchecked(relayer_account),
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

    // Trying to register tokens should fail
    let bridge_halted_error = app
        .execute(
            Addr::unchecked(signer),
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

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    let bridge_halted_error = app
        .execute(
            Addr::unchecked(signer),
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

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    // Sending from Cosmos to XRPL should fail
    let bridge_halted_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::SendToXRPL {
                recipient: generate_xrpl_address(),
                deliver_amount: None,
            },
            &coins(1, FEE_DENOM),
        )
        .unwrap_err();

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    // Updating tokens should fail too
    let bridge_halted_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateXRPLToken {
                issuer: "any_issuer".to_string(),
                currency: "any_currency".to_string(),
                state: Some(TokenState::Disabled),
                sending_precision: None,
                bridging_fee: None,
                max_holding_amount: None,
            },
            &[],
        )
        .unwrap_err();

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    let bridge_halted_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::UpdateCosmosToken {
                denom: "any_denom".to_string(),
                state: Some(TokenState::Disabled),
                sending_precision: None,
                bridging_fee: None,
                max_holding_amount: None,
            },
            &[],
        )
        .unwrap_err();

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    // Claiming pending refunds or relayers fees should fail
    let bridge_halted_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::ClaimRefund {
                pending_refund_id: "any_id".to_string(),
            },
            &[],
        )
        .unwrap_err();

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    let bridge_halted_error = app
        .execute(
            Addr::unchecked(relayer_account),
            contract_addr.clone(),
            &ExecuteMsg::ClaimRelayerFees {
                amounts: vec![coin(1, FEE_DENOM)],
            },
            &[],
        )
        .unwrap_err();

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    // Resuming the bridge should work
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::ResumeBridge {},
        &[],
    )
    .unwrap();

    // Query bridge state to confirm it's active
    let query_bridge_state: BridgeStateResponse = app
        .query(contract_addr.clone(), &QueryMsg::BridgeState {})
        .unwrap();

    assert_eq!(query_bridge_state.state, BridgeState::Active);

    // Halt it again to send some allowed operations
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::HaltBridge {},
        &[],
    )
    .unwrap();

    // Perform a simple key rotation, should be allowed
    let new_relayer = Relayer {
        cosmos_address: Addr::unchecked(new_relayer_account),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    // We perform a key rotation
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

    // Let's query the pending operations to see that this operation was saved correctly
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
                new_relayers: vec![new_relayer.clone()],
                new_evidence_threshold: 1
            },
            xrpl_base_fee,
        }
    );

    // Resuming now should not be allowed because we have a pending key rotation
    let resume_error = app
        .execute(
            Addr::unchecked(signer),
            contract_addr.clone(),
            &ExecuteMsg::ResumeBridge {},
            &[],
        )
        .unwrap_err();

    assert!(resume_error
        .root_cause()
        .to_string()
        .contains(ContractError::RotateKeysOngoing {}.to_string().as_str()));

    // Sending signatures should be allowed with the bridge halted and with pending operations
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveSignature {
            operation_id: 1,
            operation_version: 1,
            signature: "signature".to_string(),
        },
        &[],
    )
    .unwrap();

    // Sending an evidence for something that is not a RotateKeys should fail
    let bridge_halted_error = app
        .execute(
            Addr::unchecked(relayer_account),
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
        )
        .unwrap_err();

    assert!(bridge_halted_error
        .root_cause()
        .to_string()
        .contains(ContractError::BridgeHalted {}.to_string().as_str()));

    // Sending an evidence confirming a Key rotation should work and should also activate the bridge
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    // Query bridge state to confirm it's still halted
    let query_bridge_state: BridgeStateResponse = app
        .query(contract_addr.clone(), &QueryMsg::BridgeState {})
        .unwrap();

    assert_eq!(query_bridge_state.state, BridgeState::Halted);

    // Query config to see that relayers have been correctly rotated
    let config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
        .unwrap();

    assert_eq!(config.relayers, vec![new_relayer]);

    // We should now be able to resume the bridge because the key rotation has been confirmed
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::ResumeBridge {},
        &[],
    )
    .unwrap();

    // Query bridge state to confirm it's now active
    let query_bridge_state: BridgeStateResponse = app
        .query(contract_addr.clone(), &QueryMsg::BridgeState {})
        .unwrap();

    assert_eq!(query_bridge_state.state, BridgeState::Active);

    // Halt the bridge should not be possible by an address that is not owner or current relayer
    let halt_error = app
        .execute(
            Addr::unchecked(relayer_account),
            contract_addr.clone(),
            &ExecuteMsg::HaltBridge {},
            &[],
        )
        .unwrap_err();

    assert!(halt_error
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // Current relayer should be allowed to halt it
    app.execute(
        Addr::unchecked(new_relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::HaltBridge {},
        &[],
    )
    .unwrap();

    let query_bridge_state: BridgeStateResponse = app
        .query(contract_addr.clone(), &QueryMsg::BridgeState {})
        .unwrap();

    assert_eq!(query_bridge_state.state, BridgeState::Halted);

    // Triggering a fee update during halted bridge should work
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::UpdateXRPLBaseFee { xrpl_base_fee: 600 },
        &[],
    )
    .unwrap();
}
