//! Lightweight scripting boundary for Sivgo plugins.
//!
//! NOTE: despite the historical "Lua" naming, this is NOT a real Lua runtime
//! (there is no `mlua`/`rlua` dependency). It is a small, sandboxed expression
//! evaluator with a Lua-flavoured value model ([`LuaValue`]) that routes Sivgo
//! API calls through explicit permission checks and auditable host commands.
//! Treat it as a restricted DSL, not a general Lua interpreter.

use std::collections::HashMap;

use crate::manifest::{Permission, PluginManifest, PluginPermissionSet};

/// A dynamically typed value in the Lua scripting environment.
#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
    Table(Vec<(String, LuaValue)>),
}

impl std::fmt::Display for LuaValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LuaValue::Nil => write!(f, "nil"),
            LuaValue::Bool(b) => write!(f, "{b}"),
            LuaValue::Number(n) => write!(f, "{n}"),
            LuaValue::String(s) => write!(f, "{s}"),
            LuaValue::Table(_) => write!(f, "table"),
        }
    }
}

/// Host-side command emitted by a Lua script after permission checks pass.
#[derive(Debug, Clone, PartialEq)]
pub enum LuaHostCommand {
    Alert(String),
    SetFill(String),
    CreateRect { x: f64, y: f64, w: f64, h: f64 },
    Fetch(String),
    ReadFile(String),
}

/// Minimal Lua-like engine with a bounded Sivgo host API.
#[derive(Debug, Clone)]
pub struct LuaEngine {
    pub scripts: HashMap<String, String>,
    pub globals: HashMap<String, LuaValue>,
    pub output_log: Vec<String>,
    registered_api_functions: Vec<String>,
    permissions: PluginPermissionSet,
    selection: Vec<String>,
    host_commands: Vec<LuaHostCommand>,
}

