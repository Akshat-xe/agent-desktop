use super::*;
use clap::Parser;
use clap::error::ErrorKind;

fn sample(schema: &Value) -> Value {
    let item = if schema["type"] == "array" {
        &schema["items"]
    } else {
        schema
    };
    let value = match (item["type"].as_str(), item["enum"].get(0)) {
        (_, Some(first)) => first.clone(),
        (Some("boolean"), _) => Value::Bool(true),
        (Some("integer" | "number"), _) => Value::from(1),
        _ => Value::from("x"),
    };
    if schema["type"] == "array" {
        Value::Array(vec![value])
    } else {
        value
    }
}

fn parse(tool: &Tool, arguments: &Map<String, Value>) -> Result<Cli, clap::Error> {
    let argv = tool
        .argv(arguments)
        .expect("schema-shaped arguments build argv");
    Cli::try_parse_from(std::iter::once("agent-desktop".to_string()).chain(argv))
}

#[test]
fn every_schema_property_is_accepted_by_the_cli_parser() {
    let mut checked = 0;
    for tool in tools() {
        let definition = tool.definition();
        let properties = definition["inputSchema"]["properties"]
            .as_object()
            .expect("properties")
            .clone();
        for (key, schema) in &properties {
            let mut arguments = Map::new();
            arguments.insert(key.clone(), sample(schema));
            let Err(error) = parse(&tool, &arguments) else {
                checked += 1;
                continue;
            };
            let plain_string = schema["type"] == "string" && schema.get("enum").is_none();
            let tolerated = matches!(
                error.kind(),
                ErrorKind::MissingRequiredArgument | ErrorKind::ArgumentConflict
            ) || (plain_string && error.kind() == ErrorKind::ValueValidation);
            assert!(
                tolerated,
                "{} rejected its own schema property '{key}': {error}",
                tool.name
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} properties were checked");
}

#[test]
fn every_tool_runs_the_command_it_is_named_after() {
    for tool in tools() {
        let error = match parse(&tool, &Map::new()) {
            Ok(cli) => {
                let command = cli.command.expect("a command");
                assert_eq!(command.name(), tool.path[0], "{}", tool.name);
                continue;
            }
            Err(error) => error,
        };
        assert!(
            error.kind() == ErrorKind::MissingRequiredArgument,
            "{} failed to parse with no arguments: {error}",
            tool.name
        );
    }
}

#[test]
fn tool_names_are_unique_and_valid_for_every_major_client() {
    let names = tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();
    let unique = names.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique.len(), names.len(), "duplicate tool names: {names:?}");
    for name in &names {
        assert!(
            name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "{name} is not a portable tool name"
        );
    }
}

#[test]
fn unknown_arguments_and_wrong_types_are_reported_not_passed_through() {
    let tool = tools()
        .into_iter()
        .find(|tool| tool.path == ["skills", "get"])
        .expect("skills get tool");
    let mut arguments = Map::new();
    arguments.insert("name".into(), Value::from("desktop"));
    arguments.insert("full".into(), Value::from(true));
    arguments.insert("reference".into(), Value::Null);
    assert_eq!(
        tool.argv(&arguments).unwrap(),
        ["skills", "get", "--full", "--", "desktop"]
    );
    arguments.insert("full".into(), Value::from("yes"));
    assert!(tool.argv(&arguments).unwrap_err().contains("boolean"));
    arguments.remove("full");
    arguments.insert("headed".into(), Value::from(true));
    assert!(
        tool.argv(&arguments)
            .unwrap_err()
            .contains("Unknown argument 'headed'")
    );
}
