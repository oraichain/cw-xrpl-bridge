use std::collections::VecDeque;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Coin, Empty, Uint128};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, Map, MultiIndex, UniqueIndex};

use crate::{evidence::Evidences, operation::Operation, relayer::Relayer};

/// Top level storage key. Values must not conflict.
/// Each key is only one byte long to ensure we use the smallest possible storage keys.
#[repr(u8)]
pub enum TopKey {
    Config = b'1',
    TxEvidences = b'2',
    ProcessedTxs = b'3',
    CosmosTokens = b'4',
    XRPLTokens = b'5',
    UsedXRPLCurrenciesForCosmosTokens = b'6',
    AvailableTickets = b'7',
    UsedTickets = b'8',
    PendingOperations = b'9',
    PendingTicketUpdate = b'a',
    PendingRefunds = b'b',
    FeesCollected = b'c',
    FeeRemainders = b'd',
    PendingRotateKeys = b'e',
    ProhibitedXRPLAddresses = b'f',
}

impl TopKey {
    const fn as_str(&self) -> &str {
        let array_ref = unsafe { std::mem::transmute::<_, &[u8; 1]>(self) };
        match core::str::from_utf8(array_ref) {
            Ok(a) => a,
            Err(_) => panic!("Non-utf8 enum value found. Use a-z, A-Z and 0-9"),
        }
    }
}

#[cw_serde]
pub struct Config {
    pub relayers: Vec<Relayer>,
    pub evidence_threshold: u32,
    pub used_ticket_sequence_threshold: u32,
    pub trust_set_limit_amount: Uint128,
    pub bridge_xrpl_address: String,
    pub bridge_state: BridgeState,
    pub xrpl_base_fee: u64,
    pub token_factory_addr: Addr,
    pub rate_limit_addr: Option<Addr>,
    pub osor_entry_point: Option<Addr>,
}

#[cw_serde]
pub enum BridgeState {
    // Bridge is active and working
    Active,
    // Bridge is halted and no operations can be executed until it's reactivated by owner (if there are no pending rotate keys operation on going)
    Halted,
}

#[cw_serde]
pub struct XRPLToken {
    pub issuer: String,
    pub currency: String,
    pub cosmos_denom: String,
    pub sending_precision: i32,
    pub max_holding_amount: Uint128,
    pub state: TokenState,
    pub bridging_fee: Uint128,
}

#[cw_serde]
pub enum TokenState {
    // Enabled tokens are tokens that can be bridged
    Enabled,
    // Disabled tokens are tokens that can be bridged but have been disabled by the admin and they must be activated again to be bridged
    Disabled,
    // Processing are tokens that have a TrustSet operation pending to be completed. If this operation succeeds they will be Enabled, if it fails they will be Inactive
    Processing,
    // Inactive tokens are tokens that can't be bridged because the trust set registration failed so it must be triggered again.
    Inactive,
}

#[cw_serde]
pub struct CosmosToken {
    pub denom: String,
    pub decimals: u32,
    pub xrpl_currency: String,
    pub sending_precision: i32,
    pub max_holding_amount: Uint128,
    pub state: TokenState,
    pub bridging_fee: Uint128,
}

#[cw_serde]
pub struct PendingRefund {
    pub address: Addr,
    // We will use a unique id (block timestamp - operation_id) for users to claim their funds back per operation id
    pub id: String,
    // Transaction hash in XRPL that failed to be able to track it and find reason for failure
    // Optional because Invalid transactions don't have a transaction hash because they are never executed
    pub xrpl_tx_hash: Option<String>,
    pub coin: Coin,
}

pub const CONFIG: Item<Config> = Item::new(TopKey::Config.as_str());
// Tokens registered from XRPL side. These tokens are XRPL originated tokens - primary key is issuer+currency on XRPL
// XRPLTokens will have cosmos_denom as a secondary index so that we can get the XRPLToken corresponding to a cosmos_denom
pub struct XRPLTokensIndexes<'a> {
    pub cosmos_denom: UniqueIndex<'a, String, XRPLToken, String>,
}

impl<'a> IndexList<XRPLToken> for XRPLTokensIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<XRPLToken>> + '_> {
        let v: Vec<&dyn Index<XRPLToken>> = vec![&self.cosmos_denom];
        Box::new(v.into_iter())
    }
}