impl LuaEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            scripts: HashMap::new(),
            globals: HashMap::new(),
            output_log: Vec::new(),
            registered_api_functions: Vec::new(),
            permissions: PluginPermissionSet {
                read_document: true,
                ..PluginPermissionSet::default()
            },
            selection: Vec::new(),
            host_commands: Vec::new(),
        };

        for api in [
            "sivgo.get_selection",
            "sivgo.set_fill",
            "sivgo.create_rect",
            "sivgo.alert",
            "sivgo.fetch",
            "sivgo.fs_read",
        ] {
            engine.register_api_function(api);
        }

        engine
    }

    pub fn load_script(&mut self, name: &str, source: &str) {
        self.scripts.insert(name.to_string(), source.to_string());
    }

    pub fn execute(&mut self, name: &str) -> Result<LuaValue, String> {
        let source = self
            .scripts
            .get(name)
            .ok_or_else(|| format!("script not found: '{name}'"))?
            .clone();

        let mut last_value = LuaValue::Nil;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }

            if let Some(value) = self.try_execute_api_call(trimmed)? {
                last_value = value;
                continue;
            }

            if let Ok(value) = eval_expression(trimmed, &self.globals) {
                last_value = value;
            }
        }

        Ok(last_value)
    }

    pub fn set_global(&mut self, name: &str, value: LuaValue) {
        self.globals.insert(name.to_string(), value);
    }

    pub fn get_global(&self, name: &str) -> Option<&LuaValue> {
        self.globals.get(name)
    }

    pub fn set_permissions(&mut self, permissions: PluginPermissionSet) {
        self.permissions = permissions;
    }

    pub fn set_permissions_from_manifest(
        &mut self,
        manifest: &PluginManifest,
    ) -> Result<(), String> {
        manifest.validate()?;
        self.permissions = manifest.permission_set();
        Ok(())
    }

    pub fn set_selection(&mut self, selection: Vec<String>) {
        self.selection = selection;
    }

    pub fn register_api_function(&mut self, name: &str) {
        if !self
            .registered_api_functions
            .iter()
            .any(|item| item == name)
        {
            self.registered_api_functions.push(name.to_string());
        }
    }

    pub fn get_output(&self) -> &[String] {
        &self.output_log
    }

    pub fn host_commands(&self) -> &[LuaHostCommand] {
        &self.host_commands
    }

    pub fn clear_host_commands(&mut self) {
        self.host_commands.clear();
    }

    pub fn eval(&self, expr: &str) -> Result<LuaValue, String> {
        eval_expression(expr, &self.globals)
    }

    fn try_execute_api_call(&mut self, line: &str) -> Result<Option<LuaValue>, String> {
        if line.contains("sivgo.get_selection()") {
            self.require_api("sivgo.get_selection")?;
            self.require_permission(Permission::ReadDocument, "sivgo.get_selection")?;
            return Ok(Some(LuaValue::Table(
                self.selection
                    .iter()
                    .enumerate()
                    .map(|(idx, id)| ((idx + 1).to_string(), LuaValue::String(id.clone())))
                    .collect(),
            )));
        }

        if line.contains("sivgo.set_fill(") {
            self.require_api("sivgo.set_fill")?;
            self.require_permission(Permission::ModifyDocument, "sivgo.set_fill")?;
            let color = extract_single_arg(line, "sivgo.set_fill")
                .ok_or_else(|| "sivgo.set_fill requires one argument".to_string())?
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            self.host_commands.push(LuaHostCommand::SetFill(color));
            return Ok(Some(LuaValue::Nil));
        }

        if line.contains("sivgo.create_rect(") {
            self.require_api("sivgo.create_rect")?;
            self.require_permission(Permission::ModifyDocument, "sivgo.create_rect")?;
            let args = extract_args_str(line, "sivgo.create_rect")
                .ok_or_else(|| "sivgo.create_rect requires arguments".to_string())?;
            let parts: Vec<&str> = args.split(',').map(|part| part.trim()).collect();
            if parts.len() != 4 {
                return Err("sivgo.create_rect requires four numeric arguments".to_string());
            }
            let x = parse_number_arg(parts[0], "x")?;
            let y = parse_number_arg(parts[1], "y")?;
            let w = parse_number_arg(parts[2], "w")?;
            let h = parse_number_arg(parts[3], "h")?;
            self.host_commands
                .push(LuaHostCommand::CreateRect { x, y, w, h });
            return Ok(Some(LuaValue::Table(vec![
                ("x".to_string(), LuaValue::Number(x)),
                ("y".to_string(), LuaValue::Number(y)),
                ("w".to_string(), LuaValue::Number(w)),
                ("h".to_string(), LuaValue::Number(h)),
            ])));
        }

        if line.contains("sivgo.alert(") {
            self.require_api("sivgo.alert")?;
            let msg = extract_single_arg(line, "sivgo.alert")
                .ok_or_else(|| "sivgo.alert requires one argument".to_string())?
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            self.output_log.push(format!("alert: {msg}"));
            self.host_commands.push(LuaHostCommand::Alert(msg));
            return Ok(Some(LuaValue::Nil));
        }

        if line.contains("sivgo.fetch(") {
            self.require_api("sivgo.fetch")?;
            self.require_permission(Permission::NetworkAccess, "sivgo.fetch")?;
            let url = extract_single_arg(line, "sivgo.fetch")
                .ok_or_else(|| "sivgo.fetch requires one argument".to_string())?
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            self.host_commands.push(LuaHostCommand::Fetch(url));
            return Ok(Some(LuaValue::Nil));
        }

        if line.contains("sivgo.fs_read(") {
            self.require_api("sivgo.fs_read")?;
            self.require_permission(Permission::AccessFileSystem, "sivgo.fs_read")?;
            let path = extract_single_arg(line, "sivgo.fs_read")
                .ok_or_else(|| "sivgo.fs_read requires one argument".to_string())?
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            self.host_commands.push(LuaHostCommand::ReadFile(path));
            return Ok(Some(LuaValue::Nil));
        }

        Ok(None)
    }

    fn require_api(&self, name: &str) -> Result<(), String> {
        if self
            .registered_api_functions
            .iter()
            .any(|item| item == name)
        {
            Ok(())
        } else {
            Err(format!("Lua API function is not registered: {name}"))
        }
    }

    fn require_permission(&self, permission: Permission, api: &str) -> Result<(), String> {
        if self.permissions.allows(&permission) {
            Ok(())
        } else {
            Err(format!("permission denied: {api} requires {permission:?}"))
        }
    }
}

impl Default for LuaEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_single_arg<'a>(line: &'a str, func_prefix: &str) -> Option<&'a str> {
    let start = line.find(&format!("{func_prefix}("))? + func_prefix.len() + 1;
    let end = line[start..].find(')')? + start;
    let arg = line[start..end].trim();
    (!arg.is_empty()).then_some(arg)
}

fn extract_args_str<'a>(line: &'a str, func_prefix: &str) -> Option<&'a str> {
    let start = line.find(&format!("{func_prefix}("))? + func_prefix.len() + 1;
    let end = line[start..].find(')')? + start;
    let args = line[start..end].trim();
    (!args.is_empty()).then_some(args)
}

fn parse_number_arg(value: &str, name: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .map_err(|err| format!("invalid numeric argument '{name}': {err}"))
}

