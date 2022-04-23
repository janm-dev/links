use std::env;
use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Compile gRPC/protobuf
	tonic_build::configure()
		.build_client(true)
		.build_server(true)
		.compile_well_known_types(true)
		.compile(&["./proto/links.proto"], &["./proto"])?;

	// Disable pedantic clippy lints in the generated file (if anyone has a
	// more elegant solution to this, please open an issue)
	let out_dir = env::var_os("OUT_DIR").unwrap();
	let proto_path = Path::new(&out_dir).join("links.rs");
	let proto = fs::read_to_string(&proto_path)?;
	fs::write(
		&proto_path,
		"#[allow(clippy::pedantic)]\npub mod rpc {\n".to_string() + &proto + "}\n",
	)?;

	Ok(())
}
