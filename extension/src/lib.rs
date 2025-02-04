use bech32::{FromBase32, ToBase32};
use chrono::{DateTime, Datelike, Timelike};
use pallas::crypto::hash::Hasher;
use pallas::ledger::addresses::Address;
use pallas::ledger::addresses::ByronAddress;
use pallas::ledger::addresses::StakeAddress;
use pallas::ledger::primitives::ToCanonicalJson;
use pallas::ledger::traverse::wellknown::*;
use pallas::ledger::traverse::MultiEraBlock;
use pallas::ledger::traverse::MultiEraOutput;
use pallas::ledger::traverse::MultiEraTx;
use pallas::ledger::traverse::MultiEraWithdrawals;
use pgrx::prelude::*;
use std::collections::HashMap;
use std::ops::Deref;

pgrx::pg_module_magic!();

#[pg_extern(immutable)]
fn hello_extension() -> &'static str {
    "Hello, extension"
}

#[pg_extern(immutable)]
fn block_tx_count(block_cbor: &[u8]) -> i32 {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return -1,
    };

    block.tx_count() as i32
}

#[pg_extern(immutable)]
fn block_era(block_cbor: &[u8]) -> i32 {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return -1,
    };

    let block_era_as_u16: u16 = block.era().into();
    block_era_as_u16.into()
}

#[pg_extern(immutable)]
fn block_txs_cbor(block_cbor: &[u8]) -> Vec<Vec<u8>> {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return vec![],
    };

    block.txs().into_iter().map(|tx| tx.encode()).collect()
}

#[pg_extern(immutable)]
fn block_number(block_cbor: &[u8]) -> i64 {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return -1,
    };

    block.number() as i64
}

#[pg_extern(immutable)]
fn block_slot(block_cbor: &[u8]) -> i64 {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return -1,
    };

    block.slot() as i64
}

#[pg_extern(immutable)]
fn block_pool_id(block_cbor: &[u8]) -> Vec<u8> {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return vec![],
    };
    match block.header().issuer_vkey() {
        Some(hash) => Hasher::<224>::hash(hash).to_vec(),
        None => vec![],
    }
}

#[pg_extern(immutable)]
fn block_has_pool_id(block_cbor: &[u8], pool_id: &[u8]) -> bool {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    match block.header().issuer_vkey() {
        Some(hash) => Hasher::<224>::hash(hash).to_vec() == pool_id,
        None => false,
    }
}

#[pg_extern(immutable)]
fn block_size(block_cbor: &[u8]) -> i64 {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return -1,
    };

    block.size() as i64
}

#[pg_extern(immutable)]
fn block_epoch(block_cbor: &[u8], network_id: i64) -> i64 {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return -1,
    };

    let genesis = match GenesisValues::from_magic(network_id as u64) {
        Some(x) => x,
        None => return -1,
    };

    block.epoch(&genesis).0 as i64
}

#[pg_extern(immutable)]
fn block_slot_as_time(block_cbor: &[u8], network_id: i64) -> pgrx::Timestamp {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return (-1).into(),
    };

    let genesis = match GenesisValues::from_magic(network_id as u64) {
        Some(x) => x,
        None => return (-1).into(),
    };

    let seconds = block.wallclock(&genesis) as i64;

    let naive_datetime = DateTime::from_timestamp(seconds, 0)
        .unwrap_or_else(|| DateTime::from_timestamp(0, 0).unwrap());

    let year = naive_datetime.year();
    let month = naive_datetime.month() as u8;
    let day = naive_datetime.day() as u8;
    let hour = naive_datetime.hour() as u8;
    let minute = naive_datetime.minute() as u8;
    let second = naive_datetime.second() as f64;

    Timestamp::new(year, month, day, hour, minute, second).unwrap()
}

