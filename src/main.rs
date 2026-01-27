mod command_tree;
mod http;

use anyhow::{Context, Result, anyhow};
use clap::{Arg, ArgAction, Command};
use command_tree::{ArgDef, CommandTree, Operation};
use serde_json::{Map, Value, json};
use std::{env, io::Write};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let tree = command_tree::load_command_tree();
    let cli = build_cli(&tree);
    let matches = cli.get_matches();

    if let Some(matches) = matches.subcommand_matches("list") {
        return handle_list(&tree, matches);
    }
    if let Some(matches) = matches.subcommand_matches("describe") {
        return handle_describe(&tree, matches);
    }
    if let Some(matches) = matches.subcommand_matches("tree") {
        return handle_tree(&tree, matches);
    }

    let token = env::var("PENPOT_ACCESS_TOKEN").context("PENPOT_ACCESS_TOKEN missing")?;
    let api_url = resolve_api_url(&tree)?;

    let pretty = matches.get_flag("pretty");
    let input_override = matches.get_one::<String>("input").map(String::as_str);

    let (res_name, res_matches) = matches
        .subcommand()
        .ok_or_else(|| anyhow!("resource required"))?;
    let (op_name, op_matches) = res_matches
        .subcommand()
        .ok_or_else(|| anyhow!("operation required"))?;

    let op = find_op(&tree, res_name, op_name)
        .ok_or_else(|| anyhow!("unknown command {res_name} {op_name}"))?;

    let body = if let Some(input) = input_override {
        if has_any_args(op_matches, &op.args) {
            return Err(anyhow!("--input cannot be combined with other args"));
        }
        serde_json::from_str(input).context("invalid JSON for --input")?
    } else {
        build_body(&op, op_matches)?
    };

    let url = format!("{}/{}", api_url.trim_end_matches('/'), op.method);
    let client = http::HttpClient::new(token)?;
    let response = client.post_json(&url, &body)?;

    if pretty {
        write_stdout_line(&serde_json::to_string_pretty(&response)?)?;
    } else {
        write_stdout_line(&serde_json::to_string(&response)?)?;
    }

    Ok(())
}

fn resolve_api_url(tree: &CommandTree) -> Result<String> {
    if let Ok(url) = env::var("PENPOT_API_URL") {
        return Ok(url);
    }
    let base = env::var("PENPOT_BASE_URL").unwrap_or_else(|_| tree.default_base_url.clone());
    Ok(join_url(&base, &tree.default_api_path))
}

fn join_url(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{}/{}", base, path)
}

fn build_cli(tree: &CommandTree) -> Command {
    let mut cmd = Command::new("penpot")
        .about("Penpot CLI (auto-generated)")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("pretty")
                .long("pretty")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Pretty-print JSON output"),
        )
        .arg(
            Arg::new("input")
                .long("input")
                .global(true)
                .value_name("JSON")
                .help("Provide full JSON request body"),
        );

    cmd = cmd.subcommand(
        Command::new("list")
            .about("List resources and operations")
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help("Emit machine-readable JSON"),
            ),
    );

    cmd = cmd.subcommand(
        Command::new("describe")
            .about("Describe a specific operation")
            .arg(Arg::new("resource").required(true))
            .arg(Arg::new("op").required(true))
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help("Emit machine-readable JSON"),
            ),
    );

    cmd = cmd.subcommand(
        Command::new("tree").about("Show full command tree").arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .help("Emit machine-readable JSON"),
        ),
    );

    for resource in &tree.resources {
        let mut res_cmd = Command::new(resource.name.clone())
            .about(resource.name.clone())
            .subcommand_required(true)
            .arg_required_else_help(true);
        for op in &resource.ops {
            let mut op_cmd = Command::new(op.name.clone()).about(op.method.clone());
            for arg in &op.args {
                op_cmd = op_cmd.arg(build_arg(arg));
            }
            res_cmd = res_cmd.subcommand(op_cmd);
        }
        cmd = cmd.subcommand(res_cmd);
    }

    cmd
}