pub fn eval_expression(
    expr: &str,
    globals: &HashMap<String, LuaValue>,
) -> Result<LuaValue, String> {
    let tokens = tokenize(expr)?;
    if tokens.is_empty() {
        return Err("empty expression".to_string());
    }
    let mut pos = 0;
    let result = parse_add_sub(&tokens, &mut pos, globals)?;
    if pos < tokens.len() {
        return Err(format!("unexpected token: {:?}", tokens[pos]));
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Str(String),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        match ch {
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
                continue;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
                continue;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
                continue;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
                continue;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
                continue;
            }
            '-' => {
                let is_unary = tokens.is_empty()
                    || matches!(
                        tokens.last(),
                        Some(
                            Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::LParen
                        )
                    );
                if is_unary {
                    i += 1;
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        let start = i - 1;
                        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                            i += 1;
                        }
                        let s: String = chars[start..i].iter().collect();
                        tokens.push(Token::Number(
                            s.parse().map_err(|err| format!("bad number: {err}"))?,
                        ));
                    } else {
                        tokens.push(Token::Number(-1.0));
                        tokens.push(Token::Star);
                    }
                    continue;
                }
                tokens.push(Token::Minus);
                i += 1;
                continue;
            }
            _ => {}
        }

        if ch.is_ascii_digit() || ch == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            tokens.push(Token::Number(
                s.parse().map_err(|err| format!("bad number: {err}"))?,
            ));
            continue;
        }

        if ch == '"' || ch == '\'' {
            let quote = ch;
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != quote {
                i += 1;
            }
            if i >= chars.len() {
                return Err("unterminated string literal".to_string());
            }
            tokens.push(Token::Str(chars[start..i].iter().collect()));
            i += 1;
            continue;
        }

        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(Token::Ident(chars[start..i].iter().collect()));
            continue;
        }

        return Err(format!("unexpected character: '{ch}'"));
    }

    Ok(tokens)
}

fn parse_add_sub(
    tokens: &[Token],
    pos: &mut usize,
    globals: &HashMap<String, LuaValue>,
) -> Result<LuaValue, String> {
    let mut left = parse_mul_div(tokens, pos, globals)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                let right = parse_mul_div(tokens, pos, globals)?;
                left = apply_binary_op(&left, &right, '+')?;
            }
            Token::Minus => {
                *pos += 1;
                let right = parse_mul_div(tokens, pos, globals)?;
                left = apply_binary_op(&left, &right, '-')?;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_mul_div(
    tokens: &[Token],
    pos: &mut usize,
    globals: &HashMap<String, LuaValue>,
) -> Result<LuaValue, String> {
    let mut left = parse_atom(tokens, pos, globals)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Star => {
                *pos += 1;
                let right = parse_atom(tokens, pos, globals)?;
                left = apply_binary_op(&left, &right, '*')?;
            }
            Token::Slash => {
                *pos += 1;
                let right = parse_atom(tokens, pos, globals)?;
                left = apply_binary_op(&left, &right, '/')?;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_atom(
    tokens: &[Token],
    pos: &mut usize,
    globals: &HashMap<String, LuaValue>,
) -> Result<LuaValue, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".to_string());
    }

    match &tokens[*pos] {
        Token::Number(n) => {
            *pos += 1;
            Ok(LuaValue::Number(*n))
        }
        Token::Str(s) => {
            *pos += 1;
            Ok(LuaValue::String(s.clone()))
        }
        Token::Ident(name) => {
            *pos += 1;
            match name.as_str() {
                "nil" => Ok(LuaValue::Nil),
                "true" => Ok(LuaValue::Bool(true)),
                "false" => Ok(LuaValue::Bool(false)),
                _ => globals
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("undefined variable: '{name}'")),
            }
        }
        Token::LParen => {
            *pos += 1;
            let inner = parse_add_sub(tokens, pos, globals)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                return Err("expected ')'".to_string());
            }
            *pos += 1;
            Ok(inner)
        }
        other => Err(format!("unexpected token: {other:?}")),
    }
}