pub const XRPL_TOKENS: IndexedMap<String, XRPLToken, XRPLTokensIndexes> = IndexedMap::new(
    TopKey::XRPLTokens.as_str(),
    XRPLTokensIndexes {
        cosmos_denom: UniqueIndex::new(
            |xrpl_token| xrpl_token.cosmos_denom.clone(),
            "xrpl_token__cosmos_denom",
        ),
    },
);
// Tokens registered from Cosmos side. These tokens are cosmos originated tokens that are registered to be bridged - key is denom on Cosmos chain
// CosmosTokens will have xrpl_currency as a secondary index so that we can get the CosmosToken corresponding to a xrpl_currency
pub struct CosmosTokensIndexes<'a> {
    pub xrpl_currency: UniqueIndex<'a, String, CosmosToken, String>,
}

impl<'a> IndexList<CosmosToken> for CosmosTokensIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<CosmosToken>> + '_> {
        let v: Vec<&dyn Index<CosmosToken>> = vec![&self.xrpl_currency];
        Box::new(v.into_iter())
    }
}

pub const COSMOS_TOKENS: IndexedMap<String, CosmosToken, CosmosTokensIndexes> = IndexedMap::new(
    TopKey::CosmosTokens.as_str(),
    CosmosTokensIndexes {
        xrpl_currency: UniqueIndex::new(
            |cosmos_token| cosmos_token.xrpl_currency.clone(),
            "cosmos_token__xrpl_currency",
        ),
    },
);

// Evidences, when enough evidences are collected, the transaction hashes are stored in PROCESSED_TXS.
pub const TX_EVIDENCES: Map<String, Evidences> = Map::new(TopKey::TxEvidences.as_str());
// This will contain the transaction hashes of operations that have been executed (reached threshold) so that when the same hash is sent again they aren't executed again
pub const PROCESSED_TXS: Map<String, Empty> = Map::new(TopKey::ProcessedTxs.as_str());
// Current tickets available
pub const AVAILABLE_TICKETS: Item<VecDeque<u64>> = Item::new(TopKey::AvailableTickets.as_str());
// Counter we use to control the used tickets threshold.
// If we surpass this counter, we will trigger a new allocation operation.
// Every time we allocate new tickets (operation is accepted), we will substract the amount of new tickets allocated from this amount
pub const USED_TICKETS_COUNTER: Item<u32> = Item::new(TopKey::UsedTickets.as_str());
// Operations that are not accepted/rejected yet. When enough relayers send evidences confirming the correct execution or rejection of this operation,
// it will move to PROCESSED_TXS. Key is the ticket/sequence number
pub const PENDING_OPERATIONS: Map<u64, Operation> = Map::new(TopKey::PendingOperations.as_str());
// Flag to know if we are currently waiting for new_tickets to be allocated
pub const PENDING_TICKET_UPDATE: Item<bool> = Item::new(TopKey::PendingTicketUpdate.as_str());
// Flag to know if we are currently waiting for a rotate keys operation to be completed
pub const PENDING_ROTATE_KEYS: Item<bool> = Item::new(TopKey::PendingRotateKeys.as_str());
// Amounts for rejected/invalid transactions on XRPL for each Cosmos user that they can reclaim manually.
// Key is the tuple (user_address, pending_refund_id)
pub struct PendingRefundsIndexes<'a> {
    // One address can have multiple pending refunds
    pub address: MultiIndex<'a, Addr, PendingRefund, (Addr, String)>,
}

impl<'a> IndexList<PendingRefund> for PendingRefundsIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<PendingRefund>> + '_> {
        let v: Vec<&dyn Index<PendingRefund>> = vec![&self.address];
        Box::new(v.into_iter())
    }
}

pub const PENDING_REFUNDS: IndexedMap<(Addr, String), PendingRefund, PendingRefundsIndexes> =
    IndexedMap::new(
        TopKey::PendingRefunds.as_str(),
        PendingRefundsIndexes {
            address: MultiIndex::new(
                |_pk, p: &PendingRefund| p.address.clone(),
                TopKey::PendingRefunds.as_str(),
                "pending_refund__address",
            ),
        },
    );

// Fees collected that will be slowly accumulated here and relayers can individually claim them anytime
pub const FEES_COLLECTED: Map<Addr, Vec<Coin>> = Map::new(TopKey::FeesCollected.as_str());
// Fees Remainders in case that we have some small amounts left after dividing fees between our relayers we will keep them here until next time we collect fees and can add them to the new amount
// Key is Coin denom and value is Coin amount
pub const FEE_REMAINDERS: Map<String, Uint128> = Map::new(TopKey::FeeRemainders.as_str());
// XRPL addresses that have been marked as prohibited and can't be used for receiving funds, issuing tokens, or multisigning transactions
pub const PROHIBITED_XRPL_ADDRESSES: Map<String, Empty> =
    Map::new(TopKey::ProhibitedXRPLAddresses.as_str());

