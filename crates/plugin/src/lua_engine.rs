//! Lightweight Lua scripting abstraction for Sivgo plugins.
//!
//! This module provides a minimal Lua-like expression evaluator that does
//! **not** depend on a full Lua VM (no `mlua` / `rlua` crate required).
//! It supports:
//!
//! - Numeric and string literals
//! - The four basic arithmetic operators (`+`, `-`, `*`, `/`)
//! - Parenthesised sub-expressions
//! - Variable lookup from a global table
//! - Named script storage and execution
//! - Built-in Sivgo API function stubs (`sivgo.get_selection`, etc.)
//!
//! This is intentionally limited; a production build would swap in a real
//! Lua binding behind the same public API.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// LuaValue
// ---------------------------------------------------------------------------

/// A dynamically-typed value in the Lua scripting environment.
#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
    /// Association list representing a Lua table.
    /// Keys are strings for ergonomic Rust usage.
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

// ---------------------------------------------------------------------------
// LuaEngine
// ---------------------------------------------------------------------------

/// A minimal Lua scripting engine that stores globals, named scripts,
/// registered API functions, and accumulated output.
#[derive(Debug, Clone)]
pub struct LuaEngine {
    /// Named scripts that have been loaded (name -> source text).
    pub scripts: HashMap<String, String>,
    /// Named global variables visible to scripts.
    pub globals: HashMap<String, LuaValue>,
    /// Output log from `sivgo.alert()` and similar calls.
    pub output_log: Vec<String>,
    /// Registered API function names (e.g. `"sivgo.get_selection"`).
    registered_api_functions: Vec<String>,
}

impl LuaEngine {
    pub fn new() -> Self {
        Self {
            scripts: HashMap::new(),
            globals: HashMap::new(),
            output_log: Vec::new(),
            registered_api_functions: Vec::new(),
        }
    }

    /// Load a named script from source text.
    pub fn load_script(&mut self, name: &str, source: &str) {
        self.scripts.insert(name.to_string(), source.to_string());
    }

    /// Execute a previously loaded script by name.
    ///
    /// The stub implementation parses the script source for built-in API
    /// calls (`sivgo.get_selection()`, `sivgo.set_fill(color)`,
    /// `sivgo.create_rect(x,y,w,h)`, `sivgo.alert(msg)`) and simple
    /// expressions.  Returns the result of the last evaluated expression,
    /// or `LuaValue::Nil` for API-only scripts.
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

            // Check for built-in API calls.
            if let Some(result) = self.try_execute_api_call(trimmed) {
                last_value = result;
                continue;
            }

            // Fall back to expression evaluation.
            match eval_expression(trimmed, &self.globals) {
                Ok(val) => last_value = val,
                Err(_) => {
                    // Ignore lines that are not valid expressions (stubs
                    // for assignment, control flow, etc.).
                    continue;
                }
            }
        }

        Ok(last_value)
    }

    /// Set a global variable.
    pub fn set_global(&mut self, name: &str, value: LuaValue) {
        self.globals.insert(name.to_string(), value);
    }

    /// Get a reference to a global variable, if it exists.
    pub fn get_global(&self, name: &str) -> Option<&LuaValue> {
        self.globals.get(name)
    }

    /// Register a named API function that scripts may call.
    pub fn register_api_function(&mut self, name: &str) {
        if !self.registered_api_functions.contains(&name.to_string()) {
            self.registered_api_functions.push(name.to_string());
        }
    }

    /// Return the accumulated output log.
    pub fn get_output(&self) -> &[String] {
        &self.output_log
    }

    /// Evaluate a simple expression using the engine's globals.
    pub fn eval(&self, expr: &str) -> Result<LuaValue, String> {
        eval_expression(expr, &self.globals)
    }

    // -- Private helpers ----------------------------------------------------

    /// Try to interpret `line` as a built-in API call.
    /// Returns `Some(value)` if the line matched an API call, `None`
    /// otherwise.
    fn try_execute_api_call(&mut self, line: &str) -> Option<LuaValue> {
        // sivgo.get_selection()
        if line.contains("sivgo.get_selection()") {
            let result = LuaValue::Table(vec![
                ("1".to_string(), LuaValue::String("rect1".to_string())),
                ("2".to_string(), LuaValue::String("circle1".to_string())),
            ]);
            return Some(result);
        }

        // sivgo.set_fill(color)
        if line.contains("sivgo.set_fill(") {
            if let Some(arg) = extract_single_arg(line, "sivgo.set_fill") {
                self.output_log
                    .push(format!("set_fill: {}", arg.trim_matches('"')));
            }
            return Some(LuaValue::Nil);
        }

        // sivgo.create_rect(x, y, w, h)
        if line.contains("sivgo.create_rect(") {
            if let Some(args_str) = extract_args_str(line, "sivgo.create_rect") {
                let parts: Vec<&str> = args_str.split(',').map(|s| s.trim()).collect();
                if parts.len() == 4 {
                    let table = parts
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let key = ["x", "y", "w", "h"][i].to_string();
                            let val = s
                                .parse::<f64>()
                                .map(LuaValue::Number)
                                .unwrap_or(LuaValue::String(s.to_string()));
                            (key, val)
                        })
                        .collect();
                    self.output_log.push(format!("create_rect: {args_str}"));
                    return Some(LuaValue::Table(table));
                }
            }
            self.output_log
                .push("create_rect: (invalid args)".to_string());
            return Some(LuaValue::Nil);
        }

        // sivgo.alert(msg)
        if line.contains("sivgo.alert(") {
            if let Some(arg) = extract_single_arg(line, "sivgo.alert") {
                let msg = arg.trim_matches('"').trim_matches('\'').to_string();
                self.output_log.push(format!("alert: {msg}"));
            }
            return Some(LuaValue::Nil);
        }

        None
    }
}

