use crate::contract::{CHANNEL, XRP_ISSUER};
use crate::error::ContractError;
use crate::evidence::{Evidence, OperationResult, TransactionResult};
use crate::msg::{
    CosmosTokensResponse, ExecuteMsg, PendingOperationsResponse, PendingRefundsResponse, QueryMsg,
    XRPLTokensResponse,
};
use crate::operation::{Operation, OperationType};
use crate::state::Config;
use crate::tests::helper::{
    generate_hash, generate_xrpl_address, generate_xrpl_pub_key, MockApp, FEE_DENOM,
    TRUST_SET_LIMIT_AMOUNT,
};
use crate::token::{build_xrpl_token_key, full_denom};
use crate::{contract::XRP_CURRENCY, msg::InstantiateMsg, relayer::Relayer};
use cosmwasm_std::{coin, coins, Addr, Uint128};

use cw20::Cw20Coin;
use rate_limiter::packet::Packet;
use rate_limiter::state::Quota;
use rate_limiter::{
    msg::{
        ExecuteMsg as RateLimitExecuteMsg, InstantiateMsg as RateLimitInitMsg,
        QueryMsg as RateLimitQueryMsg, QuotaMsg,
    },
    state::RateLimit,
};

#[test]
fn test_register_rate_limit() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
        ("account3", &coins(100_000_000_000, FEE_DENOM)),
    ]);
    let accounts_number = accounts.len();

    let signer = &accounts[accounts_number - 1];
    let receiver = &accounts[accounts_number - 2];
    let xrpl_addresses = vec![generate_xrpl_address(), generate_xrpl_address()];

    let xrpl_pub_keys = vec![generate_xrpl_pub_key(), generate_xrpl_pub_key()];

    let mut relayer_accounts = vec![];
    let mut relayers = vec![];

    for i in 0..accounts_number - 2 {
        let account = &accounts[i];
        relayer_accounts.push(account.clone());
        relayers.push(Relayer {
            cosmos_address: Addr::unchecked(account),
            xrpl_address: xrpl_addresses[i].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i].to_string(),
        });
    }

    let bridge_xrpl_address = generate_xrpl_address();

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();
    let rate_limit_addr = app
        .create_rate_limit_contract(Addr::unchecked(signer), &RateLimitInitMsg { paths: vec![] })
        .unwrap();

    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayers[0].clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 2,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address,
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: Some(rate_limit_addr.clone()),
                osor_entry_point: None,
            },
        )
        .unwrap();

    let issuer = generate_xrpl_address();
    let currency = "USD".to_string();
    let denom = build_xrpl_token_key(&issuer, &currency);

    // register rate limit failed, unauthorized
    let res = app
        .execute(
            Addr::unchecked(receiver),
            contract_addr.clone(),
            &ExecuteMsg::AddRateLimit {
                xrpl_denom: denom.clone(),
                quotas: vec![QuotaMsg {
                    name: "daily".to_string(),
                    duration: 864000,
                    max_send: Uint128::new(1000000),
                    max_receive: Uint128::new(1000000),
                }],
            },
            &[],
        )
        .unwrap_err();
    assert!(res
        .root_cause()
        .to_string()
        .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));

    // register successful
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::AddRateLimit {
            xrpl_denom: denom.clone(),
            quotas: vec![QuotaMsg {
                name: "daily".to_string(),
                duration: 864000,
                max_send: Uint128::new(1000000),
                max_receive: Uint128::new(1000000),
            }],
        },
        &[],
    )
    .unwrap();

    // query rate limit
    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: denom.clone(),
            },
        )
        .unwrap();
    assert_eq!(rate_limits.len(), 1);
    assert_eq!(
        rate_limits[0].quota,
        Quota {
            name: "daily".to_string(),
            duration: 864000,
            max_send: Uint128::new(1000000),
            max_recv: Uint128::new(1000000),
        }
    );
    assert_eq!(rate_limits[0].flow.inflow, Uint128::zero());
    assert_eq!(rate_limits[0].flow.outflow, Uint128::zero());

    // fake send packet success, dont reach rate limit
    app.execute(
        contract_addr.clone(),
        rate_limit_addr.clone(),
        &RateLimitExecuteMsg::SendPacket {
            packet: Packet {
                channel: CHANNEL.to_string(),
                denom: denom.clone(),
                amount: Uint128::new(600000),
            },
        },
        &[],
    )
    .unwrap();
    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: denom.clone(),
            },
        )
        .unwrap();
    assert_eq!(rate_limits[0].flow.inflow, Uint128::zero());
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(600000));

    // try send other packet, got rate limit
    app.execute(
        contract_addr.clone(),
        rate_limit_addr.clone(),
        &RateLimitExecuteMsg::SendPacket {
            packet: Packet {
                channel: CHANNEL.to_string(),
                denom: denom.clone(),
                amount: Uint128::new(600000),
            },
        },
        &[],
    )
    .unwrap_err();

    // fake receive packet
    app.execute(
        contract_addr.clone(),
        rate_limit_addr.clone(),
        &RateLimitExecuteMsg::RecvPacket {
            packet: Packet {
                channel: CHANNEL.to_string(),
                denom: denom.clone(),
                amount: Uint128::new(600000),
            },
        },
        &[],
    )
    .unwrap();
    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: denom.clone(),
            },
        )
        .unwrap();
    assert_eq!(rate_limits[0].flow.inflow, Uint128::new(600000));
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(600000));

    // submit send packet successful
    app.execute(
        contract_addr.clone(),
        rate_limit_addr.clone(),
        &RateLimitExecuteMsg::SendPacket {
            packet: Packet {
                channel: CHANNEL.to_string(),
                denom: denom.clone(),
                amount: Uint128::new(600000),
            },
        },
        &[],
    )
    .unwrap();

    // reset rate limit
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::ResetRateLimitQuota {
            xrpl_denom: denom.clone(),
            quota_id: "daily".to_string(),
        },
        &[],
    )
    .unwrap();
    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: denom.clone(),
            },
        )
        .unwrap();
    assert_eq!(rate_limits[0].flow.inflow, Uint128::zero());
    assert_eq!(rate_limits[0].flow.outflow, Uint128::zero());
}