#[pg_extern(immutable)]
fn block_is_epoch(block_cbor: &[u8], network_id: i64, epoch: i64) -> bool {
    let block = match MultiEraBlock::decode(block_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    let genesis = match GenesisValues::from_magic(network_id as u64) {
        Some(x) => x,
        None => return false,
    };

    block.epoch(&genesis).0 == epoch as u64
}

/// Returns the hash of the given transaction data.
///
/// # Arguments
///
/// * `tx_cbor` - The transaction data in CBOR format.
///
/// # Returns
///
/// The hash of the given transaction data as a string.
///
/// # Example
///
/// ```
/// select tx_hash(body) from transactions;
/// ```
#[pg_extern(immutable)]
fn tx_hash(tx_cbor: &[u8]) -> String {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return "".to_string(),
    };

    tx.hash().to_string()
}

#[pg_extern(immutable)]
fn tx_inputs(tx_cbor: &[u8]) -> Vec<Option<String>> {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return vec![],
    };

    tx.consumes()
        .iter()
        .map(|i| Some(format!("{}#{}", i.hash(), i.index())))
        .collect::<Vec<Option<String>>>()
}

#[allow(clippy::type_complexity)]
#[pg_extern(immutable)]
fn tx_outputs(
    tx_cbor: &[u8],
) -> TableIterator<
    'static,
    (
        name!(output_index, i32),
        name!(address, Option<String>),
        name!(lovelace, pgrx::AnyNumeric),
        name!(assets, pgrx::Json),
        name!(datum, pgrx::Json),
        name!(cbor, Vec<u8>),
    ),
> {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return TableIterator::new(std::iter::empty()),
    };

    let outputs_data = tx
        .produces()
        .iter()
        .map(|(i, o)| {
            (
                *i as i32,
                output_to_address_string(o),
                AnyNumeric::from(o.value().coin()),
                pgrx::Json(
                    serde_json::to_value(
                        o.value()
                            .assets()
                            .iter()
                            .map(|asset| {
                                let policy_id = hex::encode(asset.policy().as_ref());
                                let assets: HashMap<String, i128> = asset
                                    .assets()
                                    .iter()
                                    .map(|a| (hex::encode(a.name()), a.any_coin()))
                                    .collect();

                                (policy_id, assets)
                            })
                            .collect::<HashMap<_, _>>(),
                    )
                    .unwrap(),
                ),
                match o.datum() {
                    Some(d) => match d {
                        pallas::ledger::primitives::conway::PseudoDatumOption::Hash(h) => {
                            pgrx::Json(serde_json::json!(h))
                        }
                        pallas::ledger::primitives::conway::PseudoDatumOption::Data(d) => {
                            pgrx::Json(d.unwrap().deref().to_json())
                        }
                    },
                    None => pgrx::Json(serde_json::json!(null)),
                },
                o.encode(),
            )
        })
        .collect::<Vec<_>>();

    TableIterator::new(outputs_data)
}

#[pg_extern(immutable)]
fn tx_outputs_json(tx_cbor: &[u8]) -> pgrx::JsonB {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return pgrx::JsonB(serde_json::json!([])),
    };

    let outputs_data: Vec<serde_json::Value> = tx
        .produces()
        .iter()
        .map(|(i, o)| {
            serde_json::json!({
                "output_index": *i as i32,
                "address": output_to_address_string(o),
                "lovelace": o.value().coin().to_string(),
                "assets": o.value().assets()
                    .iter()
                    .map(|asset| {
                        let policy_id = hex::encode(asset.policy().as_ref());
                        let assets: HashMap<String, String> = asset
                            .assets()
                            .iter()
                            .map(|a| (hex::encode(a.name()), a.any_coin().to_string()))
                            .collect();

                        (policy_id, assets)
                    })
                    .collect::<HashMap<_, _>>(),
                "datum": match o.datum() {
                    Some(d) => match d {
                        pallas::ledger::primitives::conway::PseudoDatumOption::Hash(h) =>
                            serde_json::json!(h),
                        pallas::ledger::primitives::conway::PseudoDatumOption::Data(d) =>
                            d.unwrap().deref().to_json(),
                    },
                    None => serde_json::json!(null),
                },
            })
        })
        .collect();

    pgrx::JsonB(serde_json::json!(outputs_data))
}

#[pg_extern(immutable)]
fn tx_is_valid(tx_cbor: &[u8]) -> bool {
    match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x.is_valid(),
        Err(_) => false,
    }
}

