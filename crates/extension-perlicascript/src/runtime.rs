use anyhow::{Context, Result};
use perlica_compiler::bytecode::BytecodeModule;
use perlica_compiler::types::TypeSystem;
use perlica_runtime::VM;
use perlica_runtime::value::Value;
use std::collections::HashMap;

/// The runtime environment for a PerllicaScript extension.
pub struct PerlicascriptRuntime {
    vm: VM,
    modules: Vec<LoadedModule>,
}

struct LoadedModule {
    bytecode: BytecodeModule,
    type_system: TypeSystem,
}

impl PerlicascriptRuntime {
    pub fn new() -> Result<Self> {
        let mut vm = VM::new();

        // Register Zed API bindings
        register_zed_api(&mut vm);

        Ok(Self {
            vm,
            modules: Vec::new(),
        })
    }

    /// Load a compiled module into the runtime.
    pub fn load_module(&mut self, bytecode: BytecodeModule, type_system: TypeSystem) -> Result<()> {
        self.modules.push(LoadedModule {
            bytecode,
            type_system,
        });
        Ok(())
    }

    /// Call an exported function by name.
    pub fn call_function(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
        // Push arguments onto the stack
        for arg in args {
            self.vm.define_global(&format!("__arg_{}", 0), arg);
        }

        // Run the module
        if let Some(module) = self.modules.last() {
            self.vm
                .run(&module.bytecode)
                .map_err(|e| anyhow::anyhow!("Runtime error: {:?}", e))
        } else {
            Err(anyhow::anyhow!("No modules loaded"))
        }
    }
}

impl Default for PerlicascriptRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default runtime")
    }
}

/// Register Zed API functions with the VM.
fn register_zed_api(vm: &mut VM) {
    use perlica_runtime::gc::Gc;
    use perlica_runtime::value::{FunctionValue, ObjectValue};

    // zed.toast(message)
    let toast_fn = Value::Function(Gc::new(FunctionValue {
        name: "toast".to_string(),
        params: vec!["message".to_string()],
        bytecode_index: 0,
        is_native: true,
        native_fn: Some(Box::new(|args| {
            if let Some(Value::String(message)) = args.first() {
                // In production, this would call into GPUI to show a toast
                log::info!("Toast: {}", message);
                Ok(Value::Undefined)
            } else {
                Err(perlica_runtime::value::RuntimeError::TypeError(
                    "Expected string argument".to_string(),
                ))
            }
        })),
    }));

    // zed.window.openFile(path)
    let open_file_fn = Value::Function(Gc::new(FunctionValue {
        name: "openFile".to_string(),
        params: vec!["path".to_string()],
        bytecode_index: 0,
        is_native: true,
        native_fn: Some(Box::new(|args| {
            if let Some(Value::String(path)) = args.first() {
                log::info!("Open file: {}", path);
                Ok(Value::Undefined)
            } else {
                Err(perlica_runtime::value::RuntimeError::TypeError(
                    "Expected string argument".to_string(),
                ))
            }
        })),
    }));

    // zed.window.activeEditor()
    let active_editor_fn = Value::Function(Gc::new(FunctionValue {
        name: "activeEditor".to_string(),
        params: vec![],
        bytecode_index: 0,
        is_native: true,
        native_fn: Some(Box::new(|_args| {
            // Return an editor proxy object
            let mut properties = HashMap::new();
            properties.insert("path".to_string(), Value::String(Gc::new(String::new())));
            Ok(Value::Object(Gc::new(ObjectValue {
                properties,
                prototype: None,
            })))
        })),
    }));

    // Build the zed namespace
    let mut zed_properties = HashMap::new();
    zed_properties.insert("toast".to_string(), toast_fn);

    let mut window_properties = HashMap::new();
    window_properties.insert("openFile".to_string(), open_file_fn);
    window_properties.insert("activeEditor".to_string(), active_editor_fn);

    zed_properties.insert(
        "window".to_string(),
        Value::Object(Gc::new(ObjectValue {
            properties: window_properties,
            prototype: None,
        })),
    );

    let zed_module = Value::Object(Gc::new(ObjectValue {
        properties: zed_properties,
        prototype: None,
    }));

    vm.define_global("zed", zed_module);
}
