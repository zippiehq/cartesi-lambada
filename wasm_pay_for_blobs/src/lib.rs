use celestia_proto::celestia::blob::v1::MsgPayForBlobs;
use prost::Message;
use wasm_bindgen::prelude::wasm_bindgen;
use celestia_types::Commitment;
use celestia_types::consts::appconsts;
use celestia_types::nmt::Namespace;
#[wasm_bindgen]
pub fn pay_blobs(signer_address: &str, vm_id_namespace: Vec<u8>, data: Vec<u8>, share_version: u32) -> Vec<u8> {
    let commitment =
            Commitment::from_blob(Namespace::new_v0(&vm_id_namespace).unwrap(), appconsts::SHARE_VERSION_ZERO, &data[..]).expect("commitment receiving failed");
    let msg = MsgPayForBlobs{
        signer: signer_address.to_string(),
        namespaces: vec![vm_id_namespace.to_vec()],
        blob_sizes: vec![data.len() as u32],
        share_commitments: vec![commitment.0.to_vec()],
        share_versions: vec![share_version]
    };
    let mut buf = Vec::new();
    msg.encode(&mut buf).expect("encoding failed");
    buf
}