use std::{
	cell::RefCell,
	env, fs,
	path::{Path, PathBuf},
	rc::Rc,
};

use lol_html::{element, text, RewriteStrSettings};
use minify_html::Cfg;
use sha2::{Digest, Sha256};

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
		"#[allow(clippy::pedantic, clippy::cargo, clippy::nursery, missing_docs, rustdoc::all, \
		 clippy::derive_partial_eq_without_eq)]\npub mod rpc {\n"
			.to_string() + &proto
			+ "}\n",
	)?;

	// Include and minify html pages
	minify("not-found", PathBuf::from("misc/not-found.html"));
	minify("redirect", PathBuf::from("misc/redirect.html"));
	minify("bad-request", PathBuf::from("misc/bad-request.html"));
	minify("https-redirect", PathBuf::from("misc/https-redirect.html"));

	// Generate hashes for the CSP header
	hash_tags("style", [
		"not-found",
		"redirect",
		"bad-request",
		"https-redirect",
	]);

	println!("cargo:rerun-if-changed=./proto/links.proto");
	println!("cargo:rerun-if-changed=./proto/*");

	Ok(())
}

/// Minify the html file in `path`. The resulting file will be output into the
/// `OUT_DIR` directory with the name `name.html`
fn minify(name: &str, path: PathBuf) {
	println!("cargo:rerun-if-changed={}", path.to_str().unwrap());

	let out_path = PathBuf::from(env::var_os("OUT_DIR").unwrap())
		.join(name)
		.with_extension("html");
	let html = fs::read_to_string(path).unwrap();

	let config = Cfg {
		do_not_minify_doctype: true,
		ensure_spec_compliant_unquoted_attribute_values: true,
		keep_closing_tags: false,
		keep_html_and_head_opening_tags: false,
		keep_spaces_between_attributes: true,
		keep_comments: false,
		minify_css: true,
		minify_js: true,
		remove_bangs: true,
		remove_processing_instructions: true,
	};

	let minified = minify_html::minify(html.as_bytes(), &config);

	fs::write(&out_path, &minified).unwrap();
}

/// Generate the hashes for all `<tag_name>` elements in the provided generated
/// html files, and store them in a CSP-ready format in
/// `OUT_DIR/file_name.tag_name.hash`. Must be run **after** the minifying step.
fn hash_tags(tag_name: &'static str, names: impl IntoIterator<Item = &'static str>) {
	for name in names {
		let contents = Rc::new(RefCell::new(Vec::<String>::new()));

		let file = PathBuf::from(env::var_os("OUT_DIR").unwrap())
			.join(name)
			.with_extension("html");
		let content = fs::read_to_string(file).unwrap();

		// Get the contents of all specified tags
		let buffer = Rc::new(RefCell::new(String::new()));
		let _ = lol_html::rewrite_str(&content, RewriteStrSettings {
			element_content_handlers: vec![
				element!(tag_name, |el| {
					buffer.borrow_mut().clear();
					let buffer = buffer.clone();
					let contents = contents.clone();

					el.on_end_tag(move |_| {
						let s = buffer.borrow();
						contents.borrow_mut().push(s.to_owned());

						Ok(())
					})?;

					Ok(())
				}),
				text!(tag_name, |t| {
					buffer.borrow_mut().push_str(t.as_str());

					Ok(())
				}),
			],
			..RewriteStrSettings::default()
		})
		.unwrap();

		let contents = contents
			.take()
			.into_iter()
			.map(|v| {
				let mut hasher = Sha256::new();
				hasher.update(v);
				let res = hasher.finalize();
				"'sha256-".to_string() + &base64::encode(res) + "'"
			})
			.collect::<Vec<String>>()[..]
			.join(" ");

		let out_path =
			PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(format!("{name}.{tag_name}.hash"));

		fs::write(out_path, contents).unwrap();
	}
}
