use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct CommandTree {
    pub version: u32,
    pub default_base_url: String,
    pub default_api_path: String,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct Resource {
    pub name: String,
    pub ops: Vec<Operation>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct Operation {
    pub name: String,
    pub method: String,
    pub args: Vec<ArgDef>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct ArgDef {
    pub name: String,
    pub flag: String,
    pub schema_type: Option<String>,
    pub item_type: Option<String>,
    pub format: Option<String>,
    pub required: bool,
    pub list: bool,
}

pub fn load_command_tree() -> CommandTree {
    let raw = include_str!("../schemas/command_tree.json");
    serde_json::from_str(raw).expect("invalid command_tree.json")
}