impl Default for LuaEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Argument extraction helpers
// ---------------------------------------------------------------------------

/// Extract a single argument from a call like `sivgo.func_name(arg)`.
fn extract_single_arg<'a>(line: &'a str, func_prefix: &str) -> Option<&'a str> {
    let start = line.find(&format!("{func_prefix}("))? + func_prefix.len() + 1;
    let end = line[start..].find(')')? + start;
    let arg = line[start..end].trim();
    if arg.is_empty() {
        None
    } else {
        Some(arg)
    }
}

/// Extract the raw arguments string from a call like
/// `sivgo.func_name(a, b, c)`.
fn extract_args_str<'a>(line: &'a str, func_prefix: &str) -> Option<&'a str> {
    let start = line.find(&format!("{func_prefix}("))? + func_prefix.len() + 1;
    let end = line[start..].find(')')? + start;
    let args = line[start..end].trim();
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

// ---------------------------------------------------------------------------
// Public expression evaluator
// ---------------------------------------------------------------------------

/// Evaluate a simple expression string and return the resulting
/// [`LuaValue`].
///
/// Supported syntax:
/// - Decimal numbers (including negatives and floats): `42`, `-3.14`
/// - Quoted strings: `"hello"`, `'world'`
/// - Variables looked up from `globals`
/// - Binary operators: `+`, `-`, `*`, `/`
/// - Parentheses for grouping
/// - Boolean literals: `true`, `false`
/// - `nil`
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

// ---------------------------------------------------------------------------
// Tokeniser
// ---------------------------------------------------------------------------

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

        // Whitespace
        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        // Operators / parens
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
                // Unary minus if at start, after operator, or after '('
                let is_unary = tokens.is_empty()
                    || matches!(
                        tokens.last(),
                        Some(
                            Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::LParen
                        )
                    );
                if is_unary {
                    // Consume digits as a negative number
                    i += 1;
                    if i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        let start = i - 1; // include the '-'
                        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                            i += 1;
                        }
                        let s: String = chars[start..i].iter().collect();
                        let n: f64 = s.parse().map_err(|e| format!("bad number: {e}"))?;
                        tokens.push(Token::Number(n));
                    } else {
                        // Unary minus before an identifier or paren -- emit
                        // as `Number(-1) * ...` so the parser handles it.
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

        // Number
        if ch.is_ascii_digit() || ch == '.' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let n: f64 = s.parse().map_err(|e| format!("bad number: {e}"))?;
            tokens.push(Token::Number(n));
            continue;
        }

        // String literals (single or double quotes)
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
            let s: String = chars[start..i].iter().collect();
            tokens.push(Token::Str(s));
            i += 1; // skip closing quote
            continue;
        }

        // Identifier (variable, true, false, nil)
        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            tokens.push(Token::Ident(ident));
            continue;
        }

        return Err(format!("unexpected character: '{ch}'"));
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// Recursive-descent parser  (precedence: +/- < */div)
// ---------------------------------------------------------------------------

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
            let v = LuaValue::Number(*n);
            *pos += 1;
            Ok(v)
        }
        Token::Str(s) => {
            let v = LuaValue::String(s.clone());
            *pos += 1;
            Ok(v)
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
            *pos += 1; // consume '('
            let inner = parse_add_sub(tokens, pos, globals)?;
            if *pos >= tokens.len() || tokens[*pos] != Token::RParen {
                return Err("expected ')'".to_string());
            }
            *pos += 1; // consume ')'
            Ok(inner)
        }
        other => Err(format!("unexpected token: {other:?}")),
    }
}

