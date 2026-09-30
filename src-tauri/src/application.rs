//! Declarative local mini-apps. Generated content never becomes executable code.
use crate::artifact::ArtifactKind;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
    pub title: String,
    pub description: String,
    pub fields: Vec<Field>,
    pub outputs: Vec<Output>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub id: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: FieldType,
    pub default: Value,
    pub options: Option<Vec<String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Number,
    Text,
    Select,
    Toggle,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub label: String,
    pub expression: Expression,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum Expression {
    Constant {
        value: f64,
    },
    Input {
        id: String,
    },
    Add {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Subtract {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Multiply {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Divide {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Min {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Max {
        left: Box<Expression>,
        right: Box<Expression>,
    },
}
fn bounded(s: &str, max: usize) -> bool {
    s.len() <= max && !s.contains('\0')
}
fn number(n: f64) -> bool {
    n.is_finite() && n.abs() <= 1e12
}
fn field_value(f: &Field, v: &Value) -> bool {
    match f.kind {
        FieldType::Number => v.as_f64().is_some_and(number),
        FieldType::Toggle => v.is_boolean(),
        FieldType::Text => v.as_str().is_some_and(|s| bounded(s, 2000)),
        FieldType::Select => v
            .as_str()
            .is_some_and(|s| f.options.as_ref().is_some_and(|o| o.iter().any(|x| x == s))),
    }
}
fn expression(e: &Expression, app: &Application, depth: usize, nodes: &mut usize) -> bool {
    *nodes += 1;
    if depth > 12 || *nodes > 256 {
        return false;
    }
    match e {
        Expression::Constant { value } => number(*value),
        Expression::Input { id } => app
            .fields
            .iter()
            .any(|f| f.id == *id && matches!(f.kind, FieldType::Number | FieldType::Toggle)),
        Expression::Add { left, right }
        | Expression::Subtract { left, right }
        | Expression::Multiply { left, right }
        | Expression::Divide { left, right }
        | Expression::Min { left, right }
        | Expression::Max { left, right } => {
            expression(left, app, depth + 1, nodes) && expression(right, app, depth + 1, nodes)
        }
    }
}
pub fn parse(content: &Value) -> Result<Application, String> {
    // Bound nesting before recursive serde conversion or serialization.
    let mut pending = vec![(content, 0_usize)];
    let mut count = 0;
    while let Some((value, depth)) = pending.pop() {
        count += 1;
        if depth > 18 || count > 4096 {
            return Err("Application content is too complex.".into());
        }
        match value {
            Value::Array(items) => pending.extend(items.iter().map(|v| (v, depth + 1))),
            Value::Object(items) => pending.extend(items.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    if serde_json::to_vec(content)
        .map_err(|_| "Application format is invalid.")?
        .len()
        > 64_000
    {
        return Err("Application content is too large.".into());
    }
    let app: Application =
        serde_json::from_value(content.clone()).map_err(|_| "Application format is invalid.")?;
    let mut ids = HashSet::new();
    let mut nodes = 0;
    if !bounded(&app.title, 200)
        || app.title.trim().is_empty()
        || !bounded(&app.description, 2000)
        || app.fields.is_empty()
        || app.fields.len() > 30
        || app.outputs.len() > 20
        || app.fields.iter().any(|f| {
            f.id.is_empty()
                || f.id.len() > 40
                || !f.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || !ids.insert(f.id.clone())
                || matches!(f.id.as_str(), "__proto__" | "constructor" | "prototype")
                || !bounded(&f.label, 200)
                || f.label.trim().is_empty()
                || !field_value(f, &f.default)
                || match f.kind {
                    FieldType::Select => f.options.as_ref().is_none_or(|o| {
                        o.is_empty() || o.len() > 30 || o.iter().any(|s| !bounded(s, 200))
                    }),
                    _ => f.options.is_some(),
                }
        })
        || app.outputs.iter().any(|o| {
            !bounded(&o.label, 200)
                || o.label.trim().is_empty()
                || !expression(&o.expression, &app, 0, &mut nodes)
        })
    {
        return Err("Application content is invalid or too large.".into());
    }
    Ok(app)
}
pub fn validate(kind: ArtifactKind, content: &Value) -> Result<(), String> {
    if kind != ArtifactKind::Application {
        return Err("Wrong application output type.".into());
    }
    parse(content).map(|_| ())
}
pub fn values(app: &Application, input: &Value) -> Result<Value, String> {
    let map = input.as_object().ok_or("Application values are invalid.")?;
    if map.len() != app.fields.len()
        || app
            .fields
            .iter()
            .any(|f| !map.get(&f.id).is_some_and(|v| field_value(f, v)))
    {
        return Err("Application values are invalid.".into());
    }
    Ok(input.clone())
}
pub fn defaults(app: &Application) -> Value {
    Value::Object(
        app.fields
            .iter()
            .map(|f| (f.id.clone(), f.default.clone()))
            .collect::<Map<_, _>>(),
    )
}
pub fn evaluate(e: &Expression, values: &Value) -> Result<f64, String> {
    let result = match e {
        Expression::Constant { value } => *value,
        Expression::Input { id } => {
            let v = &values[id];
            v.as_f64()
                .or_else(|| v.as_bool().map(|b| if b { 1.0 } else { 0.0 }))
                .ok_or("Input is unavailable.")?
        }
        Expression::Add { left, right } => evaluate(left, values)? + evaluate(right, values)?,
        Expression::Subtract { left, right } => evaluate(left, values)? - evaluate(right, values)?,
        Expression::Multiply { left, right } => evaluate(left, values)? * evaluate(right, values)?,
        Expression::Divide { left, right } => evaluate(left, values)? / evaluate(right, values)?,
        Expression::Min { left, right } => evaluate(left, values)?.min(evaluate(right, values)?),
        Expression::Max { left, right } => evaluate(left, values)?.max(evaluate(right, values)?),
    };
    if number(result) {
        Ok(result)
    } else {
        Err("Calculation is undefined or exceeds the supported range.".into())
    }
}
pub fn export_html(content: &Value) -> Result<String, String> {
    let app = parse(content)?;
    let schema = serde_json::to_string(&app)
        .map_err(|_| "Application export is unreadable.")?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let runtime = r#"'use strict';const app=JSON.parse(document.getElementById('schema').textContent),values=Object.create(null),form=document.getElementById('form'),results=document.getElementById('results');document.getElementById('title').textContent=app.title;document.getElementById('description').textContent=app.description;function calc(e){let n;if(e.op==='constant')n=e.value;else if(e.op==='input')n=typeof values[e.id]==='boolean'?Number(values[e.id]):values[e.id];else{const a=calc(e.left),b=calc(e.right);switch(e.op){case'add':n=a+b;break;case'subtract':n=a-b;break;case'multiply':n=a*b;break;case'divide':n=a/b;break;case'min':n=Math.min(a,b);break;case'max':n=Math.max(a,b);break;default:throw Error();}}if(!Number.isFinite(n)||Math.abs(n)>1e12)throw Error();return n;}function render(){results.replaceChildren();for(const o of app.outputs){const p=document.createElement('p');try{p.textContent=o.label+': '+calc(o.expression);}catch{p.textContent=o.label+': Calculation unavailable';}results.append(p);}}for(const f of app.fields){values[f.id]=f.default;const label=document.createElement('label');label.textContent=f.label;const input=document.createElement(f.type==='select'?'select':'input');if(f.type==='select'){for(const option of f.options){const o=document.createElement('option');o.value=option;o.textContent=option;input.append(o);}}else input.type=f.type==='toggle'?'checkbox':f.type==='number'?'number':'text';if(f.type==='toggle')input.checked=f.default;else input.value=f.default;if(f.type==='text')input.maxLength=2000;if(f.type==='number'){input.min=-1e12;input.max=1e12;input.step='any';}input.addEventListener('input',()=>{values[f.id]=f.type==='toggle'?input.checked:f.type==='number'?(input.value.trim()===''?NaN:Number(input.value)):input.value;render();});label.append(input);form.append(label);}render();"#;
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let hash = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(runtime.as_bytes()));
    Ok(format!("<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'sha256-{hash}'; style-src 'unsafe-inline'; connect-src 'none'; form-action 'none'; base-uri 'none'\"><title>Bench Application</title><style>body{{font:16px system-ui;max-width:720px;margin:48px auto;padding:20px}}label{{display:block;margin:20px 0}}input,select{{display:block;font:inherit;margin-top:8px;padding:8px}}input[type=checkbox]{{display:inline;margin-left:12px}}</style></head><body><h1 id=\"title\"></h1><p id=\"description\"></p><div id=\"form\"></div><div id=\"results\" aria-live=\"polite\"></div><script type=\"application/json\" id=\"schema\">{schema}</script><script>{runtime}</script></body></html>"))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Value {
        serde_json::json!({"title":"Calculator","description":"Local only","fields":[{"id":"amount","label":"Amount","type":"number","default":2}],"outputs":[{"label":"Double","expression":{"op":"multiply","left":{"op":"input","id":"amount"},"right":{"op":"constant","value":2}}}]})
    }
    #[test]
    fn calculations_and_typed_values() {
        let app = parse(&sample()).unwrap();
        assert_eq!(
            evaluate(&app.outputs[0].expression, &defaults(&app)).unwrap(),
            4.0
        );
        assert!(values(&app, &serde_json::json!({"amount":"2"})).is_err());
        assert!(values(&app, &serde_json::json!({"amount":2,"extra":1})).is_err());
        let division = Expression::Divide {
            left: Box::new(Expression::Constant { value: 1.0 }),
            right: Box::new(Expression::Constant { value: 0.0 }),
        };
        assert!(evaluate(&division, &defaults(&app)).is_err());
    }
    #[test]
    fn hostile_schema_and_bounds() {
        let mut s = sample();
        s["script"] = Value::String("evil".into());
        assert!(parse(&s).is_err());
        let mut s = sample();
        s["outputs"][0]["expression"] = serde_json::json!({"op":"eval","code":"process"});
        assert!(parse(&s).is_err());
        let mut s = sample();
        s["fields"][0]["default"] = serde_json::json!(1e13);
        assert!(parse(&s).is_err());
        let mut s = sample();
        s["outputs"][0]["expression"] = serde_json::json!({"op":"input","id":"missing"});
        assert!(parse(&s).is_err());
        let mut s = sample();
        let mut e = serde_json::json!({"op":"constant","value":1});
        for _ in 0..14 {
            e = serde_json::json!({"op":"add","left":e,"right":{"op":"constant","value":1}});
        }
        s["outputs"][0]["expression"] = e;
        assert!(parse(&s).is_err());
    }
    #[test]
    fn export_escapes_content_and_allows_only_trusted_runtime() {
        let mut s = sample();
        s["title"] = serde_json::json!("</script><script>alert(1)</script>");
        let html = export_html(&s).unwrap();
        assert!(!html.contains("</script><script>alert"));
        assert!(html.contains("script-src 'sha256-"));
        assert!(html.contains("connect-src 'none'"));
        assert!(!html.contains("innerHTML"));
        assert!(html.contains("\\u003c/script"));
    }
}
