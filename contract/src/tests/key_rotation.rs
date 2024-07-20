// #[test]
// fn key_rotation() {
//     let app = OraiTestApp::new();
//     let accounts_number = 4;
//     let accounts = app
//         .init_accounts(&coins(100_000_000_000, FEE_DENOM), accounts_number)
//         .unwrap();

//     let signer = accounts.get((accounts_number - 1) as usize).unwrap();
//     let xrpl_addresses: Vec<String> = (0..3).map(|_| generate_xrpl_address()).collect();
//     let xrpl_pub_keys: Vec<String> = (0..3).map(|_| generate_xrpl_pub_key()).collect();

//     let mut relayer_accounts = vec![];
//     let mut relayers = vec![];

//     for i in 0..accounts_number - 1 {
//         relayer_accounts.push(accounts.get(i as usize).unwrap());
//         relayers.push(Relayer {
//             cosmos_address: Addr::unchecked(accounts.get(i as usize).unwrap().address()),
//             xrpl_address: xrpl_addresses[i as usize].to_string(),
//             xrpl_pub_key: xrpl_pub_keys[i as usize].to_string(),
//         });
//     }

//

//     let xrpl_base_fee = 10;

//     let contract_addr = store_and_instantiate(
//         &wasm,
//         Addr::unchecked(signer),
//         Addr::unchecked(signer),
//         vec![
//             relayers[0].clone(),
//             relayers[1].clone(),
//             relayers[2].clone(),
//         ],
//         3,
//         4,
//         Uint128::new(TRUST_SET_LIMIT_AMOUNT),
//         query_issue_fee(&asset_ft),
//         generate_xrpl_address(),
//         xrpl_base_fee,
//     );