pub const TEMP_UNIVERSAL_SWAP: Item<TempUniversalSwap> = Item::new("temp_universal_swap");

#[cw_serde]
pub struct TempUniversalSwap {
    pub recovery_address: String,
    pub return_amount: Coin,
}

pub enum ContractActions {
    Instantiation,
    CreateCosmosToken,
    MintCosmosToken,
    RegisterCosmosToken,
    RegisterXRPLToken,
    RecoverTickets,
    RecoverXRPLTokenRegistration,
    SaveEvidence,
    SaveSignature,
    SendToXRPL,
    ClaimFees,
    UpdateXRPLToken,
    UpdateCosmosToken,
    UpdateXRPLBaseFee,
    UpdateProhibitedXRPLAddresses,
    ClaimRefunds,
    HaltBridge,
    ResumeBridge,
    RotateKeys,
    CancelPendingOperation,
    UpdateUsedTicketSequenceThreshold,
    AddRateLimit,
    RemoveRateLimit,
    ResetRateLimitQuota,
}

pub enum UserType {
    Owner,
    Relayer,
}

impl UserType {
    pub fn is_authorized(&self, action: &ContractActions) -> bool {
        match &action {
            ContractActions::Instantiation => true,
            ContractActions::RegisterCosmosToken => matches!(self, Self::Owner),
            ContractActions::RegisterXRPLToken => matches!(self, Self::Owner),
            ContractActions::SaveEvidence => matches!(self, Self::Relayer),
            ContractActions::RecoverTickets => matches!(self, Self::Owner),
            ContractActions::CreateCosmosToken => matches!(self, Self::Owner),
            ContractActions::MintCosmosToken => matches!(self, Self::Owner),
            ContractActions::RecoverXRPLTokenRegistration => matches!(self, Self::Owner),
            ContractActions::SaveSignature => matches!(self, Self::Relayer),
            ContractActions::SendToXRPL => true,
            ContractActions::ClaimFees => matches!(self, Self::Relayer),
            ContractActions::UpdateXRPLToken => matches!(self, Self::Owner),
            ContractActions::UpdateCosmosToken => matches!(self, Self::Owner),
            ContractActions::UpdateXRPLBaseFee => matches!(self, Self::Owner),
            ContractActions::UpdateProhibitedXRPLAddresses => matches!(self, Self::Owner),
            ContractActions::ClaimRefunds => true,
            ContractActions::HaltBridge => matches!(self, Self::Owner | Self::Relayer),
            ContractActions::ResumeBridge => matches!(self, Self::Owner),
            ContractActions::RotateKeys => matches!(self, Self::Owner),
            ContractActions::CancelPendingOperation => matches!(self, Self::Owner),
            ContractActions::UpdateUsedTicketSequenceThreshold => matches!(self, Self::Owner),
            ContractActions::AddRateLimit => matches!(self, Self::Owner),
            ContractActions::RemoveRateLimit => matches!(self, Self::Owner),
            ContractActions::ResetRateLimitQuota => matches!(self, Self::Owner),
        }
    }
}

impl ContractActions {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Instantiation => "bridge_instantiation",
            Self::CreateCosmosToken => "create_cosmos_token",
            Self::MintCosmosToken => "mint_cosmos_token",
            Self::RegisterCosmosToken => "register_cosmos_token",
            Self::RegisterXRPLToken => "register_xrpl_token",
            Self::RecoverTickets => "recover_tickets",
            Self::RecoverXRPLTokenRegistration => "recover_xrpl_token_registration",
            Self::SaveEvidence => "save_evidence",
            Self::SaveSignature => "save_signature",
            Self::SendToXRPL => "send_to_xrpl",
            Self::ClaimFees => "claim_fees",
            Self::ClaimRefunds => "claim_refunds",
            Self::UpdateXRPLToken => "update_xrpl_token",
            Self::UpdateCosmosToken => "update_cosmos_token",
            Self::UpdateXRPLBaseFee => "update_xrpl_base_fee",
            Self::UpdateProhibitedXRPLAddresses => "update_invalid_xrpl_addresses",
            Self::HaltBridge => "halt_bridge",
            Self::ResumeBridge => "resume_bridge",
            Self::RotateKeys => "rotate_keys",
            Self::CancelPendingOperation => "cancel_pending_operation",
            Self::UpdateUsedTicketSequenceThreshold => "update_used_ticket_sequence_threshold",
            Self::AddRateLimit => "add_rate_limit",
            Self::RemoveRateLimit => "remove_rate_limit",
            Self::ResetRateLimitQuota => "reset_rate_limit_quota",
        }
    }
}
