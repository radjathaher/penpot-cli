mod command_tree;
mod http;
mod mcp;

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use clap::{Arg, ArgAction, Command};
use command_tree::{ArgDef, CommandTree, Operation};
use mcp::{McpClient, base64_encode, infer_mime};
use serde_json::{Map, Value, json};
use std::{env, fs, io::Write, path::Path};

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
    if let Some(matches) = matches.subcommand_matches("mcp") {
        return handle_mcp(matches);
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

    cmd = cmd.subcommand(build_mcp_cli());

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

fn build_mcp_cli() -> Command {
    let mut cmd = Command::new("mcp")
        .about("Penpot MCP tools (plugin-based)")
        .arg(
            Arg::new("mcp_url")
                .long("mcp-url")
                .value_name("URL")
                .help("MCP HTTP endpoint URL (default: PENPOT_MCP_URL)"),
        )
        .arg(
            Arg::new("mcp_api_key")
                .long("mcp-api-key")
                .value_name("KEY")
                .help("MCP API key (default: PENPOT_MCP_API_KEY)"),
        )
        .subcommand_required(true)
        .arg_required_else_help(true);

    cmd = cmd.subcommand(Command::new("overview").about("Show MCP high-level overview"));
    cmd = cmd.subcommand(
        Command::new("api-info")
            .about("Get Penpot Plugin API info")
            .arg(Arg::new("type").long("type").required(true))
            .arg(Arg::new("member").long("member")),
    );
    cmd = cmd.subcommand(
        Command::new("exec")
            .about("Execute JS in plugin context")
            .arg(Arg::new("code").long("code").value_name("JS"))
            .arg(
                Arg::new("file")
                    .long("file")
                    .value_name("PATH")
                    .help("Read JS code from file"),
            ),
    );
    cmd = cmd.subcommand(
        Command::new("export-shape")
            .about("Export shape to PNG or SVG")
            .arg(Arg::new("shape_id").long("shape-id").required(true))
            .arg(
                Arg::new("format")
                    .long("format")
                    .value_name("png|svg")
                    .default_value("png"),
            )
            .arg(
                Arg::new("mode")
                    .long("mode")
                    .value_name("shape|fill")
                    .default_value("shape"),
            )
            .arg(
                Arg::new("out")
                    .long("out")
                    .value_name("PATH")
                    .help("Write output to local file"),
            ),
    );
    cmd = cmd.subcommand(
        Command::new("import-image")
            .about("Import local image via plugin")
            .arg(Arg::new("file").long("file").required(true))
            .arg(Arg::new("x").long("x"))
            .arg(Arg::new("y").long("y"))
            .arg(Arg::new("width").long("width"))
            .arg(Arg::new("height").long("height")),
    );

    cmd
}

fn handle_mcp(matches: &clap::ArgMatches) -> Result<()> {
    let mcp_url = matches
        .get_one::<String>("mcp_url")
        .cloned()
        .or_else(|| env::var("PENPOT_MCP_URL").ok())
        .ok_or_else(|| anyhow!("PENPOT_MCP_URL missing"))?;
    let api_key = matches
        .get_one::<String>("mcp_api_key")
        .cloned()
        .or_else(|| env::var("PENPOT_MCP_API_KEY").ok());
    let pretty = matches.get_flag("pretty");
    let mut client = McpClient::new(mcp_url, api_key)?;

    let (cmd, cmd_matches) = matches
        .subcommand()
        .ok_or_else(|| anyhow!("mcp subcommand required"))?;

    let result = match cmd {
        "overview" => client.call_tool("high_level_overview", json!({}))?,
        "api-info" => {
            let ty = cmd_matches
                .get_one::<String>("type")
                .ok_or_else(|| anyhow!("--type required"))?;
            let member = cmd_matches.get_one::<String>("member");
            let mut args = Map::new();
            args.insert("type".to_string(), json!(ty));
            if let Some(member) = member {
                args.insert("member".to_string(), json!(member));
            }
            client.call_tool("penpot_api_info", Value::Object(args))?
        }
        "exec" => {
            let code = if let Some(path) = cmd_matches.get_one::<String>("file") {
                fs::read_to_string(path).context("read code file")?
            } else {
                cmd_matches
                    .get_one::<String>("code")
                    .cloned()
                    .ok_or_else(|| anyhow!("--code or --file required"))?
            };
            client.call_tool("execute_code", json!({ "code": code }))?
        }
        "export-shape" => {
            let shape_id = cmd_matches
                .get_one::<String>("shape_id")
                .ok_or_else(|| anyhow!("--shape-id required"))?;
            let format = cmd_matches
                .get_one::<String>("format")
                .map(String::as_str)
                .unwrap_or("png");
            let mode = cmd_matches
                .get_one::<String>("mode")
                .map(String::as_str)
                .unwrap_or("shape");
            let out_path = cmd_matches.get_one::<String>("out");

            let result = client.call_tool(
                "export_shape",
                json!({
                    "shapeId": shape_id,
                    "format": format,
                    "mode": mode
                }),
            )?;

            if let Some(out_path) = out_path {
                write_mcp_output_file(out_path, &result)?;
                json!({ "saved": out_path })
            } else {
                result
            }
        }
        "import-image" => {
            let path = cmd_matches
                .get_one::<String>("file")
                .ok_or_else(|| anyhow!("--file required"))?;
            let file_path = Path::new(path);
            let bytes = fs::read(file_path).context("read image file")?;
            let mime = infer_mime(file_path)?;
            let name = file_path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("image");
            let base64 = base64_encode(&bytes);
            let x = cmd_matches.get_one::<String>("x").map(String::as_str);
            let y = cmd_matches.get_one::<String>("y").map(String::as_str);
            let width = cmd_matches.get_one::<String>("width").map(String::as_str);
            let height = cmd_matches.get_one::<String>("height").map(String::as_str);

            let code = format!(
                "const rect = await penpotUtils.importImage({base64}, {mime}, {name}, {x}, {y}, {width}, {height}); return {{ shapeId: rect.id }};",
                base64 = serde_json::to_string(&base64)?,
                mime = serde_json::to_string(mime)?,
                name = serde_json::to_string(name)?,
                x = x.unwrap_or("undefined"),
                y = y.unwrap_or("undefined"),
                width = width.unwrap_or("undefined"),
                height = height.unwrap_or("undefined"),
            );

            client.call_tool("execute_code", json!({ "code": code }))?
        }
        _ => return Err(anyhow!("unknown mcp subcommand: {cmd}")),
    };

    if pretty {
        write_stdout_line(&serde_json::to_string_pretty(&result)?)?;
    } else {
        write_stdout_line(&serde_json::to_string(&result)?)?;
    }
    Ok(())
}

fn write_mcp_output_file(path: &str, result: &Value) -> Result<()> {
    let content = result
        .get("content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("mcp result missing content"))?;
    let first = content
        .first()
        .ok_or_else(|| anyhow!("mcp content empty"))?;
    let ty = first.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match ty {
        "image" => {
            let data = first
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("mcp image data missing"))?;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .context("decode base64 image")?;
            fs::write(path, bytes).context("write image file")?;
        }
        "text" => {
            let text = first
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("mcp text missing"))?;
            fs::write(path, text).context("write text file")?;
        }
        _ => return Err(anyhow!("unsupported mcp content type: {ty}")),
    }
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
        arg.schema_type
            .clone()
            .unwrap_or_else(|| "json".to_string())
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

fn parse_scalar_value(
    schema_type: Option<&str>,
    _format: Option<&str>,
    value: &str,
) -> Result<Value> {
    match schema_type.unwrap_or("") {
        "integer" => Ok(Value::Number(value.parse::<i64>()?.into())),
        "number" => Ok(json!(value.parse::<f64>()?)),
        "boolean" => Ok(Value::Bool(parse_bool(value)?)),
        "object" | "array" => {
            let parsed: Value = serde_json::from_str(value).context("invalid JSON value")?;
            Ok(parsed)
        }
        "json" => {
            if value.trim_start().starts_with('{')
                || value.trim_start().starts_with('[')
                || value.trim() == "null"
            {
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