#[pg_extern(immutable)]
fn tx_addresses(tx_cbor: &[u8]) -> Vec<Option<String>> {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return vec![],
    };

    let outputs_data = tx
        .outputs()
        .iter()
        .map(|o| output_to_address_string(o))
        .collect::<Vec<_>>();

    outputs_data
}

#[pg_extern(immutable)]
fn tx_plutus_data(tx_cbor: &[u8]) -> pgrx::JsonB {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return pgrx::JsonB(serde_json::json!([])),
    };

    let plutus_data: Vec<serde_json::Value> =
        tx.plutus_data().iter().map(|x| x.to_json()).collect();

    pgrx::JsonB(serde_json::json!(plutus_data))
}

#[pg_extern(immutable)]
fn tx_lovelace(tx_cbor: &[u8]) -> pgrx::AnyNumeric {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };

    AnyNumeric::from(tx.outputs().iter().map(|o| o.value().coin()).sum::<u64>())
}

#[pg_extern(immutable)]
fn tx_fee(tx_cbor: &[u8]) -> pgrx::AnyNumeric {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };
    let fee = match tx.fee() {
        Some(f) => f,
        None => return AnyNumeric::from(0),
    };
    AnyNumeric::from(fee)
}

#[pg_extern(immutable)]
fn tx_mint(tx_cbor: &[u8]) -> Option<pgrx::JsonB> {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return None,
    };

    let mints = tx.mints();

    let mint_data: HashMap<String, HashMap<String, String>> = mints
        .iter()
        .map(|m| {
            let policy_id = hex::encode(m.policy().as_ref());
            let assets: HashMap<String, String> = m
                .assets()
                .iter()
                .map(|a| (hex::encode(a.name()), a.any_coin().to_string()))
                .collect();

            (policy_id, assets)
        })
        .collect();

    if mint_data.is_empty() {
        return None;
    }

    Some(pgrx::JsonB(serde_json::json!(mint_data)))
}

#[pg_extern(immutable)]
fn tx_subject_amount_output(tx_cbor: &[u8], subject: &[u8]) -> pgrx::AnyNumeric {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };

    const POLICY_ID_LEN: usize = 28;
    let (policy_id, asset_name) = subject.split_at(POLICY_ID_LEN);

    let amount = tx
        .outputs()
        .iter()
        .filter(|o| {
            o.value().assets().to_vec().iter().any(|a| {
                a.policy().deref() == policy_id && a.assets().iter().any(|a| a.name() == asset_name)
            })
        })
        .map(|o| {
            o.value()
                .assets()
                .to_vec()
                .iter()
                .flat_map(|a| {
                    a.assets()
                        .iter()
                        .filter(|a| a.name() == asset_name)
                        .map(|a| a.any_coin())
                        .collect::<Vec<_>>()
                })
                .next()
                .unwrap_or(0)
        })
        .sum::<i128>();

    AnyNumeric::from(amount)
}

#[pg_extern(immutable)]
fn tx_subject_amount_mint(tx_cbor: &[u8], subject: &[u8]) -> pgrx::AnyNumeric {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };

    const POLICY_ID_LEN: usize = 28;
    let (policy_id, asset_name) = subject.split_at(POLICY_ID_LEN);

    let amount = tx
        .mints()
        .iter()
        .filter(|m| {
            m.assets()
                .iter()
                .any(|a| a.policy().deref() == policy_id && a.name() == asset_name)
        })
        .map(|m| {
            m.assets()
                .iter()
                .filter(|a| a.name() == asset_name)
                .map(|a| a.any_coin())
                .sum::<i128>()
        })
        .sum::<i128>();

    AnyNumeric::from(amount)
}

#[pg_extern(immutable)]
fn tx_withdrawals(
    tx_cbor: &[u8],
) -> TableIterator<
    'static,
    (
        name!(stake_address, Vec<u8>),
        name!(amount, pgrx::AnyNumeric),
    ),
