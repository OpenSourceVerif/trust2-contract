{{#rule contract.sources}}

# 实现依据与测试覆盖

{{#rule contract.sources.intro}}

以下源码和样例分别对应宏展开、前端提取和已有测试覆盖。

- [规约宏实现](../../../../trust2-contract-macros/src/impl.rs)与[标识符替换](../../../../trust2-contract-macros/src/impl/parse.rs)：入口解析与展开。

- [内部接口](../../../../trust2-contract/src/internal.rs)：参数类型与闭包约束。

- [函数体规约提取](../../../../charon/charon/src/transform/collect_specs/unstructured_body.rs)与[类型规约提取](../../../../charon/charon/src/transform/collect_specs/non_body.rs)：规约与程序的关联。

- [标记识别](../../../../charon/charon/src/bin/charon-driver/translate/translate_functions.rs)：蕴含和旧状态标记。

- [前端集成样例](../../../../charon/charon/tests/cargo/trust2-contract-sample/src/lib.rs)与[LLBC 快照](../../../../charon/charon/tests/cargo/trust2-contract-sample.out)：已有组合用例。
