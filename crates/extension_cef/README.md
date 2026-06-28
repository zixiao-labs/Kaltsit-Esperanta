# 适用于带有WebView的扩展的CEF绑定

此处由于FFI，unsafe代码可能较多，心脏不好的慎入（虽然不会让整个编辑器段错误但是别让Nathan Sobo看到，GPUI也是unsafe堆出来的）

将使用C ABI（extern "C"）暴露动态链接接口（省我们的编译时间&最终包存储空间&内存占用，阿里云ECS和OSS很贵的）

请确保libcef，libcef_dll_wrapper和libcef.lib（如果需要）在系统中，不然会遇到扩展无法激活的情况