> {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return TableIterator::new(std::iter::empty()),
    };

    let withdrawals_data = match tx.withdrawals() {
        MultiEraWithdrawals::AlonzoCompatible(w) => w
            .iter()
            .map(|(k, v)| (k.to_vec(), AnyNumeric::from(*v)))
            .collect::<Vec<_>>(),
        _ => vec![],
    };
    TableIterator::new(withdrawals_data)
}

#[pg_extern(immutable)]
fn tx_withdrawals_json(tx_cbor: &[u8]) -> pgrx::JsonB {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return pgrx::JsonB(serde_json::json!({})),
    };

    let withdrawals_data: HashMap<String, String> = match tx.withdrawals() {
        MultiEraWithdrawals::AlonzoCompatible(w) => w
            .iter()
            .map(|(k, v)| (hex::encode(k.to_vec()), v.to_string()))
            .collect(),
        _ => HashMap::new(),
    };

    pgrx::JsonB(serde_json::json!(withdrawals_data))
}

#[pg_extern(immutable)]
fn tx_hash_is(tx_cbor: &[u8], hash: &[u8]) -> bool {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    tx.hash().to_vec().eq(&hash)
}

#[pg_extern(immutable)]
fn tx_has_mint(tx_cbor: &[u8]) -> bool {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    !tx.mints().is_empty()
}

#[pg_extern(immutable)]
fn tx_has_address_output(tx_cbor: &[u8], address: &[u8]) -> bool {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    tx.outputs().iter().any(|o| {
        o.address()
            .ok()
            .map(|iter_address| iter_address.to_vec().eq(address))
            .unwrap_or(false)
    })
}

#[pg_extern(immutable)]
fn tx_has_policy_id_output(tx_cbor: &[u8], policy_id: &[u8]) -> bool {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    tx.outputs().iter().any(|o| {
        o.value()
            .assets()
            .to_vec()
            .iter()
            .any(|a| a.policy().deref().eq(&policy_id))
    })
}

#[pg_extern(immutable)]
fn tx_has_policy_id_mint(tx_cbor: &[u8], policy_id: &[u8]) -> bool {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    tx.mints().iter().any(|m| m.policy().deref().eq(&policy_id))
}

#[pg_extern(immutable)]
fn tx_has_subject_output(tx_cbor: &[u8], subject: &[u8]) -> bool {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    const POLICY_ID_LEN: usize = 28;
    let (policy_id, asset_name) = subject.split_at(POLICY_ID_LEN);

    tx.outputs().iter().any(|o| {
        o.value().assets().to_vec().iter().any(|a| {
            a.policy().deref() == policy_id && a.assets().iter().any(|a| a.name() == asset_name)
        })
    })
}

