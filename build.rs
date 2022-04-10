fn main() -> Result<(), Box<dyn std::error::Error>> {
	tonic_build::configure()
		.build_client(false)
		.build_server(true)
		.compile_well_known_types(true)
		.compile(&["./proto/links.proto"], &["./proto"])?;

	Ok(())
}