#[test]
fn send_xrpl_originated_tokens_from_xrpl_to_cosmos_with_rate_limit() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
        ("account3", &coins(100_000_000_000, FEE_DENOM)),
    ]);
    let accounts_number = accounts.len();

    let signer = &accounts[accounts_number - 1];
    let receiver = &accounts[accounts_number - 2];
    let xrpl_addresses = vec![generate_xrpl_address(), generate_xrpl_address()];

    let xrpl_pub_keys = vec![generate_xrpl_pub_key(), generate_xrpl_pub_key()];

    let mut relayer_accounts = vec![];
    let mut relayers = vec![];

    for i in 0..accounts_number - 2 {
        let account = &accounts[i];
        relayer_accounts.push(account.clone());
        relayers.push(Relayer {
            cosmos_address: Addr::unchecked(account),
            xrpl_address: xrpl_addresses[i].to_string(),
            xrpl_pub_key: xrpl_pub_keys[i].to_string(),
        });
    }

    let bridge_xrpl_address = generate_xrpl_address();

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();
    let rate_limit_addr = app
        .create_rate_limit_contract(Addr::unchecked(signer), &RateLimitInitMsg { paths: vec![] })
        .unwrap();

    // Test with 1 relayer and 1 evidence threshold first
    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayers[0].clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 2,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address,
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: Some(rate_limit_addr),
                osor_entry_point: None,
            },
        )
        .unwrap();

    let issuer = generate_xrpl_address();
    let currency = "USD".to_string();
    let sending_precision = 15;
    let max_holding_amount = Uint128::new(50000);
    let bridging_fee = Uint128::zero();
    let denom = build_xrpl_token_key(&issuer, &currency);

    // init rate limit (1000000 per side)
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::AddRateLimit {
            xrpl_denom: denom.clone(),
            quotas: vec![QuotaMsg {
                name: "daily".to_string(),
                duration: 86400,
                max_send: Uint128::new(1000),
                max_receive: Uint128::new(1000),
            }],
        },
        &[],
    )
    .unwrap();

    // Set up enough tickets first to allow registering tokens.
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(3),
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..4).collect()),
                }),
            },
        },
        &[],
    )
    .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterXRPLToken {
            issuer: issuer.clone(),
            currency: currency.clone(),
            sending_precision: sending_precision.clone(),
            max_holding_amount: max_holding_amount.clone(),
            bridging_fee: bridging_fee,
        },
        &coins(10_000_000u128, FEE_DENOM),
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

    let denom = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == issuer && t.currency == currency)
        .unwrap()
        .cosmos_denom
        .clone();

    let hash = generate_hash();
    let amount = Uint128::new(600);

    // Activate the token
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
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    // Bridge with 1 relayer should immediately mint and send to the receiver address
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: hash.clone(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
                memo: None,
            },
        },
        &[],
    )
    .unwrap();

    let request_balance = app
        .query_balance(Addr::unchecked(receiver), denom.clone())
        .unwrap();

    assert_eq!(request_balance, amount);

    // If we try to bridge to the contract one more time with the same amount, it should fail because reach rate limit

    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
                memo: None,
            },
        },
        &[],
    )
    .unwrap_err();

    // after time period, we can bridge again
    app.increase_time(86400);
    app.execute(
        Addr::unchecked(&relayer_accounts[0]),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: issuer.clone(),
                currency: currency.clone(),
                amount: amount.clone(),
                recipient: Addr::unchecked(receiver),
                memo: None,
            },
        },
        &[],
    )
    .unwrap();
}