#[pg_extern(immutable)]
fn tx_has_mint_output(tx_cbor: &[u8], subject: &[u8]) -> bool {
    let tx = match MultiEraTx::decode(tx_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    const POLICY_ID_LEN: usize = 28;
    let (policy_id, asset_name) = subject.split_at(POLICY_ID_LEN);

    tx.mints().iter().any(|m| {
        m.assets()
            .iter()
            .any(|a| a.policy().deref() == policy_id && a.name() == asset_name)
    })
}

#[pg_extern(immutable)]
fn address_network_id(address: &[u8]) -> i64 {
    let address = match Address::from_bytes(address) {
        Ok(x) => x,
        Err(_) => return -1,
    };

    match address.network() {
        Some(n) => n.value() as i64,
        None => -1,
    }
}

#[pg_extern(immutable)]
fn address_payment_part(address: &[u8]) -> Vec<u8> {
    let address = match Address::from_bytes(address) {
        Ok(x) => x,
        Err(_) => return vec![],
    };

    let payment_part = match address {
        Address::Shelley(a) => a.payment().to_vec(),
        Address::Byron(_) => {
            vec![]
        }
        _ => return vec![],
    };

    payment_part
}

#[pg_extern(immutable)]
fn address_stake_part(address: &[u8]) -> Vec<u8> {
    let address = match Address::from_bytes(address) {
        Ok(x) => x,
        Err(_) => return vec![],
    };

    let stake_part = match address {
        Address::Shelley(a) => a.delegation().to_vec(),
        Address::Byron(_) => {
            vec![]
        }
        _ => return vec![],
    };

    stake_part
}

#[pg_extern(immutable)]
fn address_to_bytes(address: String) -> Vec<u8> {
    let address = match Address::from_bech32(&address) {
        Ok(x) => x,
        Err(_) => return vec![],
    };

    address.to_vec()
}

#[pg_extern(immutable)]
fn address_to_bech32(address_bytes: &[u8]) -> String {
    let address = match Address::from_bytes(address_bytes) {
        Ok(x) => x,
        Err(_) => return String::new(),
    };

    match address.to_bech32() {
        Ok(x) => x,
        // @TODO: this is not bech32 though?
        Err(_) => ByronAddress::from_bytes(address_bytes).unwrap().to_base58(),
    }
}

#[pg_extern(immutable)]
fn address_to_stake_part_bech32(address_bytes: &[u8]) -> String {
    let address = match Address::from_bytes(address_bytes) {
        Ok(addr) => addr,
        Err(_) => return String::new(),
    };

    match address {
        Address::Shelley(a) => StakeAddress::try_from(a)
            .map(|stake_addr| stake_addr.to_bech32().unwrap_or_else(|_| String::new()))
            .unwrap_or_else(|_| String::new()),
        Address::Byron(_) => String::new(),
        _ => String::new(),
    }
}

#[pg_extern(immutable)]
fn stake_part_to_bech32(stake_part_bytes: &[u8]) -> String {
    let stake_part = match Address::from_bytes(stake_part_bytes) {
        Ok(x) => x,
        Err(_) => return String::new(),
    };

    stake_part.to_bech32().unwrap_or_default()
}

#[pg_extern(immutable)]
fn utxo_address(era: i32, utxo_cbor: &[u8]) -> Option<Vec<u8>> {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return None,
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return Some(vec![]),
    };

    output.address().ok().map(|address| address.to_vec())
}

#[pg_extern(immutable)]
fn utxo_has_policy_id(era: i32, utxo_cbor: &[u8], policy_id: &[u8]) -> bool {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return false,
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    output
        .value()
        .assets()
        .to_vec()
        .iter()
        .any(|a| a.policy().deref().eq(&policy_id))
}

#[pg_extern(immutable)]
fn utxo_has_address(era: i32, utxo_cbor: &[u8], address: &[u8]) -> bool {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return false,
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return false,
    };

    output
        .address()
        .ok()
        .map(|iter_address| iter_address.to_vec().eq(&address))
        .unwrap_or_else(|| false)
}

#[pg_extern(immutable)]
fn utxo_lovelace(era: i32, utxo_cbor: &[u8]) -> pgrx::AnyNumeric {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };

    AnyNumeric::from(output.value().coin())
}