fn apply_binary_op(left: &LuaValue, right: &LuaValue, op: char) -> Result<LuaValue, String> {
    match (left, right, op) {
        // Number arithmetic
        (LuaValue::Number(a), LuaValue::Number(b), '+') => Ok(LuaValue::Number(a + b)),
        (LuaValue::Number(a), LuaValue::Number(b), '-') => Ok(LuaValue::Number(a - b)),
        (LuaValue::Number(a), LuaValue::Number(b), '*') => Ok(LuaValue::Number(a * b)),
        (LuaValue::Number(a), LuaValue::Number(b), '/') => {
            if *b == 0.0 {
                return Err("division by zero".to_string());
            }
            Ok(LuaValue::Number(a / b))
        }
        // String concatenation with `+`
        (LuaValue::String(a), LuaValue::String(b), '+') => Ok(LuaValue::String(format!("{a}{b}"))),
        _ => Err(format!("unsupported operation: {left:?} {op} {right:?}")),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_basic_arithmetic() {
        let globals = HashMap::new();

        assert_eq!(
            eval_expression("2 + 3", &globals).unwrap(),
            LuaValue::Number(5.0),
        );
        assert_eq!(
            eval_expression("10 - 4", &globals).unwrap(),
            LuaValue::Number(6.0),
        );
        assert_eq!(
            eval_expression("3 * 7", &globals).unwrap(),
            LuaValue::Number(21.0),
        );
        assert_eq!(
            eval_expression("20 / 4", &globals).unwrap(),
            LuaValue::Number(5.0),
        );
    }

    #[test]
    fn eval_operator_precedence() {
        let globals = HashMap::new();

        // 2 + 3 * 4  =>  2 + 12  =>  14
        assert_eq!(
            eval_expression("2 + 3 * 4", &globals).unwrap(),
            LuaValue::Number(14.0),
        );
        // (2 + 3) * 4  =>  20
        assert_eq!(
            eval_expression("(2 + 3) * 4", &globals).unwrap(),
            LuaValue::Number(20.0),
        );
    }

    #[test]
    fn eval_variable_lookup() {
        let mut globals = HashMap::new();
        globals.insert("x".to_string(), LuaValue::Number(10.0));
        globals.insert("y".to_string(), LuaValue::Number(3.0));

        assert_eq!(
            eval_expression("x + y * 2", &globals).unwrap(),
            LuaValue::Number(16.0),
        );
    }

    #[test]
    fn eval_string_concat() {
        let globals = HashMap::new();
        assert_eq!(
            eval_expression("\"hello\" + \" world\"", &globals).unwrap(),
            LuaValue::String("hello world".to_string()),
        );
    }

    #[test]
    fn eval_literals() {
        let globals = HashMap::new();
        assert_eq!(eval_expression("nil", &globals).unwrap(), LuaValue::Nil);
        assert_eq!(
            eval_expression("true", &globals).unwrap(),
            LuaValue::Bool(true),
        );
        assert_eq!(
            eval_expression("false", &globals).unwrap(),
            LuaValue::Bool(false),
        );
    }

    #[test]
    fn eval_undefined_variable_is_error() {
        let globals = HashMap::new();
        assert!(eval_expression("unknown_var", &globals).is_err());
    }

    #[test]
    fn engine_set_global_and_eval() {
        let mut engine = LuaEngine::new();
        engine.set_global("width", LuaValue::Number(800.0));
        engine.set_global("margin", LuaValue::Number(20.0));

        let result = engine.eval("width - margin * 2").unwrap();
        assert_eq!(result, LuaValue::Number(760.0));
    }

    // -- New LuaEngine tests --

    #[test]
    fn engine_load_and_execute_script() {
        let mut engine = LuaEngine::new();
        engine.load_script("test", "2 + 3");
        let result = engine.execute("test").unwrap();
        assert_eq!(result, LuaValue::Number(5.0));
    }

    #[test]
    fn engine_execute_missing_script_fails() {
        let mut engine = LuaEngine::new();
        assert!(engine.execute("nonexistent").is_err());
    }

    #[test]
    fn engine_get_global() {
        let mut engine = LuaEngine::new();
        assert!(engine.get_global("x").is_none());

        engine.set_global("x", LuaValue::Number(42.0));
        assert_eq!(engine.get_global("x"), Some(&LuaValue::Number(42.0)));
    }

    #[test]
    fn engine_register_api_function() {
        let mut engine = LuaEngine::new();
        engine.register_api_function("sivgo.get_selection");
        engine.register_api_function("sivgo.set_fill");
        // Duplicate registration is a no-op.
        engine.register_api_function("sivgo.get_selection");
        assert_eq!(engine.registered_api_functions.len(), 2);
    }

    #[test]
    fn engine_sivgo_get_selection() {
        let mut engine = LuaEngine::new();
        engine.load_script("sel", "sivgo.get_selection()");
        let result = engine.execute("sel").unwrap();
        match result {
            LuaValue::Table(entries) => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].1, LuaValue::String("rect1".to_string()));
                assert_eq!(entries[1].1, LuaValue::String("circle1".to_string()));
            }
            other => panic!("Expected Table, got {:?}", other),
        }
    }

    #[test]
    fn engine_sivgo_set_fill() {
        let mut engine = LuaEngine::new();
        engine.load_script("fill", "sivgo.set_fill(\"#ff0000\")");
        let result = engine.execute("fill").unwrap();
        assert_eq!(result, LuaValue::Nil);
        assert_eq!(engine.get_output(), &["set_fill: #ff0000"]);
    }

    #[test]
    fn engine_sivgo_create_rect() {
        let mut engine = LuaEngine::new();
        engine.load_script("rect", "sivgo.create_rect(10, 20, 100, 50)");
        let result = engine.execute("rect").unwrap();
        match result {
            LuaValue::Table(entries) => {
                assert_eq!(entries.len(), 4);
                assert_eq!(entries[0], ("x".to_string(), LuaValue::Number(10.0)));
                assert_eq!(entries[1], ("y".to_string(), LuaValue::Number(20.0)));
                assert_eq!(entries[2], ("w".to_string(), LuaValue::Number(100.0)));
                assert_eq!(entries[3], ("h".to_string(), LuaValue::Number(50.0)));
            }
            other => panic!("Expected Table, got {:?}", other),
        }
        assert!(engine.get_output()[0].contains("create_rect"));
    }

    #[test]
    fn engine_sivgo_alert() {
        let mut engine = LuaEngine::new();
        engine.load_script("msg", "sivgo.alert(\"Hello!\")");
        engine.execute("msg").unwrap();
        assert_eq!(engine.get_output(), &["alert: Hello!"]);
    }

    #[test]
    fn engine_multiline_script() {
        let mut engine = LuaEngine::new();
        engine.set_global("x", LuaValue::Number(10.0));
        let source = r#"
-- This is a comment
x + 5
sivgo.alert("done")
"#;
        engine.load_script("multi", source);
        let result = engine.execute("multi").unwrap();
        // Last evaluated is the alert call, which returns Nil.
        assert_eq!(result, LuaValue::Nil);
        assert_eq!(engine.get_output(), &["alert: done"]);
    }

    #[test]
    fn engine_output_log_accumulates() {
        let mut engine = LuaEngine::new();
        engine.load_script("a", "sivgo.alert(\"first\")");
        engine.load_script("b", "sivgo.alert(\"second\")");
        engine.execute("a").unwrap();
        engine.execute("b").unwrap();
        assert_eq!(engine.get_output().len(), 2);
        assert_eq!(engine.get_output()[0], "alert: first");
        assert_eq!(engine.get_output()[1], "alert: second");
    }
}
