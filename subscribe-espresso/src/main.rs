use es_version::SequencerVersion;
use sequencer::SeqTypes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use tide_disco::error::ServerError;
type HotShotClient = surf_disco::Client<ServerError, SequencerVersion>;
use ark_serialize::CanonicalSerialize;
use async_std::stream::StreamExt;
use cid::Cid;
use hotshot_query_service::availability;
use hotshot_query_service::availability::BlockQueryData;
use hotshot_query_service::availability::VidCommonQueryData;
use hotshot_query_service::types::HeightIndexed;
use lambada::{
    executor::calculate_sha256, handle_tx, is_chain_info_same, setup_subscriber,
    trigger_callback_for_newblock, ExecutorOptions,
};
use sequencer::block::payload::{parse_ns_payload, NamespaceProof};
use sqlite::State;
use std::str::FromStr;
use surf_disco::Url;

#[async_std::main]
async fn main() {
    if let Some((subscribe_input, chain_cid)) = setup_subscriber("espresso") {
        subscribe_espresso(
            subscribe_input.height,
            subscribe_input.opt,
            &mut Cid::try_from(subscribe_input.current_cid).unwrap(),
            Arc::new(Mutex::new(Some(Cid::from_str(&chain_cid).unwrap()))),
            subscribe_input.chain_vm_id,
            subscribe_input.genesis_cid_text,
            subscribe_input.network_type,
        )
        .await;
    }
}
async fn subscribe_espresso(
    current_height: u64,
    opt: ExecutorOptions,
    current_cid: &mut Cid,
    current_chain_info_cid: Arc<Mutex<Option<Cid>>>,
    chain_vm_id: String,
    genesis_cid_text: String,
    network_type: String,
) {
    let sequencer_map = serde_json::from_str::<serde_json::Value>(&opt.sequencer_map)
        .expect("error getting sequencer url from sequencer map");
    let espresso_client_endpoint = sequencer_map
        .get("espresso")
        .unwrap()
        .get(network_type)
        .unwrap()
        .get("endpoint")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let query_service_url = Url::parse(&espresso_client_endpoint)
        .unwrap()
        .join("v0/availability")
        .unwrap();

    let hotshot = HotShotClient::new(query_service_url);
    hotshot.connect(None).await;

    let mut block_query_stream = hotshot
        .socket(&format!("stream/blocks/{}", current_height))
        .subscribe::<availability::BlockQueryData<SeqTypes>>()
        .await
        .expect("Unable to subscribe to HotShot block stream");
    let mut vid_common = hotshot
        .socket(&format!("stream/vid/common/{}", current_height))
        .subscribe::<VidCommonQueryData<SeqTypes>>()
        .await
        .unwrap();
    let mut chain = block_query_stream.zip(vid_common).enumerate();
    while let Some((i, (block, common))) = chain.next().await {
        let block = block.unwrap();
        let common = common.unwrap();
        let chain_info_cid = Arc::clone(&current_chain_info_cid);

        if !is_chain_info_same(opt.clone(), *current_cid, chain_info_cid.clone()).await {
            tracing::info!("is_chain_info_same = false {:?}", chain_info_cid);
            return;
        }

        let block: BlockQueryData<SeqTypes> = block;

        let payload = block.payload();

        let block_timestamp: u64 = block.header().timestamp;
        let espresso_block_timestamp = block_timestamp.to_be_bytes().to_vec();

        let espresso_tx_namespace = chain_vm_id.to_string();
        let mut espresso_tx_number: u64 = 0;
        let vm_id: u64 = chain_vm_id.parse().expect("vm-id should be a valid u64");

        let mut metadata: HashMap<Vec<u8>, Vec<u8>> = HashMap::<Vec<u8>, Vec<u8>>::new();
        metadata.insert(
            calculate_sha256("sequencer".as_bytes()),
            calculate_sha256("espresso".as_bytes()),
        );
        metadata.insert(
            calculate_sha256("espresso-block-height".as_bytes()),
            block.height().to_be_bytes().to_vec(),
        );
        metadata.insert(
            calculate_sha256("espresso-block-timestamp".as_bytes()),
            espresso_block_timestamp,
        );

        let mut bytes = Vec::new();
        block.hash().serialize_uncompressed(&mut bytes).unwrap();
        metadata.insert(
            calculate_sha256("espresso-block-hash".as_bytes()),
            bytes.clone(),
        );
        if let Some(info) = block.header().l1_finalized {
            metadata.insert(
                calculate_sha256("espresso-l1-block-height".as_bytes()),
                info.number.to_be_bytes().to_vec(),
            );
            let mut block_timestamp = vec![0; 32];
            info.timestamp.to_big_endian(&mut block_timestamp);
            metadata.insert(
                calculate_sha256("espresso-l1-block-timestamp".as_bytes()),
                block_timestamp,
            );
            metadata.insert(
                calculate_sha256("espresso-l1-block-hash".as_bytes()),
                info.hash.as_bytes().to_vec(),
            );
        }

        let height = block.height();
        //tracing::info!("block.height() {:?}", height);

        let connection = sqlite::Connection::open_thread_safe(format!(
            "{}/chains/{}",
            opt.db_path, genesis_cid_text
        ))
        .unwrap();
        let mut statement = connection
            .prepare("SELECT * FROM blocks WHERE height=?")
            .unwrap();
        statement.bind((1, height as i64)).unwrap();

        if let Ok(state) = statement.next() {
            // We've not processed this block before, so let's process it (can we even end here since we set starting point?)
            if state == State::Done {
                drop(statement);
                drop(connection);

                let common = common.common();

                let proof = block.payload().namespace_with_proof(
                    block.payload().get_ns_table(),
                    vm_id.into(),
                    common.clone(),
                );

                if let Some(p) = proof {
                    match p {
                        NamespaceProof::Existence {
                            ns_payload_flat, ..
                        } => {
                            let transactions = parse_ns_payload(&ns_payload_flat, vm_id.into());
                            for (_, tx) in transactions.into_iter().enumerate() {
                                let mut tx_metadata = metadata.clone();
                                tx_metadata.insert(
                                    calculate_sha256("espresso-tx-number".as_bytes()),
                                    espresso_tx_number.to_be_bytes().to_vec(),
                                );
                                tx_metadata.insert(
                                    calculate_sha256("espresso-tx-namespace".as_bytes()),
                                    espresso_tx_namespace.clone().into(),
                                );

                                tracing::info!("new tx, call handle_tx");
                                handle_tx(
                                    opt.clone(),
                                    Some(tx.payload().to_vec()),
                                    current_cid,
                                    tx_metadata,
                                    height,
                                    genesis_cid_text.clone(),
                                    hex::encode(bytes.clone()),
                                )
                                .await;

                                espresso_tx_number += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let new_state_cid = current_cid.to_string();
            let new_block_height = height;

            let options_clone = Arc::new(opt.clone());
            let genesis_block_cid = genesis_cid_text.clone();

            thread::spawn(move || {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                runtime.block_on(async {
                    trigger_callback_for_newblock(
                        options_clone,
                        &genesis_block_cid,
                        new_block_height,
                        &new_state_cid,
                    )
                    .await;
                });
            });
        }
    }
}
