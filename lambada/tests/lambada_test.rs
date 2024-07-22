#[cfg(test)]
mod lambada_functions_test {
    use hyper::{Body, Request};
    use sequential_test::sequential;
    use std::thread;
    const APPCHAIN: &str = "bafybeicdhhtwmgpnt7jugvlv3xtp2u4w4mkunpmg6txkkkjhpvnt2buyqa";
    const COMPUTE_CID_EXPECTED: &str =
        "bafybeiayjruyctdk6thwhy67ke7hjgjtwsnaoas3ekj2gapqlwt63khxle";
    const SUBMIT_HASH_EXPECTED: &str = "TX~N0btRzGOg9TAcpYy0Wzckt8bYkZXUSgm1xEQYJRuFznO";

    fn subscribe_request_test(server_address: String) {
        tokio_test::block_on(async {
            let req: Request<Body> = Request::builder()
                .method("GET")
                .header("Content-Type", "application/octet-stream")
                .uri(format!("{}/subscribe/{}", server_address, APPCHAIN))
                .body(Body::empty())
                .unwrap();
            let client = hyper::Client::new();
            match client.request(req).await {
                Ok(result) => {
                    let response = serde_json::from_slice::<serde_json::Value>(
                        &hyper::body::to_bytes(result)
                            .await
                            .expect("/subscribe test failed with no response")
                            .to_vec(),
                    )
                    .expect("/subscribe test failed with no response");

                    assert_eq!(response.get("ok").unwrap().as_str().unwrap(), "true");
                    thread::sleep(std::time::Duration::from_millis(15000));
                    println!(
                        "/subscribe request was successfully handled: response - {:?} ",
                        response
                    );
                }
                Err(e) => {
                    panic!("/subscribe test failed with error: {:?}", e);
                }
            }
        });
    }

    fn latest_request_test(server_address: String) -> (u64, String) {
        let height_state_cid = tokio_test::block_on(async {
            let req = Request::builder()
                .method("GET")
                .header("Content-Type", "application/octet-stream")
                .uri(format!("{}/latest/{}", server_address, APPCHAIN))
                .body(Body::empty())
                .unwrap();
            let client = hyper::Client::new();
            match client.request(req).await {
                Ok(result) => {
                    let response = serde_json::from_slice::<serde_json::Value>(
                        &hyper::body::to_bytes(result)
                            .await
                            .expect("/latest test failed with no response")
                            .to_vec(),
                    )
                    .expect("/latest test failed with no response");
                    let height: u64 = match response.get("height") {
                        Some(serde_json::Value::Number(n)) => {
                            if let Some(height) = n.as_u64() {
                                height
                            } else {
                                panic!("/latest test failed with no response");
                            }
                        }
                        _ => {
                            panic!("/latest test failed with no response");
                        }
                    };

                    let state_cid = match response.get("state_cid") {
                        Some(serde_json::Value::String(cid)) => cid.clone(),
                        _ => {
                            panic!("/latest test failed with no response");
                        }
                    };
                    println!(
                        "/latest request was successfully handled: response - {:?} ",
                        response
                    );
                    return (height, state_cid);
                }
                Err(e) => {
                    panic!("/latest test failed with error: {:?}", e);
                }
            }
        });
        return height_state_cid;
    }

