use fighorse::code_connect::generate::{generate_template, CodeComponentContext, GenerateRequest};
use fighorse::code_connect::model::{FigmaComponentContext, FigmaPropertyDefinition};
use fighorse::code_connect::project::load_project;
use fighorse::code_connect::template::parse_project;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("fighorse-{name}-{nonce}"));
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn parserless_templates_are_discovered_without_executing_code() {
    let root = temp_root("code-connect-template");
    write(
        &root.join("figma.config.json"),
        r#"{"codeConnect":{"include":["src/**/*.figma.ts"],"label":"React","language":"jsx"}}"#,
    );
    write(
        &root.join("src/Button.figma.ts"),
        r#"// url=https://www.figma.com/design/ABCDEF/Test?node-id=1-2
// component=Button
// source=src/Button.tsx
import figma from 'figma'

globalThis.__FIGHORSE_TEST_SHOULD_NOT_RUN = true
const label = figma.selectedInstance.getString("Label")

export default {
  example: figma.code`<Button label="${label}" />`,
  imports: ['import { Button } from "./Button"'],
  id: "Button",
  metadata: { nestable: true },
}
"#,
    );
    write(
        &root.join("node_modules/Bad.figma.ts"),
        "// url=https://www.figma.com/design/BAD/Test?node-id=1-2\nexport default {}",
    );

    std::env::remove_var("__FIGHORSE_TEST_SHOULD_NOT_RUN");
    let project = load_project(&root, None).expect("load project");
    assert_eq!(project.files.len(), 1);

    let docs = parse_project(&project).expect("parse templates");
    assert_eq!(docs.len(), 1);
    let doc = &docs[0];
    assert_eq!(
        doc.figma_node,
        "https://www.figma.com/design/ABCDEF/Test?node-id=1-2"
    );
    assert_eq!(doc.component.as_deref(), Some("Button"));
    assert_eq!(doc.source.as_deref(), Some("src/Button.tsx"));
    assert_eq!(doc.label, "React");
    assert_eq!(doc.language, "jsx");
    assert!(doc.template.contains("require('figma')"));
    assert_eq!(doc.template_data.is_parserless, Some(true));
    assert!(std::env::var("__FIGHORSE_TEST_SHOULD_NOT_RUN").is_err());
}

#[test]
fn project_config_rejects_repository_controlled_api_url() {
    let root = temp_root("code-connect-config");
    write(
        &root.join("figma.config.json"),
        r#"{"codeConnect":{"apiUrl":"https://evil.example","include":["**/*.figma.ts"]}}"#,
    );

    let err = load_project(&root, None).expect_err("apiUrl must be rejected");
    assert!(err.to_string().contains("apiUrl"));
}

#[test]
fn generator_maps_simple_props_and_blocks_ambiguous_input() {
    let figma = FigmaComponentContext {
        figma_node: "https://www.figma.com/design/ABCDEF/Test?node-id=1-2".into(),
        file_key: "ABCDEF".into(),
        node_id: "1:2".into(),
        name: "Primary Button".into(),
        normalized_name: "PrimaryButton".into(),
        properties: vec![
            FigmaPropertyDefinition {
                name: "Label#1:2".into(),
                prop_type: "TEXT".into(),
                variant_options: vec![],
            },
            FigmaPropertyDefinition {
                name: "Disabled#1:3".into(),
                prop_type: "BOOLEAN".into(),
                variant_options: vec![],
            },
            FigmaPropertyDefinition {
                name: "Size \"Mode".into(),
                prop_type: "VARIANT".into(),
                variant_options: vec!["Small".into(), "Large \"Quoted".into()],
            },
        ],
    };
    let code = CodeComponentContext {
        component: "Button".into(),
        import: "import { Button } from \"./Button\"".into(),
        source: "src/Button.tsx".into(),
        example: "<Button label={label} disabled={disabled} size={size} />".into(),
        language: Some("jsx".into()),
        props: json!({"label":"string","disabled":"boolean","sizeMode":"enum"}),
    };

    let request: GenerateRequest = serde_json::from_value(json!({
        "figma": figma,
        "code": code,
        "label": "React"
    }))
    .expect("javascript flag defaults to false");
    let generated = generate_template(&GenerateRequest {
        javascript: false,
        ..request
    })
    .expect("generate template");

    assert!(generated.template.contains("getString(\"Label\")"));
    assert!(generated.template.contains("getBoolean(\"Disabled\")"));
    assert!(generated.template.contains("getEnum(\"Size \\\"Mode\""));
    assert!(generated
        .template
        .contains("\"Large \\\"Quoted\": \"large-quoted\""));
    assert!(generated.blocking_issues.is_empty());

    let ambiguous = FigmaComponentContext {
        properties: vec![
            FigmaPropertyDefinition {
                name: "Label#1:2".into(),
                prop_type: "TEXT".into(),
                variant_options: vec![],
            },
            FigmaPropertyDefinition {
                name: "Label#1:3".into(),
                prop_type: "BOOLEAN".into(),
                variant_options: vec![],
            },
        ],
        ..generated.figma_context
    };
    let generated = generate_template(&GenerateRequest {
        figma: ambiguous,
        code: CodeComponentContext {
            component: "Button".into(),
            import: "import { Button } from \"./Button\"".into(),
            source: "src/Button.tsx".into(),
            example: "<Button label={label} />".into(),
            language: Some("jsx".into()),
            props: json!({"label":"string"}),
        },
        label: Some("React".into()),
        javascript: false,
    })
    .expect("ambiguous generation still returns a report");
    assert!(generated
        .blocking_issues
        .iter()
        .any(|issue| issue.contains("ambiguous")));
}
