use anyhow::Result;
use perlica_runtime::gc::Gc;
use perlica_runtime::value::Value;
use perlica_runtime::value::{FunctionValue, ObjectValue};
use std::collections::HashMap;

/// Zed API bindings for PerllicaScript extensions.
///
/// This module provides the API surface that PerllicaScript extensions
/// can use to interact with the Zed editor.
pub struct PerlicascriptApi {
    /// Registered commands
    commands: HashMap<String, Box<dyn Fn(&[Value]) -> Result<Value>>>,
}

impl PerlicascriptApi {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a command handler.
    pub fn register_command(
        &mut self,
        name: &str,
        handler: Box<dyn Fn(&[Value]) -> Result<Value>>,
    ) {
        self.commands.insert(name.to_string(), handler);
    }

    /// Call a registered command.
    pub fn call_command(&self, name: &str, args: &[Value]) -> Result<Value> {
        if let Some(handler) = self.commands.get(name) {
            handler(args)
        } else {
            Err(anyhow::anyhow!("Unknown command: {}", name))
        }
    }
}

impl Default for PerlicascriptApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the complete Zed API object for injection into the runtime.
pub fn build_zed_api_object() -> Value {
    let mut zed_props = HashMap::new();

    // zed.toast(message)
    zed_props.insert(
        "toast".to_string(),
        Value::Function(Gc::new(FunctionValue {
            name: "toast".to_string(),
            params: vec!["message".to_string()],
            bytecode_index: 0,
            is_native: true,
            native_fn: Some(Box::new(|args| {
                if let Some(Value::String(msg)) = args.first() {
                    log::info!("[Zed Toast] {}", msg);
                    Ok(Value::Undefined)
                } else {
                    Err(perlica_runtime::value::RuntimeError::TypeError(
                        "toast() expects a string".to_string(),
                    ))
                }
            })),
        })),
    );

    // zed.window namespace
    let mut window_props = HashMap::new();

    window_props.insert(
        "openFile".to_string(),
        Value::Function(Gc::new(FunctionValue {
            name: "openFile".to_string(),
            params: vec!["path".to_string()],
            bytecode_index: 0,
            is_native: true,
            native_fn: Some(Box::new(|args| {
                if let Some(Value::String(path)) = args.first() {
                    log::info!("[Zed] Open file: {}", path);
                    Ok(Value::Undefined)
                } else {
                    Err(perlica_runtime::value::RuntimeError::TypeError(
                        "openFile() expects a string".to_string(),
                    ))
                }
            })),
        })),
    );

    window_props.insert(
        "activeEditor".to_string(),
        Value::Function(Gc::new(FunctionValue {
            name: "activeEditor".to_string(),
            params: vec![],
            bytecode_index: 0,
            is_native: true,
            native_fn: Some(Box::new(|_args| {
                // Return an editor proxy
                let mut editor_props = HashMap::new();
                editor_props.insert(
                    "getText".to_string(),
                    Value::Function(Gc::new(FunctionValue {
                        name: "getText".to_string(),
                        params: vec![],
                        bytecode_index: 0,
                        is_native: true,
                        native_fn: Some(Box::new(|_args| {
                            Ok(Value::String(Gc::new(String::new())))
                        })),
                    })),
                );
                editor_props.insert(
                    "setText".to_string(),
                    Value::Function(Gc::new(FunctionValue {
                        name: "setText".to_string(),
                        params: vec!["text".to_string()],
                        bytecode_index: 0,
                        is_native: true,
                        native_fn: Some(Box::new(|args| {
                            if let Some(Value::String(text)) = args.first() {
                                log::info!("[Zed] Set text: {}", text);
                                Ok(Value::Undefined)
                            } else {
                                Err(perlica_runtime::value::RuntimeError::TypeError(
                                    "setText() expects a string".to_string(),
                                ))
                            }
                        })),
                    })),
                );
                Ok(Value::Object(Gc::new(ObjectValue {
                    properties: editor_props,
                    prototype: None,
                })))
            })),
        })),
    );

    zed_props.insert(
        "window".to_string(),
        Value::Object(Gc::new(ObjectValue {
            properties: window_props,
            prototype: None,
        })),
    );

    // zed.workspace namespace
    let mut workspace_props = HashMap::new();

    workspace_props.insert(
        "rootPath".to_string(),
        Value::Function(Gc::new(FunctionValue {
            name: "rootPath".to_string(),
            params: vec![],
            bytecode_index: 0,
            is_native: true,
            native_fn: Some(Box::new(|_args| {
                Ok(Value::String(Gc::new(
                    std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )))
            })),
        })),
    );

    zed_props.insert(
        "workspace".to_string(),
        Value::Object(Gc::new(ObjectValue {
            properties: workspace_props,
            prototype: None,
        })),
    );

    // zed.lsp namespace
    let mut lsp_props = HashMap::new();

    lsp_props.insert(
        "start".to_string(),
        Value::Function(Gc::new(FunctionValue {
            name: "start".to_string(),
            params: vec!["serverName".to_string(), "command".to_string()],
            bytecode_index: 0,
            is_native: true,
            native_fn: Some(Box::new(|args| {
                log::info!("[Zed] Start LSP server: {:?}", args);
                Ok(Value::Undefined)
            })),
        })),
    );

    zed_props.insert(
        "lsp".to_string(),
        Value::Object(Gc::new(ObjectValue {
            properties: lsp_props,
            prototype: None,
        })),
    );

    Value::Object(Gc::new(ObjectValue {
        properties: zed_props,
        prototype: None,
    }))
}