#[test]
fn send_cosmos_originated_tokens_from_xrpl_to_cosmos_with_rate_limit() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let sender = &accounts[1];
    let relayer_account = &accounts[2];
    let relayer = Relayer {
        cosmos_address: Addr::unchecked(relayer_account),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let xrpl_receiver_address = generate_xrpl_address();
    let bridge_xrpl_address = generate_xrpl_address();

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(sender)).unwrap();
    let rate_limit_addr = app
        .create_rate_limit_contract(Addr::unchecked(signer), &RateLimitInitMsg { paths: vec![] })
        .unwrap();

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
                xrpl_base_fee: 10,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: Some(rate_limit_addr.clone()),
                osor_entry_point: None,
            },
        )
        .unwrap();

    // Add enough tickets for all our test operations
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

    // Let's issue a token where decimals are less than an XRPL token decimals to the sender and register it.
    let subunit = "utest".to_string();
    let decimals = 6;
    let initial_amount = Uint128::new(100000000000000000000);

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::CreateCosmosToken {
            subdenom: subunit.to_uppercase(),
            initial_balances: vec![Cw20Coin {
                address: signer.to_string(),
                amount: initial_amount,
            }],
        },
        &coins(10_000_000u128, FEE_DENOM),
    )
    .unwrap();

    let denom = full_denom(&token_factory_addr, &subunit.to_uppercase());

    // Send all initial amount tokens to the sender so that we can correctly test freezing without sending to the issuer

    app.send_coins(
        Addr::unchecked(signer),
        Addr::unchecked(sender),
        &coins(initial_amount.u128(), denom.clone()),
    )
    .unwrap();

    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RegisterCosmosToken {
            denom: denom.clone(),
            decimals,
            sending_precision: 5,
            max_holding_amount: Uint128::new(100000000000000000000),
            bridging_fee: Uint128::zero(),
        },
        &[],
    )
    .unwrap();
    // Get the token information
    let query_cosmos_tokens: CosmosTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::CosmosTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let cosmos_originated_token = query_cosmos_tokens
        .tokens
        .iter()
        .find(|t| t.denom == denom)
        .unwrap();

    let xrpl_denom_key =
        build_xrpl_token_key(&bridge_xrpl_address, &cosmos_originated_token.xrpl_currency);

    // init rate limit (1000000 per side)
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::AddRateLimit {
            xrpl_denom: xrpl_denom_key.clone(),
            quotas: vec![QuotaMsg {
                name: "daily".to_string(),
                duration: 86400,
                max_send: Uint128::new(1500000),
                max_receive: Uint128::new(1500000),
            }],
        },
        &[],
    )
    .unwrap();

    // It should truncate 1 because sending precision is 5
    let amount_to_send = Uint128::new(1000001);

    // Try to bridge the token to the xrpl receiver address so that we can send it back.
    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(amount_to_send.u128(), denom.clone()),
    )
    .unwrap();

    // Check balance of sender and contract
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom.clone())
        .unwrap();

    assert_eq!(request_balance, initial_amount - amount_to_send);

    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    assert_eq!(request_balance, amount_to_send);

    // query rate limits
    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: xrpl_denom_key.clone(),
            },
        )
        .unwrap();

    assert_eq!(rate_limits[0].flow.inflow, Uint128::zero());
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(1000000));
    // Confirm the operation to remove it from pending operations.
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let amount_truncated_and_converted = Uint128::new(1000000000000000); // 100001 -> truncate -> 100000 -> convert -> 1e15
    assert_eq!(query_pending_operations.operations.len(), 1);
    assert_eq!(
        query_pending_operations.operations[0].operation_type,
        OperationType::CosmosToXRPLTransfer {
            issuer: bridge_xrpl_address.clone(),
            currency: cosmos_originated_token.xrpl_currency.clone(),
            amount: amount_truncated_and_converted,
            max_amount: Some(amount_truncated_and_converted),
            sender: Addr::unchecked(sender),
            recipient: xrpl_receiver_address.clone(),
        }
    );

    let tx_hash = generate_hash();
    // Reject the operation, therefore the tokens should be stored in the pending refunds (except for truncated amount).
    app.execute(
        Addr::unchecked(relayer_account),
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

    // Truncated amount and amount to be refunded will stay in the contract until relayers and users to be refunded claim
    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();
    assert_eq!(request_balance, amount_to_send);

    // If we try to query pending refunds for any address that has no pending refunds, it should return an empty array
    let query_pending_refunds: PendingRefundsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingRefunds {
                address: Addr::unchecked("any_address"),
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_refunds.pending_refunds, vec![]);

    // Let's verify the pending refunds and try to claim them
    let query_pending_refunds: PendingRefundsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingRefunds {
                address: Addr::unchecked(sender),
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
    assert_eq!(
        query_pending_refunds.pending_refunds[0].xrpl_tx_hash,
        Some(tx_hash)
    );
    // Truncated amount (1) is not refundable
    assert_eq!(
        query_pending_refunds.pending_refunds[0].coin,
        coin(amount_to_send.u128() - 1u128, denom.clone())
    );

    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: xrpl_denom_key.clone(),
            },
        )
        .unwrap();

    assert_eq!(rate_limits[0].flow.inflow, Uint128::zero());
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(0));

    // Let's claim our pending refund
    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::ClaimRefund {
            pending_refund_id: query_pending_refunds.pending_refunds[0].id.clone(),
        },
        &[],
    )
    .unwrap();

    // Verify balance of sender (to check it was correctly refunded) and verify that the amount refunded was removed from pending refunds
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom.clone())
        .unwrap();

    assert_eq!(
        request_balance,
        initial_amount - Uint128::one() // truncated amount
    );

    let query_pending_refunds: PendingRefundsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingRefunds {
                address: Addr::unchecked(sender),
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    // We verify our pending refund operation was removed from the pending refunds
    assert!(query_pending_refunds.pending_refunds.is_empty());

    // Try to send again
    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(amount_to_send.u128(), denom.clone()),
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

    // Send successfull evidence to remove from queue (tokens should be released on XRPL to the receiver)
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: query_pending_operations.operations[0].account_sequence,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: xrpl_denom_key.clone(),
            },
        )
        .unwrap();
    assert_eq!(rate_limits[0].flow.inflow, Uint128::zero());
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(1000000));

    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(query_pending_operations.operations.len(), 0);

    // Try to send again, failed because got rate limit
    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(amount_to_send.u128(), denom.clone()),
    )
    .unwrap_err();

    // Test sending the amount back from XRPL to Cosmos
    // 10000000000 (1e10) is the minimum we can send back (15 - 5 (sending precision))
    let amount_to_send_back = Uint128::new(1000000000000000);

    // Sending the right evidence should move tokens from the contract to the sender's account
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: bridge_xrpl_address.clone(),
                currency: cosmos_originated_token.xrpl_currency.clone(),
                amount: amount_to_send_back.clone(),
                recipient: Addr::unchecked(sender),
                memo: None,
            },
        },
        &[],
    )
    .unwrap();

    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: xrpl_denom_key.clone(),
            },
        )
        .unwrap();
    assert_eq!(rate_limits[0].flow.inflow, Uint128::new(1000000));
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(1000000));

    // Check balance of sender and contract
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom.clone())
        .unwrap();

    assert_eq!(
        request_balance,
        initial_amount
            .checked_sub(amount_to_send) // initial amount
            .unwrap()
            .checked_sub(Uint128::one()) // amount lost during truncation of first rejection
            .unwrap()
            .checked_add(Uint128::new(1000000)) // Amount that we sent back (10) after conversion, the minimum
            .unwrap()
    );

    let request_balance = app
        .query_balance(contract_addr.clone(), denom.clone())
        .unwrap();

    assert_eq!(
        request_balance,
        amount_to_send
            .checked_add(Uint128::one()) // Truncated amount staying in contract
            .unwrap()
            .checked_sub(Uint128::new(1000000))
            .unwrap()
    );

    // currently, we can send to xrpl
    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(amount_to_send.u128(), denom.clone()),
    )
    .unwrap();
}

