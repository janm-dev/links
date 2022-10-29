//! Integration tests for statistic collection in the server

mod util;

use links::api::GetStatisticsRequest;
use reqwest::{redirect::Policy, ClientBuilder};
use tonic::Request;
use util::get_rpc_client;

/// HTTP/1.1 statistic collection tests
#[tokio::test]
#[serial_test::serial]
async fn http1_collection() {
	let _terminator = util::start_server(false);

	let client = ClientBuilder::new()
		.http1_only()
		.redirect(Policy::none())
		.user_agent("links-integration-test/1.1")
		.build()
		.unwrap();

	let mut rpc_client = get_rpc_client("localhost", 50051, false).await;

	client.get("http://localhost/example").send().await.unwrap();
	client
		.get("http://localhost/nonexistent")
		.send()
		.await
		.unwrap();

	let mut rpc_req = Request::new(GetStatisticsRequest {
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	// 4 from "/example", 4 from its ID, 4 from "/nonexistent"
	assert_eq!(stats.len(), 12);

	// Enabled
	assert!(stats.iter().any(|s| s.r#type == "request"));
	assert!(stats.iter().any(|s| s.r#type == "host_request"));
	assert!(stats.iter().any(|s| s.r#type == "status_code"));
	assert!(stats.iter().any(|s| s.r#type == "http_version"));

	// Enabled, but no data
	assert!(stats.iter().all(|s| s.r#type != "sni_request"));
	assert!(stats.iter().all(|s| s.r#type != "tls_version"));
	assert!(stats.iter().all(|s| s.r#type != "tls_cipher_suite"));

	// Not enabled
	assert!(stats.iter().all(|s| s.r#type != "user_agent"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_mobile"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_platform"));

	let mut rpc_req = Request::new(GetStatisticsRequest {
		link: Some("example".to_string()),
		r#type: Some("http_version".to_string()),
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	assert_eq!(stats.len(), 1);
	assert_eq!(stats[0].data, "HTTP/1.1");
	assert_eq!(stats[0].r#type, "http_version");
	assert_eq!(stats[0].link, "example");
	assert_eq!(stats[0].value, 1);
}

/// HTTP/2.0 statistic collection tests
#[tokio::test]
#[serial_test::serial]
async fn http2_collection() {
	let _terminator = util::start_server(false);

	let client = ClientBuilder::new()
		.http2_prior_knowledge()
		.redirect(Policy::none())
		.user_agent("links-integration-test/2")
		.build()
		.unwrap();

	let mut rpc_client = get_rpc_client("localhost", 50051, false).await;

	client.get("http://localhost/example").send().await.unwrap();
	client
		.get("http://localhost/nonexistent")
		.send()
		.await
		.unwrap();

	let mut rpc_req = Request::new(GetStatisticsRequest {
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	// 4 from "/example", 4 from its ID, 4 from "/nonexistent"
	assert_eq!(stats.len(), 12);

	// Enabled
	assert!(stats.iter().any(|s| s.r#type == "request"));
	assert!(stats.iter().any(|s| s.r#type == "host_request"));
	assert!(stats.iter().any(|s| s.r#type == "status_code"));
	assert!(stats.iter().any(|s| s.r#type == "http_version"));

	// Enabled, but no data
	assert!(stats.iter().all(|s| s.r#type != "sni_request"));
	assert!(stats.iter().all(|s| s.r#type != "tls_version"));
	assert!(stats.iter().all(|s| s.r#type != "tls_cipher_suite"));

	// Not enabled
	assert!(stats.iter().all(|s| s.r#type != "user_agent"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_mobile"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_platform"));

	let mut rpc_req = Request::new(GetStatisticsRequest {
		link: Some("example".to_string()),
		r#type: Some("http_version".to_string()),
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	assert_eq!(stats.len(), 1);
	assert_eq!(stats[0].data, "HTTP/2");
	assert_eq!(stats[0].r#type, "http_version");
	assert_eq!(stats[0].link, "example");
	assert_eq!(stats[0].value, 1);
}

/// HTTPS/1.1 statistic collection tests
#[tokio::test]
#[serial_test::serial]
async fn https1_collection() {
	let _terminator = util::start_server(true);

	let client = ClientBuilder::new()
		.http1_only()
		.https_only(true)
		.redirect(Policy::none())
		.user_agent("links-integration-test/1.1")
		.build()
		.unwrap();

	let mut rpc_client = get_rpc_client("localhost", 530, true).await;

	client
		.get("https://localhost/example")
		.send()
		.await
		.unwrap();
	client
		.get("https://localhost/nonexistent")
		.send()
		.await
		.unwrap();

	let mut rpc_req = Request::new(GetStatisticsRequest {
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	// 7 from "/example", 7 from its ID, 7 from "/nonexistent"
	assert_eq!(stats.len(), 21);

	// Enabled
	assert!(stats.iter().any(|s| s.r#type == "request"));
	assert!(stats.iter().any(|s| s.r#type == "host_request"));
	assert!(stats.iter().any(|s| s.r#type == "sni_request"));
	assert!(stats.iter().any(|s| s.r#type == "status_code"));
	assert!(stats.iter().any(|s| s.r#type == "http_version"));
	assert!(stats.iter().any(|s| s.r#type == "tls_version"));
	assert!(stats.iter().any(|s| s.r#type == "tls_cipher_suite"));

	// Not enabled
	assert!(stats.iter().all(|s| s.r#type != "user_agent"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_mobile"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_platform"));

	let mut rpc_req = Request::new(GetStatisticsRequest {
		link: Some("example".to_string()),
		r#type: Some("http_version".to_string()),
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	assert_eq!(stats.len(), 1);
	assert_eq!(stats[0].data, "HTTP/1.1");
	assert_eq!(stats[0].r#type, "http_version");
	assert_eq!(stats[0].link, "example");
	assert_eq!(stats[0].value, 1);
}

/// HTTPS/2.0 statistic collection tests
#[tokio::test]
#[serial_test::serial]
async fn https2_collection() {
	let _terminator = util::start_server(true);

	let client = ClientBuilder::new()
		.http2_prior_knowledge()
		.https_only(true)
		.redirect(Policy::none())
		.user_agent("links-integration-test/2")
		.build()
		.unwrap();

	let mut rpc_client = get_rpc_client("localhost", 530, true).await;

	client
		.get("https://localhost/example")
		.send()
		.await
		.unwrap();
	client
		.get("https://localhost/nonexistent")
		.send()
		.await
		.unwrap();

	let mut rpc_req = Request::new(GetStatisticsRequest {
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	// 7 from "/example", 7 from its ID, 7 from "/nonexistent"
	assert_eq!(stats.len(), 21);

	// Enabled
	assert!(stats.iter().any(|s| s.r#type == "request"));
	assert!(stats.iter().any(|s| s.r#type == "host_request"));
	assert!(stats.iter().any(|s| s.r#type == "sni_request"));
	assert!(stats.iter().any(|s| s.r#type == "status_code"));
	assert!(stats.iter().any(|s| s.r#type == "http_version"));
	assert!(stats.iter().any(|s| s.r#type == "tls_version"));
	assert!(stats.iter().any(|s| s.r#type == "tls_cipher_suite"));

	// Not enabled
	assert!(stats.iter().all(|s| s.r#type != "user_agent"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_mobile"));
	assert!(stats.iter().all(|s| s.r#type != "user_agent_platform"));

	let mut rpc_req = Request::new(GetStatisticsRequest {
		link: Some("example".to_string()),
		r#type: Some("http_version".to_string()),
		..Default::default()
	});
	rpc_req
		.metadata_mut()
		.append("auth", "abc123".parse().unwrap());
	let stats = rpc_client
		.get_statistics(rpc_req)
		.await
		.unwrap()
		.into_inner()
		.statistics;

	assert_eq!(stats.len(), 1);
	assert_eq!(stats[0].data, "HTTP/2");
	assert_eq!(stats[0].r#type, "http_version");
	assert_eq!(stats[0].link, "example");
	assert_eq!(stats[0].value, 1);
}
