use crate::code_connect::model::{FigmaComponentContext, FigmaPropertyDefinition};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CodeComponentContext {
    pub component: String,
    pub import: String,
    pub source: String,
    pub example: String,
    pub language: Option<String>,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateRequest {
    pub figma: FigmaComponentContext,
    pub code: CodeComponentContext,
    pub label: Option<String>,
    #[serde(default)]
    pub javascript: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerateResult {
    pub file_name: String,
    pub template: String,
    pub blocking_issues: Vec<String>,
    pub warnings: Vec<String>,
    pub figma_context: FigmaComponentContext,
}

fn normalize_component_name(name: &str) -> String {
    let mut out = String::new();
    for word in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.extend(chars);
        }
    }
    if out
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        out.insert(0, '_');
    }
    if out.is_empty() {
        "Component".into()
    } else {
        out
    }
}

fn strip_prop_id(name: &str) -> &str {
    match name.rfind('#') {
        Some(idx) => &name[..idx],
        None => name,
    }
}

fn camel_case(name: &str) -> String {
    let mut out = String::new();
    for (idx, word) in name
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .enumerate()
    {
        if idx == 0 {
            out.push_str(&word.to_ascii_lowercase());
        } else {
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_ascii_uppercase());
                out.extend(chars.map(|c| c.to_ascii_lowercase()));
            }
        }
    }
    if out.is_empty() { "value".into() } else { out }
}

fn kebab(value: &str) -> String {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

fn is_boolean_variant(options: &[String]) -> bool {
    if options.len() != 2 {
        return false;
    }
    let lowered: Vec<String> = options.iter().map(|v| v.to_ascii_lowercase()).collect();
    for pair in [["true", "false"], ["yes", "no"], ["on", "off"]] {
        if lowered.contains(&pair[0].to_string()) && lowered.contains(&pair[1].to_string()) {
            return true;
        }
    }
    false
}

fn prop_lookup<'a>(
    prop_name: &str,
    figma_props: &'a [FigmaPropertyDefinition],
) -> Vec<&'a FigmaPropertyDefinition> {
    let normalized = prop_key(prop_name);
    figma_props
        .iter()
        .filter(|p| prop_key(strip_prop_id(&p.name)) == normalized)
        .collect()
}

fn prop_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn js_template_chunk(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${")
}

fn declaration(prop: &FigmaPropertyDefinition) -> Option<String> {
    let code_name = camel_case(strip_prop_id(&prop.name));
    let figma_name = strip_prop_id(&prop.name);
    let figma_name = js_string(figma_name);
    match prop.prop_type.as_str() {
        "TEXT" => Some(format!(
            "const {code_name} = figma.selectedInstance.getString({figma_name})"
        )),
        "BOOLEAN" => Some(format!(
            "const {code_name} = figma.selectedInstance.getBoolean({figma_name})"
        )),
        "VARIANT" if is_boolean_variant(&prop.variant_options) => Some(format!(
            "const {code_name} = figma.selectedInstance.getBoolean({figma_name})"
        )),
        "VARIANT" => {
            let options = prop
                .variant_options
                .iter()
                .map(|v| format!("  {}: {}", js_string(v), js_string(&kebab(v))))
                .collect::<Vec<_>>()
                .join(",\n");
            Some(format!(
                "const {code_name} = figma.selectedInstance.getEnum({figma_name}, {{\n{options}\n}})"
            ))
        }
        _ => None,
    }
}

pub fn generate_template(request: &GenerateRequest) -> Result<GenerateResult> {
    let normalized_name = if request.figma.normalized_name.trim().is_empty() {
        normalize_component_name(&request.figma.name)
    } else {
        request.figma.normalized_name.clone()
    };
    let mut blocking_issues = Vec::new();
    let mut warnings = Vec::new();
    let mut declarations = Vec::new();

    if let Some(props) = request.code.props.as_object() {
        for prop_name in props.keys() {
            let matches = prop_lookup(prop_name, &request.figma.properties);
            match matches.len() {
                0 => warnings.push(format!("No Figma property matched code prop `{prop_name}`")),
                1 => {
                    if let Some(decl) = declaration(matches[0]) {
                        declarations.push(decl);
                    } else {
                        blocking_issues.push(format!(
                            "Unsupported Figma property type `{}` for `{}`",
                            matches[0].prop_type, matches[0].name
                        ));
                    }
                }
                _ => blocking_issues.push(format!(
                    "ambiguous Figma property mapping for code prop `{prop_name}`"
                )),
            }
        }
    }

    for prop in &request.figma.properties {
        if prop.prop_type == "INSTANCE_SWAP" {
            blocking_issues.push(format!(
                "Unsupported automatic INSTANCE_SWAP mapping for `{}`",
                prop.name
            ));
        }
    }

    let extension = if request.javascript { "js" } else { "ts" };
    let declaration_block = if declarations.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", declarations.join("\n"))
    };
    let template = format!(
        "// url={}\n// component={}\n// source={}\nimport figma from \"figma\"\n\n{}export default {{\n  example: figma.code`{}`,\n  imports: [{}],\n  id: {},\n  metadata: {{ nestable: true }},\n}}\n",
        request.figma.figma_node,
        request.code.component,
        request.code.source,
        declaration_block,
        js_template_chunk(&request.code.example),
        js_string(&request.code.import),
        js_string(&normalized_name)
    );
    Ok(GenerateResult {
        file_name: format!("{normalized_name}.figma.{extension}"),
        template,
        blocking_issues,
        warnings,
        figma_context: request.figma.clone(),
    })
}