fn handle_list(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<()> {
    if matches.get_flag("json") {
        let mut out = Vec::new();
        for res in &tree.resources {
            let ops: Vec<String> = res.ops.iter().map(|op| op.name.clone()).collect();
            out.push(json!({"resource": res.name, "ops": ops}));
        }
        write_stdout_line(&serde_json::to_string_pretty(&out)?)?;
        return Ok(());
    }

    for res in &tree.resources {
        write_stdout_line(&res.name)?;
        for op in &res.ops {
            write_stdout_line(&format!("  {}", op.name))?;
        }
    }
    Ok(())
}

fn handle_describe(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<()> {
    let resource = matches
        .get_one::<String>("resource")
        .ok_or_else(|| anyhow!("resource required"))?;
    let op_name = matches
        .get_one::<String>("op")
        .ok_or_else(|| anyhow!("operation required"))?;

    let op = find_op(tree, resource, op_name)
        .ok_or_else(|| anyhow!("unknown command {resource} {op_name}"))?;

    if matches.get_flag("json") {
        write_stdout_line(&serde_json::to_string_pretty(op)?)?;
        return Ok(());
    }

    write_stdout_line(&format!("{} {}", resource, op.name))?;
    write_stdout_line(&format!("  method: {}", op.method))?;
    if !op.args.is_empty() {
        write_stdout_line("  args:")?;
        for arg in &op.args {
            let mut line = format!("    --{}", arg.flag);
            if let Some(ty) = &arg.schema_type {
                line.push_str(&format!("  {ty}"));
            }
            if arg.required {
                line.push_str("  (required)");
            }
            write_stdout_line(&line)?;
        }
    }
    Ok(())
}

fn handle_tree(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<()> {
    if matches.get_flag("json") {
        write_stdout_line(&serde_json::to_string_pretty(tree)?)?;
        return Ok(());
    }
    write_stdout_line("Run with --json for machine-readable output.")?;
    Ok(())
}

fn write_stdout_line(value: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    if let Err(err) = out.write_all(value.as_bytes()) {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(err.into());
    }
    if let Err(err) = out.write_all(b"\n") {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(err.into());
    }
    Ok(())
}

fn build_arg(arg: &ArgDef) -> Arg {
    let mut arg_def = Arg::new(arg.name.clone())
        .long(arg.flag.clone())
        .value_name(arg_value_name(arg));

    if arg.list {
        arg_def = arg_def.action(ArgAction::Append);
    }

    arg_def
}

fn arg_value_name(arg: &ArgDef) -> String {
    let base = if arg.list {
        arg.item_type
            .clone()
            .or_else(|| arg.schema_type.clone())
            .unwrap_or_else(|| "json".to_string())
    } else {
        arg.schema_type.clone().unwrap_or_else(|| "json".to_string())
    };
    if arg.list {
        format!("list<{base}>")
    } else {
        base
    }
}

fn find_op<'a>(tree: &'a CommandTree, res: &str, op: &str) -> Option<&'a Operation> {
    tree.resources
        .iter()
        .find(|r| r.name == res)
        .and_then(|r| r.ops.iter().find(|o| o.name == op))
}

fn has_any_args(matches: &clap::ArgMatches, args: &[ArgDef]) -> bool {
    for arg in args {
        if arg.list {
            if matches.get_many::<String>(&arg.name).is_some() {
                return true;
            }
        } else if matches.get_one::<String>(&arg.name).is_some() {
            return true;
        }
    }
    false
}

fn build_body(op: &Operation, matches: &clap::ArgMatches) -> Result<Value> {
    if op.args.is_empty() {
        return Ok(http::build_empty_body());
    }

    let mut obj = Map::new();
    for arg in &op.args {
        if arg.list {
            if let Some(values) = matches.get_many::<String>(&arg.name) {
                let list_values: Vec<String> = values.cloned().collect();
                let parsed = parse_list_arg(arg, &list_values)?;
                obj.insert(arg.name.clone(), parsed);
                continue;
            }
        } else if let Some(value) = matches.get_one::<String>(&arg.name) {
            let parsed = parse_scalar_arg(arg, value)?;
            obj.insert(arg.name.clone(), parsed);
            continue;
        }

        if arg.required {
            return Err(anyhow!("missing required argument --{}", arg.flag));
        }
    }

    Ok(Value::Object(obj))
}

fn parse_list_arg(arg: &ArgDef, values: &[String]) -> Result<Value> {
    if values.len() == 1 && values[0].trim_start().starts_with('[') {
        let parsed: Value = serde_json::from_str(&values[0]).context("invalid JSON list")?;
        return Ok(parsed);
    }

    let mut out = Vec::new();
    for value in values {
        let item_type = arg
            .item_type
            .as_deref()
            .or_else(|| arg.schema_type.as_deref());
        let parsed = parse_scalar_value(item_type, arg.format.as_deref(), value)?;
        out.push(parsed);
    }
    Ok(Value::Array(out))
}

fn parse_scalar_arg(arg: &ArgDef, value: &str) -> Result<Value> {
    parse_scalar_value(arg.schema_type.as_deref(), arg.format.as_deref(), value)
}

fn parse_scalar_value(schema_type: Option<&str>, _format: Option<&str>, value: &str) -> Result<Value> {
    match schema_type.unwrap_or("") {
        "integer" => Ok(Value::Number(value.parse::<i64>()?.into())),
        "number" => Ok(json!(value.parse::<f64>()?)),
        "boolean" => Ok(Value::Bool(parse_bool(value)?)),
        "object" | "array" => {
            let parsed: Value = serde_json::from_str(value).context("invalid JSON value")?;
            Ok(parsed)
        }
        "json" => {
            if value.trim_start().starts_with('{') || value.trim_start().starts_with('[') || value.trim() == "null" {
                let parsed: Value = serde_json::from_str(value).context("invalid JSON value")?;
                Ok(parsed)
            } else {
                Ok(Value::String(value.to_string()))
            }
        }
        _ => Ok(Value::String(value.to_string())),
    }
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(anyhow!("invalid boolean: {value}")),
    }
}