//     // Recover enough tickets for testing
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RecoverTickets {
//             account_sequence: 1,
//             number_of_tickets: Some(5),
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     let tx_hash = generate_hash();
//     for relayer in relayer_accounts.iter() {
//         app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(tx_hash.clone()),
//                     account_sequence: Some(1),
//                     ticket_sequence: None,
//                     transaction_result: TransactionResult::Accepted,
//                     operation_result: Some(OperationResult::TicketsAllocation {
//                         tickets: Some((1..6).collect()),
//                     }),
//                 },
//             },
//             &[],
//             relayer,
//         )
//         .unwrap();
//     }

//     // Let's send a random evidence from 1 relayer that will stay after key rotation to confirm that it will be cleared after key rotation confirmation
//     let tx_hash_old_evidence = generate_hash();
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToCosmosTransfer {
//                 tx_hash: tx_hash_old_evidence.clone(),
//                 issuer: XRP_ISSUER.to_string(),
//                 currency: XRP_CURRENCY.to_string(),
//                 amount: Uint128::one(),
//                 recipient: Addr::unchecked(signer),
//             },
//         },
//         &[],
//         Addr::unchecked(relayer_accounts[0]),
//     )
//     .unwrap();

//     // If we send it again it should by same relayer it should fail because it's duplicated
//     let error_duplicated_evidence = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToCosmosTransfer {
//                     tx_hash: tx_hash_old_evidence.clone(),
//                     issuer: XRP_ISSUER.to_string(),
//                     currency: XRP_CURRENCY.to_string(),
//                     amount: Uint128::one(),
//                     recipient: Addr::unchecked(signer),
//                 },
//             },
//             &[],
//             Addr::unchecked(relayer_accounts[0]),
//         )
//         .unwrap_err();

//     assert!(error_duplicated_evidence.to_string().contains(
//         ContractError::EvidenceAlreadyProvided {}
//             .to_string()
//             .as_str()
//     ));

//     // We are going to perform a key rotation, for that we are going to remove a malicious relayer
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RotateKeys {
//             new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
//             new_evidence_threshold: 2,
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // If we try to perform another key rotation, it should fail because we have one pending ongoing
//     let pending_rotation_error = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::RotateKeys {
//                 new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
//                 new_evidence_threshold: 2,
//             },
//             &[],
//             Addr::unchecked(signer),
//         )
//         .unwrap_err();

//     assert!(pending_rotation_error
//         .to_string()
//         .contains(ContractError::RotateKeysOngoing {}.to_string().as_str()));

//     // Let's confirm that a pending operation is created
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 1);
//     assert_eq!(
//         query_pending_operations.operations[0],
//         Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: Some(1),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::RotateKeys {
//                 new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
//                 new_evidence_threshold: 2
//             },
//             xrpl_base_fee,
//         }
//     );

//     // Any evidence we send now that is not a RotateKeys evidence should fail
//     let error_no_key_rotation_evidence = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToCosmosTransfer {
//                     tx_hash: tx_hash_old_evidence.clone(),
//                     issuer: XRP_ISSUER.to_string(),
//                     currency: XRP_CURRENCY.to_string(),
//                     amount: Uint128::one(),
//                     recipient: Addr::unchecked(signer),
//                 },
//             },
//             &[],
//             &Addr::unchecked(&relayer_accounts[1])
//         )
//         .unwrap_err();

//     assert!(error_no_key_rotation_evidence
//         .to_string()
//         .contains(ContractError::BridgeHalted {}.to_string().as_str()));

//     // We are going to confirm the RotateKeys as rejected and check that nothing is changed and bridge is still halted
//     let tx_hash = generate_hash();
//     for relayer in relayer_accounts.iter() {
//         app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(tx_hash.clone()),
//                     account_sequence: None,
//                     ticket_sequence: Some(1),
//                     transaction_result: TransactionResult::Rejected,
//                     operation_result: None,
//                 },
//             },
//             &[],
//             relayer,
//         )
//         .unwrap();
//     }

//     // Pending operation should have been removed
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert!(query_pending_operations.operations.is_empty());

//     // Check config and see that it's the same as before and bridge is still halted
//     let query_config = wasm
//         .query::<QueryMsg, Config>(contract_addr.clone(), &QueryMsg::Config {})
//         .unwrap();

//     assert_eq!(query_config.relayers, relayers);
//     assert_eq!(query_config.evidence_threshold, 3);
//     assert_eq!(query_config.bridge_state, BridgeState::Halted);

//     // Let's try to perform a key rotation again and check that it works
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::RotateKeys {
//             new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
//             new_evidence_threshold: 2,
//         },
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Let's confirm that a pending operation is created
//     let query_pending_operations = wasm
//         .query::<QueryMsg, PendingOperationsResponse>(
//             contract_addr.clone(),
//             &QueryMsg::PendingOperations {
//                 start_after_key: None,
//                 limit: None,
//             },
//         )
//         .unwrap();

//     assert_eq!(query_pending_operations.operations.len(), 1);
//     assert_eq!(
//         query_pending_operations.operations[0],
//         Operation {
//             id: query_pending_operations.operations[0].id.clone(),
//             version: 1,
//             ticket_sequence: Some(2),
//             account_sequence: None,
//             signatures: vec![],
//             operation_type: OperationType::RotateKeys {
//                 new_relayers: vec![relayers[0].clone(), relayers[1].clone()],
//                 new_evidence_threshold: 2
//             },
//             xrpl_base_fee,
//         }
//     );

//     // We are going to confirm the RotateKeys as accepted and check that config has been updated correctly
//     let tx_hash = generate_hash();
//     for relayer in relayer_accounts.iter() {
//         app.execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLTransactionResult {
//                     tx_hash: Some(tx_hash.clone()),
//                     account_sequence: None,
//                     ticket_sequence: Some(2),
//                     transaction_result: TransactionResult::Accepted,
//                     operation_result: None,
//                 },
//             },
//             &[],
//             relayer,
//         )
//         .unwrap();
//     }

//     let query_config = wasm
//         .query::<QueryMsg, Config>(contract_addr.clone(), &QueryMsg::Config {})
//         .unwrap();

//     assert_eq!(
//         query_config.relayers,
//         vec![relayers[0].clone(), relayers[1].clone()]
//     );
//     assert_eq!(query_config.evidence_threshold, 2);
//     assert_eq!(query_config.bridge_state, BridgeState::Halted);

//     // Owner can now resume the bridge
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::ResumeBridge {},
//         &[],
//         Addr::unchecked(signer),
//     )
//     .unwrap();

//     // Let's check that evidences have been cleared by sending again the old evidence and it succeeds
//     // If evidences were cleared, this message will succeed because the evidence is not stored
//     app.execute(
//         contract_addr.clone(),
//         &ExecuteMsg::SaveEvidence {
//             evidence: Evidence::XRPLToCosmosTransfer {
//                 tx_hash: tx_hash_old_evidence.clone(),
//                 issuer: XRP_ISSUER.to_string(),
//                 currency: XRP_CURRENCY.to_string(),
//                 amount: Uint128::one(),
//                 recipient: Addr::unchecked(signer),
//             },
//         },
//         &[],
//         Addr::unchecked(relayer_accounts[0]),
//     )
//     .unwrap();

//     // Finally, let's check that the old relayer can not send evidences anymore
//     let error_not_relayer = wasm
//         .execute(
//             contract_addr.clone(),
//             &ExecuteMsg::SaveEvidence {
//                 evidence: Evidence::XRPLToCosmosTransfer {
//                     tx_hash: tx_hash_old_evidence.clone(),
//                     issuer: XRP_ISSUER.to_string(),
//                     currency: XRP_CURRENCY.to_string(),
//                     amount: Uint128::one(),
//                     recipient: Addr::unchecked(signer),
//                 },
//             },
//             &[],
//             &Addr::unchecked(&relayer_accounts[2])
//         )
//         .unwrap_err();

//     assert!(error_not_relayer
//         .to_string()
//         .contains(ContractError::UnauthorizedSender {}.to_string().as_str()));
// }
