use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub const COMPATIBILITY_CLI_VERSION: &str = "1.5.1";
pub const COMPATIBILITY_COMMIT: &str = "6a6b50b1f71438768512e1b67475ba2bd555a018";

#[derive(Debug, Clone)]
pub struct CodeConnectProjectConfig {
    pub root: PathBuf,
    pub label: Option<String>,
    pub language: Option<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub document_url_substitutions: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct CodeConnectProject {
    pub config: CodeConnectProjectConfig,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemplateData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub props: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nestable: Option<bool>,
    #[serde(rename = "isParserless", skip_serializing_if = "Option::is_none")]
    pub is_parserless: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeConnectLink {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeConnectDocument {
    #[serde(rename = "figmaNode")]
    pub figma_node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<Value>,
    pub template: String,
    #[serde(rename = "templateData")]
    pub template_data: TemplateData,
    pub language: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<CodeConnectLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(rename = "sourceLocation", skip_serializing_if = "Option::is_none")]
    pub source_location: Option<Value>,
    pub metadata: Value,
    #[serde(
        rename = "_codeConnectFilePath",
        skip_serializing_if = "Option::is_none"
    )]
    pub code_connect_file_path: Option<String>,
}

impl CodeConnectDocument {
    pub fn upload_value(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(obj) = value.as_object_mut() {
            obj.remove("_codeConnectFilePath");
        }
        value
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FigmaPropertyDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub prop_type: String,
    #[serde(default)]
    pub variant_options: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FigmaComponentContext {
    pub figma_node: String,
    pub file_key: String,
    pub node_id: String,
    pub name: String,
    pub normalized_name: String,
    #[serde(default)]
    pub properties: Vec<FigmaPropertyDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationReport {
    pub valid: bool,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}
