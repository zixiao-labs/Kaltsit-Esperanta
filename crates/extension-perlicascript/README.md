# 使用PerlicaScript开发扩展

## 介绍

PerlicaScript是一个语法来源于TypeScript的脚本语言，相比原版的Rust+WASM提供更多灵活性和更快的开发迭代速度。PerlicaScript扩展可以直接在Zed（ZetaCode内置，原版Zed适配器正在开发中，暂不提供VS Code适配器）中编写和调试，无需额外的构建步骤。

## 优点

- **有GC**: Rust的所有权有多反直觉不用多说了吧
- **热重载**: 修改后立即生效（dev状态下，prod还是要编译成字节码）
- **基于Cranelift的JIT**: 动态语言，系统级性能
- **RTTI**: 运行时类型信息，便于调试和开发，以及减少JIT时的类型混淆问题


## 快速入门

创建extension目录，添加`extension.toml`：

```toml
id = "my-extension"
name = "My Extension"
description = "..."
version = "0.0.1"
schema_version = 1
authors = ["Your Name <you@example.com>"]
repository = "https://github.com/your/extension-repository"
```

创建extension.pscript

```
import { toast } from "zed"

export function OnActive: {

  zed.toast(string:"HelloWorld")

}

```

然后确保系统中安装了息壤编辑器基础设施和ZetaCode CLI，随后在插件目录中执行

```bash
zeta pack
```

将编译为字节码并创建tar.gz归档

回到编辑器，按cmd+shift+x，然后点击Install Dev Extension，选中归档

应当被加载并弹出toast通知

## 应用场景

LSP语言适配器

```
import { lsp } from "zed"
import { spawn } from "std"
```