#[test]
fn send_from_cosmos_to_xrp_with_rate_limit() {
    let (mut app, accounts) = MockApp::new(&[
        ("account0", &coins(100_000_000_000, FEE_DENOM)),
        ("account1", &coins(100_000_000_000, FEE_DENOM)),
        ("account2", &coins(100_000_000_000, FEE_DENOM)),
    ]);

    let signer = &accounts[0];
    let sender = &accounts[1];
    let relayer_account = &accounts[2];
    let relayer = Relayer {
        cosmos_address: Addr::unchecked(relayer_account),
        xrpl_address: generate_xrpl_address(),
        xrpl_pub_key: generate_xrpl_pub_key(),
    };

    let xrpl_base_fee = 10;
    let multisig_address = generate_xrpl_address();

    let token_factory_addr = app.create_tokenfactory(Addr::unchecked(signer)).unwrap();
    let rate_limit_addr = app
        .create_rate_limit_contract(Addr::unchecked(signer), &RateLimitInitMsg { paths: vec![] })
        .unwrap();

    let contract_addr = app
        .create_bridge(
            Addr::unchecked(signer),
            &InstantiateMsg {
                owner: Addr::unchecked(signer),
                relayers: vec![relayer.clone()],
                evidence_threshold: 1,
                used_ticket_sequence_threshold: 10,
                trust_set_limit_amount: Uint128::new(TRUST_SET_LIMIT_AMOUNT),
                bridge_xrpl_address: multisig_address.clone(),
                xrpl_base_fee,
                token_factory_addr: token_factory_addr.clone(),
                issue_token: true,
                rate_limit_addr: Some(rate_limit_addr.clone()),
                osor_entry_point: None,
            },
        )
        .unwrap();

    let config: Config = app
        .query(contract_addr.clone(), &QueryMsg::Config {})
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

    let denom_xrp = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == XRP_ISSUER && t.currency == XRP_CURRENCY)
        .unwrap()
        .cosmos_denom
        .clone();

    let xrp_key = build_xrpl_token_key(XRP_ISSUER, XRP_CURRENCY);
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::AddRateLimit {
            xrpl_denom: xrp_key.clone(),
            quotas: vec![QuotaMsg {
                name: "daily".to_string(),
                duration: 86400,
                max_send: Uint128::new(1500000),
                max_receive: Uint128::new(1500000),
            }],
        },
        &[],
    )
    .unwrap();

    // Add enough tickets for all our test operations
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::RecoverTickets {
            account_sequence: 1,
            number_of_tickets: Some(11),
        },
        &[],
    )
    .unwrap();

    let tx_hash = generate_hash();
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(tx_hash.clone()),
                account_sequence: Some(1),
                ticket_sequence: None,
                transaction_result: TransactionResult::Accepted,
                operation_result: Some(OperationResult::TicketsAllocation {
                    tickets: Some((1..12).collect()),
                }),
            },
        },
        &[],
    )
    .unwrap();

    // If we query processed Txes with this tx_hash it should return true
    let query_processed_tx: bool = app
        .query(
            contract_addr.clone(),
            &QueryMsg::ProcessedTx {
                hash: tx_hash.to_uppercase(),
            },
        )
        .unwrap();

    assert_eq!(query_processed_tx, true);

    // If we query something that is not processed it should return false
    let query_processed_tx: bool = app
        .query(
            contract_addr.clone(),
            &QueryMsg::ProcessedTx {
                hash: generate_hash(),
            },
        )
        .unwrap();

    assert_eq!(query_processed_tx, false);

    // *** Test sending XRP back to XRPL, which is already enabled so we can bridge it directly ***

    let amount_to_send_xrp = Uint128::new(50000);
    let amount_to_send_back = Uint128::new(10000);
    let final_balance_xrp = amount_to_send_xrp.checked_sub(amount_to_send_back).unwrap();
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                amount: amount_to_send_xrp.clone(),
                recipient: Addr::unchecked(sender),
                memo: None,
            },
        },
        &[],
    )
    .unwrap();

    // Check that balance is in the sender's account
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom_xrp.clone())
        .unwrap();

    assert_eq!(request_balance, amount_to_send_xrp);

    let xrpl_receiver_address = generate_xrpl_address();
    // Send the XRP back to XRPL successfully
    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(amount_to_send_back.u128(), denom_xrp.clone()),
    )
    .unwrap();

    // Check that operation is in the queue
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
            operation_type: OperationType::CosmosToXRPLTransfer {
                issuer: XRP_ISSUER.to_string(),
                currency: XRP_CURRENCY.to_string(),
                amount: amount_to_send_back,
                max_amount: None,
                sender: Addr::unchecked(sender),
                recipient: xrpl_receiver_address.clone(),
            },
            xrpl_base_fee,
        }
    );

    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: xrp_key.clone(),
            },
        )
        .unwrap();

    assert_eq!(rate_limits[0].flow.inflow, Uint128::new(50000));
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(10000));

    // Send successful evidence to remove from queue (tokens should be released on XRPL to the receiver)
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: Some(1),
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
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

    assert_eq!(query_pending_operations.operations.len(), 0);

    // Since transaction result was Accepted, the tokens must have been burnt
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom_xrp.clone())
        .unwrap();

    assert_eq!(request_balance, final_balance_xrp);

    let request_balance = app
        .query_balance(contract_addr.clone(), denom_xrp.clone())
        .unwrap();
    assert_eq!(request_balance, Uint128::zero());

    // Now we will try to send back again but this time reject it, thus balance must be sent back to the sender.
    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(amount_to_send_back.u128(), denom_xrp.clone()),
    )
    .unwrap();

    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: xrp_key.clone(),
            },
        )
        .unwrap();

    assert_eq!(rate_limits[0].flow.inflow, Uint128::new(50000));
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(20000));

    // Transaction was rejected
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: Some(2),
                transaction_result: TransactionResult::Rejected,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    let rate_limits: Vec<RateLimit> = app
        .query(
            rate_limit_addr.clone(),
            &RateLimitQueryMsg::GetQuotas {
                contract: contract_addr.clone(),
                channel_id: CHANNEL.to_string(),
                denom: xrp_key.clone(),
            },
        )
        .unwrap();

    assert_eq!(rate_limits[0].flow.inflow, Uint128::new(50000));
    assert_eq!(rate_limits[0].flow.outflow, Uint128::new(10000));
    // Since transaction result was Rejected, the tokens must have been sent to pending refunds

    let request_balance = app
        .query_balance(contract_addr.clone(), denom_xrp.clone())
        .unwrap();
    assert_eq!(request_balance, amount_to_send_back);

    let query_pending_refunds: PendingRefundsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingRefunds {
                address: Addr::unchecked(sender),
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    // We verify that these tokens are refundable
    assert_eq!(query_pending_refunds.pending_refunds.len(), 1);
    assert_eq!(
        query_pending_refunds.pending_refunds[0].coin,
        coin(amount_to_send_back.u128(), denom_xrp.clone())
    );

    // *** Test sending an XRPL originated token back to XRPL ***

    let issuer = generate_xrpl_address();
    let currency = "TST".to_string();
    let sending_precision = 15;
    let max_holding_amount = Uint128::new(50000000000000000000); // 5e20
    let bridging_fee = Uint128::zero();

    let xrp_key = build_xrpl_token_key(&issuer, &currency);
    app.execute(
        Addr::unchecked(signer),
        contract_addr.clone(),
        &ExecuteMsg::AddRateLimit {
            xrpl_denom: xrp_key.clone(),
            quotas: vec![QuotaMsg {
                name: "daily".to_string(),
                duration: 86400,
                max_send: Uint128::new(15000000000000000000),
                max_receive: Uint128::new(15000000000000000000),
            }],
        },
        &[],
    )
    .unwrap();

    // First we need to register and activate it
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

    // Activate the token
    let query_pending_operations: PendingOperationsResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::PendingOperations {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let tx_hash = generate_hash();
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(tx_hash.clone()),
                account_sequence: None,
                ticket_sequence: query_pending_operations.operations[0].ticket_sequence,
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
    )
    .unwrap();

    let amount_to_send = Uint128::new(10000000000000000000); // 1e20
    let amount_to_send_back = Uint128::new(6000000000000000000);
    let final_balance = amount_to_send.checked_sub(amount_to_send_back).unwrap();
    // Bridge some tokens to the sender address
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: issuer.to_string(),
                currency: currency.to_string(),
                amount: amount_to_send.clone(),
                recipient: Addr::unchecked(sender),
                memo: None,
            },
        },
        &[],
    )
    .unwrap();

    // bridge one more time, error because rate limited
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: issuer.to_string(),
                currency: currency.to_string(),
                amount: amount_to_send.clone(),
                recipient: Addr::unchecked(sender),
                memo: None,
            },
        },
        &[],
    )
    .unwrap_err();

    let query_xrpl_tokens: XRPLTokensResponse = app
        .query(
            contract_addr.clone(),
            &QueryMsg::XRPLTokens {
                start_after_key: None,
                limit: None,
            },
        )
        .unwrap();

    let xrpl_originated_token = query_xrpl_tokens
        .tokens
        .iter()
        .find(|t| t.issuer == issuer && t.currency == currency)
        .unwrap();
    let denom_xrpl_origin_token = xrpl_originated_token.cosmos_denom.clone();

    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom_xrpl_origin_token.clone())
        .unwrap();

    assert_eq!(request_balance, amount_to_send);

    // We will send a successful transfer to XRPL considering the token has no transfer rate

    app.execute(
        Addr::unchecked(sender),
        contract_addr.clone(),
        &ExecuteMsg::SendToXRPL {
            recipient: xrpl_receiver_address.clone(),
            deliver_amount: None,
        },
        &coins(amount_to_send_back.u128(), denom_xrpl_origin_token.clone()),
    )
    .unwrap();

    // Check that the operation was added to the queue

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
                issuer: xrpl_originated_token.issuer.clone(),
                currency: xrpl_originated_token.currency.clone(),
                amount: amount_to_send_back,
                max_amount: Some(amount_to_send_back),
                sender: Addr::unchecked(sender),
                recipient: xrpl_receiver_address.clone(),
            },
            xrpl_base_fee
        }
    );

    // Send successful should burn the tokens
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLTransactionResult {
                tx_hash: Some(generate_hash()),
                account_sequence: None,
                ticket_sequence: Some(4),
                transaction_result: TransactionResult::Accepted,
                operation_result: None,
            },
        },
        &[],
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

    assert_eq!(query_pending_operations.operations.len(), 0);

    // Tokens should have been burnt since transaction was accepted
    let request_balance = app
        .query_balance(Addr::unchecked(sender), denom_xrpl_origin_token.clone())
        .unwrap();
    assert_eq!(request_balance, final_balance);

    let request_balance = app
        .query_balance(contract_addr.clone(), denom_xrpl_origin_token.clone())
        .unwrap();

    assert_eq!(request_balance, Uint128::zero());

    // can bridge from xrpl to cosmos
    app.execute(
        Addr::unchecked(relayer_account),
        contract_addr.clone(),
        &ExecuteMsg::SaveEvidence {
            evidence: Evidence::XRPLToCosmosTransfer {
                tx_hash: generate_hash(),
                issuer: issuer.to_string(),
                currency: currency.to_string(),
                amount: amount_to_send.clone(),
                recipient: Addr::unchecked(sender),
                memo: None,
            },
        },
        &[],
    )
    .unwrap();
}
