use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use celestia_proto::celestia::blob::v1::MsgPayForBlobs;
use celestia_proto::cosmos::base::v1beta1::Coin;
use celestia_proto::cosmos::tx::v1beta1::TxBody;
use celestia_proto::cosmos::tx::v1beta1::{
    mode_info::{Single, Sum},
    AuthInfo, Fee, ModeInfo, SignerInfo,
};
use celestia_tendermint_proto::v0_34::types::{Blob, BlobTx};
use celestia_types::consts::appconsts;
use celestia_types::nmt::Namespace;
use celestia_types::Commitment;
use prost::Message;
use prost_types::Any;
use wasm_bindgen::prelude::wasm_bindgen;
#[wasm_bindgen]
pub fn pay_blobs(
    signer_address: &str,
    vm_id_namespace: Vec<u8>,
    data: Vec<u8>,
    share_version: u32,
) -> Vec<u8> {
    let commitment = Commitment::from_blob(
        Namespace::from_raw(&vm_id_namespace).unwrap(),
        appconsts::SHARE_VERSION_ZERO,
        &data[..],
    )
    .expect("commitment receiving failed");
    let msg = MsgPayForBlobs {
        signer: signer_address.to_string(),
        namespaces: vec![vm_id_namespace],
        blob_sizes: vec![data.len() as u32],
        share_commitments: vec![commitment.0.to_vec()],
        share_versions: vec![share_version],
    };
    let mut buf = Vec::new();
    msg.encode(&mut buf).expect("encoding failed");
    buf
}

#[wasm_bindgen]
pub fn message_to_tx(value: Vec<u8>) -> Vec<u8> {
    let any_msg = Any {
        type_url: "/celestia.blob.v1.MsgPayForBlobs".to_string(),
        value: value,
    };

    let tx_body = TxBody {
        messages: vec![any_msg],
        ..Default::default()
    };

    let mut buf = Vec::new();
    tx_body.encode(&mut buf).expect("encoding failed");
    buf
}

pub fn encode_blob_tx(tx: Vec<u8>, encoded_blobs: Vec<Vec<u8>>, type_id: String) -> Vec<u8> {
    let blobs: Vec<Blob> = encoded_blobs
        .iter()
        .map(|encoded_blob| Blob::decode(encoded_blob as &[u8]).unwrap())
        .collect();
    let blob_tx = BlobTx { tx, blobs, type_id };
    let mut buf = Vec::new();
    blob_tx.encode(&mut buf).expect("encoding failed");
    buf
}

#[wasm_bindgen]
pub fn auth_info_encode(
    pub_key: &str,
    sequence: u64,
    coin_denom: String,
    coin_amount: String,
    fee_gas: u64,
    fee_payer: String,
    fee_granter: String,
) -> Vec<u8> {
    let pub_key = BASE64_STANDARD.decode(pub_key).unwrap();
    let pub_key = Any {
        type_url: "/cosmos.crypto.secp256k1.PubKey".to_string(),
        value: pub_key,
    };
    let mode_info = ModeInfo {
        sum: Some(Sum::Single(Single { mode: 1 })),
    };
    let signer_info = SignerInfo {
        public_key: Some(pub_key),
        mode_info: Some(mode_info),
        sequence,
    };
    let coin = Coin {
        amount: coin_amount,
        denom: coin_denom,
    };
    let fee = Fee {
        amount: vec![coin],
        gas_limit: fee_gas,
        payer: fee_payer,
        granter: fee_granter,
    };

    let auth_info = AuthInfo {
        signer_infos: vec![signer_info],
        fee: Some(fee),
        tip: None,
    };

    let mut buf = Vec::new();
    auth_info.encode(&mut buf).expect("encoding failed");
    buf
}
