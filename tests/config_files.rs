//! Test example and docker configuration files.

use std::{env, fs};

use links::config::Partial;

#[test]
fn valid_config_files() {
	let path = fs::canonicalize(file!())
		.unwrap()
		.join("../../example_config");

	// JSON must first have comments removed to be checked.
	let json = fs::read_to_string(&path.with_extension("json")).unwrap();
	let json_path = env::temp_dir()
		.join("links_test_json")
		.with_extension("json");
	let json = json
		.lines()
		.filter(|l| !l.trim().starts_with("//"))
		.collect::<String>();
	fs::write(&json_path, json).unwrap();

	let json = Partial::from_file(&json_path);
	let toml = Partial::from_file(&path.with_extension("toml"));
	let yaml = Partial::from_file(&path.with_extension("yaml"));

	assert!(json.is_ok());
	assert!(toml.is_ok());
	assert!(yaml.is_ok());

	fs::remove_file(json_path).unwrap();
}

#[test]
fn json_example_is_complete() {
	let config = Partial::from_json(
		&include_str!("../example_config.json")
			.lines()
			.filter(|l| !l.trim().starts_with("//"))
			.collect::<String>(),
	)
	.unwrap();

	assert!(!format!("{config:?}").contains("None"));
	assert_eq!(
		config.store_config.unwrap().get("option"),
		Some(&"value".to_string())
	)
}

#[test]
fn toml_example_is_complete() {
	let config = Partial::from_toml(include_str!("../example_config.toml")).unwrap();

	assert!(!format!("{config:?}").contains("None"));
	assert_eq!(
		config.store_config.unwrap().get("option"),
		Some(&"value".to_string())
	)
}

#[test]
fn yaml_example_is_complete() {
	let config = Partial::from_yaml(include_str!("../example_config.yaml")).unwrap();

	assert!(!format!("{config:?}").contains("None"));
	assert_eq!(
		config.store_config.unwrap().get("option"),
		Some(&"value".to_string())
	)
}

#[test]
fn examples_are_equivalent() {
	let path = fs::canonicalize(file!())
		.unwrap()
		.join("../../example_config");

	// JSON must first have comments removed to be checked.
	let json = fs::read_to_string(&path.with_extension("json")).unwrap();
	let json_path = env::temp_dir()
		.join("links_test_json")
		.with_extension("json");
	let json = json
		.lines()
		.filter(|l| !l.trim().starts_with("//"))
		.collect::<String>();
	fs::write(&json_path, json).unwrap();

	let json = Partial::from_file(&json_path).unwrap();
	let toml = Partial::from_file(&path.with_extension("toml")).unwrap();
	let yaml = Partial::from_file(&path.with_extension("yaml")).unwrap();

	assert_eq!(json, toml);
	assert_eq!(json, yaml);
	assert_eq!(toml, yaml);
}

#[test]
fn docker_config_is_valid() {
	let path = fs::canonicalize(file!())
		.unwrap()
		.join("../../example_config");

	let config = Partial::from_file(&path.with_extension("toml"));

	assert!(config.is_ok());
}
