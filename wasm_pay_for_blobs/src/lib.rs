use celestia_proto::celestia::blob::v1::MsgPayForBlobs;
use celestia_proto::cosmos::tx::v1beta1::TxBody;
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