    fn block_request_test(server_address: String, block_height: u64) -> String {
        let state_cid = tokio_test::block_on(async {
            let req = Request::builder()
                .method("GET")
                .header("Content-Type", "application/octet-stream")
                .uri(format!(
                    "{}/block/{}/{}",
                    server_address, APPCHAIN, block_height
                ))
                .body(Body::from("transaction data"))
                .unwrap();
            let client = hyper::Client::new();
            match client.request(req).await {
                Ok(result) => {
                    let response = serde_json::from_slice::<serde_json::Value>(
                        &hyper::body::to_bytes(result)
                            .await
                            .expect("/block test failed with no response")
                            .to_vec(),
                    )
                    .expect("/block test failed with no response");
                    let state_cid = match response.get("state_cid") {
                        Some(serde_json::Value::String(cid)) => cid.clone(),
                        _ => {
                            panic!("/block test failed with no response");
                        }
                    };
                    println!(
                        "/block request was successfully handled: response - {:?} ",
                        response
                    );
                    return state_cid;
                }
                Err(e) => {
                    panic!("/block test failed with error: {:?}", e);
                }
            }
        });
        state_cid
    }
    #[test]
    #[sequential]
    fn compute_request_test() {
        tokio_test::block_on(async {
            let server_address: String = std::env::var("SERVER_ADDRESS").unwrap();
            let req = Request::builder()
                .method("POST")
                .header("Content-Type", "application/octet-stream")
                .uri(format!("{}/compute/{}", server_address, APPCHAIN))
                .body(Body::from("echo hello world"))
                .unwrap();
            let client = hyper::Client::new();
            match client.request(req).await {
                Ok(result) => {
                    let cid = serde_json::from_slice::<serde_json::Value>(
                        &hyper::body::to_bytes(result)
                            .await
                            .expect("/compute test failed with no response")
                            .to_vec(),
                    )
                    .expect("/compute test failed with no response");
                    assert_eq!(
                        cid.get("cid").unwrap().as_str().unwrap(),
                        "bafybeiincorrectcidexamplewrongcid"
                    );
                }
                Err(e) => {
                    panic!("/compute test failed with error: {:?}", e);
                }
            }
        });
    }

    #[test]
    #[sequential]
    fn subscribe_latest_block_requests_test() {
        let server_address: String = std::env::var("SERVER_ADDRESS").unwrap();
        subscribe_request_test(server_address.clone());
        let latest_request_test_response = latest_request_test(server_address.clone());
        let block_request_test =
            block_request_test(server_address.clone(), latest_request_test_response.0);
        assert_eq!(latest_request_test_response.1, block_request_test);
    }

    #[test]
    #[sequential]
    fn health_request_test() {
        tokio_test::block_on(async {
            let server_address: String = std::env::var("SERVER_ADDRESS").unwrap();

            let req = Request::builder()
                .method("GET")
                .header("Content-Type", "application/octet-stream")
                .uri(format!("{}/health", server_address))
                .body(Body::empty())
                .unwrap();
            let client = hyper::Client::new();
            match client.request(req).await {
                Ok(result) => {
                    let response = serde_json::from_slice::<serde_json::Value>(
                        &hyper::body::to_bytes(result)
                            .await
                            .expect("/health test failed with no response")
                            .to_vec(),
                    )
                    .expect("/health test failed with no response");
                    assert_eq!(response.get("healthy").unwrap().as_str().unwrap(), "true");
                }
                Err(e) => {
                    panic!("/health test failed with error: {:?}", e);
                }
            }
        });
    }

    #[test]
    #[sequential]
    fn submit_request_test() {
        tokio_test::block_on(async {
            let server_address: String = std::env::var("SERVER_ADDRESS").unwrap();

            let req = Request::builder()
                .method("POST")
                .header("Content-Type", "application/octet-stream")
                .uri(format!("{}/submit/{}", server_address, APPCHAIN))
                .body(Body::from("transaction data"))
                .unwrap();
            let client = hyper::Client::new();
            match client.request(req).await {
                Ok(result) => {
                    let response = serde_json::from_slice::<serde_json::Value>(
                        &hyper::body::to_bytes(result)
                            .await
                            .expect("/submit test failed with no response")
                            .to_vec(),
                    )
                    .expect("/submit test failed with no response");
                    assert_eq!(
                        response.get("hash").unwrap().as_str().unwrap(),
                        SUBMIT_HASH_EXPECTED
                    );
                }
                Err(e) => {
                    panic!("/submit test failed with error: {:?}", e);
                }
            }
        });
    }
}