#[pg_extern(immutable)]
fn utxo_policy_id_asset_names(
    era: i32,
    utxo_cbor: &[u8],
    policy_id: &[u8],
) -> SetOfIterator<'static, Vec<u8>> {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return SetOfIterator::new(std::iter::empty()),
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return SetOfIterator::new(std::iter::empty()),
    };

    let asset_names = output
        .value()
        .assets()
        .to_vec()
        .iter()
        .filter(|a| a.policy().deref().eq(&policy_id))
        .flat_map(|a| {
            a.assets()
                .iter()
                .map(|a| a.name().to_vec())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    SetOfIterator::new(asset_names)
}

#[pg_extern(immutable)]
fn utxo_asset_values(
    era: i32,
    utxo_cbor: &[u8],
) -> TableIterator<
    'static,
    (
        name!(policy_id, Vec<u8>),
        name!(asset_name, Vec<u8>),
        name!(amount, pgrx::AnyNumeric),
    ),
> {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return TableIterator::new(std::iter::empty()),
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return TableIterator::new(std::iter::empty()),
    };

    let asset_values = output
        .value()
        .assets()
        .to_vec()
        .iter()
        .flat_map(|a| {
            a.assets()
                .iter()
                .map(|a| {
                    (
                        a.policy().to_vec(),
                        a.name().to_vec(),
                        AnyNumeric::from(a.any_coin()),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    TableIterator::new(asset_values)
}

#[pg_extern(immutable)]
fn utxo_policy_id_asset_values(
    era: i32,
    utxo_cbor: &[u8],
    policy_id: &[u8],
) -> TableIterator<'static, (name!(asset_name, Vec<u8>), name!(amount, pgrx::AnyNumeric))> {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return TableIterator::new(std::iter::empty()),
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return TableIterator::new(std::iter::empty()),
    };

    let asset_values = output
        .value()
        .assets()
        .to_vec()
        .iter()
        .filter(|a| a.policy().deref().eq(&policy_id))
        .flat_map(|a| {
            a.assets()
                .iter()
                .map(|a| (a.name().to_vec(), AnyNumeric::from(a.any_coin())))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    TableIterator::new(asset_values)
}

#[pg_extern(immutable)]
fn utxo_subject_amount(era: i32, utxo_cbor: &[u8], subject: &[u8]) -> pgrx::AnyNumeric {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };

    let output = match MultiEraOutput::decode(era_enum, utxo_cbor) {
        Ok(x) => x,
        Err(_) => return AnyNumeric::from(0),
    };

    const POLICY_ID_LEN: usize = 28;
    let (policy_id, asset_name) = subject.split_at(POLICY_ID_LEN);

    let amount = output
        .value()
        .assets()
        .iter()
        .filter(|a| a.policy().deref() == policy_id)
        .flat_map(|a| {
            a.assets()
                .iter()
                .filter(|a| a.name() == asset_name)
                .map(|a| a.any_coin())
                .collect::<Vec<_>>()
        })
        .next()
        .unwrap_or(0);

    AnyNumeric::from(amount)
}

#[pg_extern(immutable)]
fn utxo_plutus_data(era: i32, utxo_cbor: &[u8]) -> Option<pgrx::Json> {
    let era_enum = match pallas::ledger::traverse::Era::try_from(era as u16) {
        Ok(x) => x,
        Err(_) => return None,
    };

    let output = MultiEraOutput::decode(era_enum, utxo_cbor).ok()?;

    output.datum().and_then(|datum_option| match datum_option {
        pallas::ledger::primitives::conway::PseudoDatumOption::Hash(_) => None,
        pallas::ledger::primitives::conway::PseudoDatumOption::Data(d) => {
            Some(pgrx::Json(d.deref().to_json()))
        }
    })
}

#[pg_extern(immutable)]
fn to_bech32(hash: &[u8], hrp: &str) -> String {
    match bech32::encode(hrp, hash.to_base32(), bech32::Variant::Bech32) {
        Ok(x) => x,
        Err(_) => "".to_string(),
    }
}

#[pg_extern(immutable)]
fn from_bech32(bech32: &str) -> Vec<u8> {
    match bech32::decode(bech32) {
        Ok((_, data, _)) => Vec::from_base32(&data).unwrap(),
        Err(_) => vec![],
    }
}

fn output_to_address_string(output: &MultiEraOutput) -> Option<String> {
    match output.address() {
        Ok(addr) => {
            let addr_str = addr.to_string();
            // Truncate the address to 103 characters typical HEADER, PAYMENT PART and DELEGATION PART encoding in bech32
            if addr_str.len() > 103 {
                Some(addr_str[..103].to_string())
            } else {
                Some(addr_str)
            }
        }
        Err(_) => None,
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_hello_extension() {
        assert_eq!("Hello, extension", crate::hello_extension());
    }

    #[pg_test]
    fn test_tx_hash() {
        // Decoded transaction data for testing
        const TX_DATA_HEX: &str = "84ad009282582040e50ebf0ded25391f7dd13ad2d32a8eef5a2cc76cc0e95b8bb2330c482def2f0082582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520082582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520282582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520382582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520482582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520582582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520682582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520782582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520882582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520982582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520a82582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520b82582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520c82582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520d82582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520e82582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520f82582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822521082582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f282252110182a300581d71071bd7f4b5e059ea90e763467cf559167b21c82ef1cb5fe34fb7a9e501821a030a32c0a3581c1cc1aceaf5c7df55e270864a60600b9f52383fe418164574ffdeeed0a14010581cc0e5564cf5786031d9053f567ec78b8383a0f2bc01318e690e0503f4a14001581cf66d78b4a3cb3d37afa0ec36461e51ecbde00f26c8f0a68f94b69880a144695553441b00000201d16e7cf2028201d818479f0000000000ff82583901da299558c70a8970781806dca93d1801ba2f3b3894227a7b284786e49baba19195b7cb8b1c6febb192cc487b5e8b96d737baddb8bb09866f1b000000015053786b021a0007a272031a07138899075820d36a2619a672494604e11bb447cbcf5231e9f2ba25c2169177edc941bd50ad6c081a0713876d0b5820de92cfe211abe2b770d253ff364362e4281c96ce70c3048b104acb5fc172ea900d8182582040e50ebf0ded25391f7dd13ad2d32a8eef5a2cc76cc0e95b8bb2330c482def2f000e81581cda299558c70a8970781806dca93d1801ba2f3b3894227a7b284786e40f011082583901da299558c70a8970781806dca93d1801ba2f3b3894227a7b284786e49baba19195b7cb8b1c6febb192cc487b5e8b96d737baddb8bb09866f1b00000001502d541d111a002dc6c0128482582032536acbfa12b80a3c570b1dac7948187dfa66992460d11542f67ba357c0fd2c0082582083d52903a465b2cf0dbb0900c1d8a1e2dec10578075cd7484b869a205f2822520182582089f6715ff7affd8bdeff696f47d7a08bd899cc9c627483a8885f9fd3943286a100825820db7900797bf9c1235976b226d0cdbe1040d199555158bfe2bc042575f142da6100a30082825820f44ce6186d190f8776fd871d753df7ae503972e4793a2360a423d2f96021e60158400b18e4fcf4be17a531d3fd7a0320df6ba7acbff31b35118275d7ff1cb3de25523453c767af39ac1f3b435749c44af64ecbaf19ae1e23a7b4ab9c7939d653a90182582063179f731829d60aade12a1398c07b7a905cc38e7d9901850c9b186946f5ca3e58403b3932c709d9a355f8a0bb453d2722f39f82a16bb7669669f11698cacc825ce2a74b26aa31f3740a8a820829bfda3f6f3f4bbce1f045707d037df0085273a50004800591840001d87980821a001aaf3e1a315977a684000ad87980821a00011efa1a01c4794f840006d87980821a00011efa1a01c4794f840004d87980821a00011efa1a01c4794f840002d87980821a00011efa1a01c4794f840003d87980821a00011efa1a01c4794f840005d87980821a00011efa1a01c4794f840008d87980821a00011efa1a01c4794f840007d87980821a00011efa1a01c4794f840009d87980821a00011efa1a01c4794f84000ed87980821a00011efa1a01c4794f84000cd87980821a00011efa1a01c4794f84000bd87980821a00011efa1a01c4794f84000dd87980821a00011efa1a01c4794f840010d87980821a00011efa1a01c4794f84000fd87980821a00011efa1a01c4794f840011d87980821a00011efa1a01c4794ff5a0";
        // Expected hash result for the given transaction data
        const EXPECTED_HASH: &str =
            "691bb954d364ac5a2fe4bafc72b43a77edee54bd4237d748547426f14f304c96";

        let tx_cbor = hex::decode(TX_DATA_HEX).expect("Failed to decode hex string into bytes");

        assert_eq!(
            EXPECTED_HASH,
            crate::tx_hash(&tx_cbor).to_string(),
            "The hash of the provided transaction data did not match the expected value."
        );
    }

    #[pg_test]
    fn test_utxo_address() {
        let utxo_hex = "825839000ba2902f70b40716d84de3d9c01ddc19b514d18f9b6911319a72900d6ee29460029464593dd53cd1435025e2e5614f60be06104c54b472eb1a68022c9f";
        let utxo_address = utxo_address(7, &hex::decode(utxo_hex).unwrap());
        assert_eq!("000ba2902f70b40716d84de3d9c01ddc19b514d18f9b6911319a72900d6ee29460029464593dd53cd1435025e2e5614f60be06104c54b472eb", utxo_address.map(hex::encode).unwrap());
    }
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
