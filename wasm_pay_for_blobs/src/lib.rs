use celestia_proto::celestia::blob::v1::MsgPayForBlobs;
use celestia_proto::cosmos::tx::v1beta1::TxBody;
use prost::Message;
use wasm_bindgen::prelude::wasm_bindgen;
use celestia_types::Commitment;
use celestia_types::consts::appconsts;
use celestia_types::nmt::Namespace;
use prost_types::Any;
#[wasm_bindgen]
pub fn pay_blobs(signer_address: &str, vm_id_namespace: Vec<u8>, data: Vec<u8>, share_version: u32) -> Vec<u8> {
    let commitment =
            Commitment::from_blob(Namespace::from_raw(&vm_id_namespace).unwrap(), appconsts::SHARE_VERSION_ZERO, &data[..]).expect("commitment receiving failed");
    let msg = MsgPayForBlobs{
        signer: signer_address.to_string(),
        namespaces: vec![vm_id_namespace],
        blob_sizes: vec![data.len() as u32],
        share_commitments: vec![commitment.0.to_vec()],
        share_versions: vec![share_version]
    };
    let mut buf = Vec::new();
    msg.encode(&mut buf).expect("encoding failed");
    buf
}

#[wasm_bindgen]
pub fn message_to_tx(velue: Vec<u8>) -> Vec<u8>{
    let any_msg = Any {
        type_url: "/celestia.blob.v1.MsgPayForBlobs".to_string(),
        value: velue, 
    };

    let tx_body = TxBody {
        messages: vec![any_msg], 
        ..Default::default() 
    };

    let mut buf = Vec::new();
    tx_body.encode(&mut buf).expect("encoding failed");
    buf}