use std::collections::HashMap;

use cosmwasm_std::{to_json_string, to_json_vec, Addr, Uint128};

use super::helper::{generate_invalid_xrpl_address, generate_xrpl_address};
use crate::{
    address::validate_xrpl_address_format,
    contract::INITIAL_PROHIBITED_XRPL_ADDRESSES,
    evidence::{hash_bytes, Evidence, OperationResult, TransactionResult},
    tests::helper::generate_hash,
};

#[test]
fn validate_xrpl_addresses() {
    let mut valid_addresses = vec![
        "rU6K7V3Po4snVhBBaU29sesqs2qTQJWDw1".to_string(),
        "rLUEXYuLiQptky37CqLcm9USQpPiz5rkpD".to_string(),
        "rBTwLga3i2gz3doX6Gva3MgEV8ZCD8jjah".to_string(),
        "rDxMt25DoKeNv7te7WmLvWwsmMyPVBctUW".to_string(),
        "rPbPkTSrAqANkoTFpwheTxRyT8EQ38U5ok".to_string(),
        "rQ3fNyLjbvcDaPNS4EAJY8aT9zR3uGk17c".to_string(),
        "rnATJKpFCsFGfEvMC3uVWHvCEJrh5QMuYE".to_string(),
        generate_xrpl_address(),
        generate_xrpl_address(),
        generate_xrpl_address(),
        generate_xrpl_address(),
    ];

    // Add the current prohibited address and check that they are valid generated xrpl addresses
    for prohibited_address in INITIAL_PROHIBITED_XRPL_ADDRESSES {
        valid_addresses.push(prohibited_address.to_string());
    }

    for address in valid_addresses.iter() {
        validate_xrpl_address_format(address).unwrap();
    }

    let mut invalid_addresses: Vec<String> = vec![
        "zDTXLQ7ZKZVKz33zJbHjgVShjsBnqMBhmN".to_string(), // Invalid prefix
        "rf1BiGeXwwQoi8Z2u".to_string(),                  // Too short
        "rU6K7V3Po4snVhBBaU29sesqs2qTQJWDw1hBBaU29".to_string(), // Too long
        "rU6K7V3Po4snVhBBa029sesqs2qTQJWDw1".to_string(), // Contains invalid character 0
        "rU6K7V3Po4snVhBBaU29sesql2qTQJWDw1".to_string(), // Contains invalid character l
        "rLUEXYuLiQptky37OqLcm9USQpPiz5rkpD".to_string(), // Contains invalid character O
        "rLUEXYuLiQpIky37CqLcm9USQpPiz5rkpD".to_string(), // Contains invalid character I
    ];

    for _ in 0..100 {
        invalid_addresses.push(generate_invalid_xrpl_address()); // Just random address without checksum calculation
    }

    for address in invalid_addresses.iter() {
        validate_xrpl_address_format(address).unwrap_err();
    }
}

#[test]
fn enum_hashes() {
    let hash = generate_hash();
    let issuer = "issuer".to_string();
    let currency = "currency".to_string();
    let amount = Uint128::new(100);
    let recipient = Addr::unchecked("signer");

    // Create multiple evidences changing only 1 field to verify that all of them have different hashes
    let xrpl_to_cosmos_transfer_evidences = vec![
        Evidence::XRPLToCosmosTransfer {
            tx_hash: hash.clone(),
            issuer: issuer.clone(),
            currency: currency.clone(),
            amount: amount.clone(),
            recipient: recipient.clone(),
            memo: None,
        },
        Evidence::XRPLToCosmosTransfer {
            tx_hash: generate_hash(),
            issuer: issuer.clone(),
            currency: currency.clone(),
            amount: amount.clone(),
            recipient: recipient.clone(),
            memo: None,
        },
        Evidence::XRPLToCosmosTransfer {
            tx_hash: hash.clone(),
            issuer: "new_issuer".to_string(),
            currency: currency.clone(),
            amount: amount.clone(),
            recipient: recipient.clone(),
            memo: None,
        },
        Evidence::XRPLToCosmosTransfer {
            tx_hash: hash.clone(),
            issuer: issuer.clone(),
            currency: "new_currency".to_string(),
            amount: amount.clone(),
            recipient: recipient.clone(),
            memo: None,
        },
        Evidence::XRPLToCosmosTransfer {
            tx_hash: hash.clone(),
            issuer: issuer.clone(),
            currency: currency.clone(),
            amount: Uint128::one(),
            recipient: recipient.clone(),
            memo: None,
        },
        Evidence::XRPLToCosmosTransfer {
            tx_hash: hash.clone(),
            issuer: issuer.clone(),
            currency: currency.clone(),
            amount: amount.clone(),
            recipient: Addr::unchecked("new_recipient"),
            memo: None,
        },
    ];

    // Add them all to a map to see that they create different entries
    let mut evidence_map = HashMap::new();
    for evidence in xrpl_to_cosmos_transfer_evidences.iter() {
        evidence_map.insert(
            hash_bytes(&to_json_string(evidence).unwrap().into_bytes()),
            true,
        );
    }

    assert_eq!(evidence_map.len(), xrpl_to_cosmos_transfer_evidences.len());

    let hash = Some(generate_hash());
    let operation_id = Some(1);
    let transaction_result = TransactionResult::Accepted;
    let operation_result = None;
    // Create multiple evidences changing only 1 field to verify that all of them have different hashes
    let xrpl_transaction_result_evidences = vec![
        Evidence::XRPLTransactionResult {
            tx_hash: hash.clone(),
            account_sequence: operation_id,
            ticket_sequence: None,
            transaction_result: transaction_result.clone(),
            operation_result: operation_result.clone(),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: Some(generate_hash()),
            account_sequence: operation_id,
            ticket_sequence: None,
            transaction_result: transaction_result.clone(),
            operation_result: operation_result.clone(),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: hash.clone(),
            account_sequence: Some(2),
            ticket_sequence: None,
            transaction_result: transaction_result.clone(),
            operation_result: operation_result.clone(),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: hash.clone(),
            account_sequence: None,
            ticket_sequence: operation_id,
            transaction_result: transaction_result.clone(),
            operation_result: operation_result.clone(),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: hash.clone(),
            account_sequence: None,
            ticket_sequence: Some(2),
            transaction_result: transaction_result.clone(),
            operation_result: operation_result.clone(),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: hash.clone(),
            account_sequence: operation_id,
            ticket_sequence: None,
            transaction_result: TransactionResult::Rejected,
            operation_result: operation_result.clone(),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: hash.clone(),
            account_sequence: operation_id,
            ticket_sequence: None,
            transaction_result: transaction_result.clone(),
            operation_result: Some(OperationResult::TicketsAllocation { tickets: None }),
        },
        Evidence::XRPLTransactionResult {
            tx_hash: hash.clone(),
            account_sequence: operation_id,
            ticket_sequence: None,
            transaction_result: transaction_result.clone(),
            operation_result: Some(OperationResult::TicketsAllocation {
                tickets: Some(vec![1, 2, 3]),
            }),
        },
    ];

    // Add them all to a map to see that they create different entries
    let mut evidence_map = HashMap::new();
    for evidence in xrpl_transaction_result_evidences.iter() {
        evidence_map.insert(hash_bytes(&to_json_vec(evidence).unwrap()), true);
    }

    assert_eq!(evidence_map.len(), xrpl_transaction_result_evidences.len());
}