fn apply_binary_op(left: &LuaValue, right: &LuaValue, op: char) -> Result<LuaValue, String> {
    match (left, right, op) {
        (LuaValue::Number(a), LuaValue::Number(b), '+') => Ok(LuaValue::Number(a + b)),
        (LuaValue::Number(a), LuaValue::Number(b), '-') => Ok(LuaValue::Number(a - b)),
        (LuaValue::Number(a), LuaValue::Number(b), '*') => Ok(LuaValue::Number(a * b)),
        (LuaValue::Number(a), LuaValue::Number(b), '/') => {
            if *b == 0.0 {
                Err("division by zero".to_string())
            } else {
                Ok(LuaValue::Number(a / b))
            }
        }
        (LuaValue::String(a), LuaValue::String(b), '+') => Ok(LuaValue::String(format!("{a}{b}"))),
        _ => Err(format!("unsupported operation: {left:?} {op} {right:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modify_permissions() -> PluginPermissionSet {
        PluginPermissionSet {
            read_document: true,
            modify_document: true,
            ..PluginPermissionSet::default()
        }
    }

    #[test]
    fn eval_basic_arithmetic_and_globals() {
        let mut globals = HashMap::new();
        globals.insert("x".to_string(), LuaValue::Number(10.0));
        assert_eq!(
            eval_expression("2 + x * 4", &globals).unwrap(),
            LuaValue::Number(42.0)
        );
        assert_eq!(
            eval_expression("\"hello\" + \" world\"", &globals).unwrap(),
            LuaValue::String("hello world".to_string())
        );
        assert!(eval_expression("unknown", &globals).is_err());
    }

    #[test]
    fn engine_loads_scripts_and_records_alerts() {
        let mut engine = LuaEngine::new();
        engine.load_script("msg", "sivgo.alert(\"Hello\")");
        assert_eq!(engine.execute("msg").unwrap(), LuaValue::Nil);
        assert_eq!(engine.get_output(), &["alert: Hello"]);
        assert_eq!(
            engine.host_commands(),
            &[LuaHostCommand::Alert("Hello".to_string())]
        );
    }

    #[test]
    fn get_selection_uses_host_selection_not_hardcoded_values() {
        let mut engine = LuaEngine::new();
        engine.set_selection(vec!["node-a".to_string(), "node-b".to_string()]);
        engine.load_script("sel", "sivgo.get_selection()");
        let result = engine.execute("sel").unwrap();
        assert_eq!(
            result,
            LuaValue::Table(vec![
                ("1".to_string(), LuaValue::String("node-a".to_string())),
                ("2".to_string(), LuaValue::String("node-b".to_string()))
            ])
        );
    }

    #[test]
    fn modify_apis_require_modify_document_permission() {
        let mut engine = LuaEngine::new();
        engine.load_script("fill", "sivgo.set_fill(\"#ff0000\")");
        assert!(engine
            .execute("fill")
            .unwrap_err()
            .contains("permission denied"));

        engine.set_permissions(modify_permissions());
        assert_eq!(engine.execute("fill").unwrap(), LuaValue::Nil);
        assert_eq!(
            engine.host_commands(),
            &[LuaHostCommand::SetFill("#ff0000".to_string())]
        );
    }

    #[test]
    fn create_rect_records_auditable_host_command() {
        let mut engine = LuaEngine::new();
        engine.set_permissions(modify_permissions());
        engine.load_script("rect", "sivgo.create_rect(10, 20, 100, 50)");
        let result = engine.execute("rect").unwrap();
        assert!(matches!(result, LuaValue::Table(_)));
        assert_eq!(
            engine.host_commands(),
            &[LuaHostCommand::CreateRect {
                x: 10.0,
                y: 20.0,
                w: 100.0,
                h: 50.0
            }]
        );
    }

    #[test]
    fn network_and_filesystem_apis_are_permission_gated() {
        let mut engine = LuaEngine::new();
        engine.load_script("net", "sivgo.fetch(\"https://example.test\")");
        assert!(engine.execute("net").unwrap_err().contains("NetworkAccess"));

        engine.set_permissions(PluginPermissionSet {
            read_document: true,
            network_access: true,
            access_filesystem: true,
            ..PluginPermissionSet::default()
        });
        engine.execute("net").unwrap();
        engine.load_script("fs", "sivgo.fs_read(\"/tmp/a.svg\")");
        engine.execute("fs").unwrap();
        assert_eq!(
            engine.host_commands(),
            &[
                LuaHostCommand::Fetch("https://example.test".to_string()),
                LuaHostCommand::ReadFile("/tmp/a.svg".to_string())
            ]
        );
    }

    #[test]
    fn manifest_permissions_configure_lua_boundary() {
        let manifest = PluginManifest {
            id: "ext.lua".to_string(),
            name: "Lua".to_string(),
            version: "1.0.0".to_string(),
            author: "Tester".to_string(),
            description: "Lua extension".to_string(),
            permissions: vec![Permission::ModifyDocument],
        };
        let mut engine = LuaEngine::new();
        engine.set_permissions_from_manifest(&manifest).unwrap();
        engine.load_script("fill", "sivgo.set_fill(\"blue\")");
        assert!(engine.execute("fill").is_ok());
    }
}
