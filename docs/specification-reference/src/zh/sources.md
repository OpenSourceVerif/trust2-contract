{{#rule contract.sources}}

# 实现依据与测试覆盖

{{#rule contract.sources.intro}}

以下源码和样例分别说明前端提取、后端翻译与已有测试覆盖。

## 前端 rust-llbc

- [规约宏实现](../../../../trust2-contract-macros/src/impl.rs)与[标识符替换](../../../../trust2-contract-macros/src/impl/parse.rs)：入口解析与展开。

- [内部接口](../../../../trust2-contract/src/internal.rs)：参数类型与闭包约束。

- [函数体规约提取](../../../../charon/charon/src/transform/collect_specs/unstructured_body.rs)与[类型规约提取](../../../../charon/charon/src/transform/collect_specs/non_body.rs)：规约与程序的关联。

- [标记识别](../../../../charon/charon/src/bin/charon-driver/translate/translate_functions.rs)：蕴含和旧状态标记。

- [前端集成样例](../../../../charon/charon/tests/cargo/trust2-contract-sample/src/lib.rs)与[LLBC 快照](../../../../charon/charon/tests/cargo/trust2-contract-sample.out)：已有组合用例。

## 后端 llbc-why3

- [函数契约翻译](../../../../cargo-verify/src/llbc_to_whyml/ctx/decl.rs)与[规约闭包翻译](../../../../cargo-verify/src/llbc_to_whyml/ctx/spec.rs)：前置条件、后置条件和捕获值。
- [语句翻译](../../../../cargo-verify/src/llbc_to_whyml/ctx/stmt_expr.rs)与[逻辑项翻译](../../../../cargo-verify/src/llbc_to_whyml/ctx/stmt_term.rs)：断言、假设和量词的翻译分支。
- [内建函数翻译](../../../../cargo-verify/src/llbc_to_whyml/ctx/func_ref.rs)：`SpecImplies`、`SpecOld` 尚待实现的分支。
- [类型翻译](../../../../cargo-verify/src/llbc_to_whyml/ctx/ty.rs)与[类型声明翻译](../../../../cargo-verify/src/llbc_to_whyml/ctx/type_decl.rs)：类型支持范围；类型不变式尚未接入自动保持检查。
- [端到端样例](../../../../cargo-verify/tests/end-to-end/simple/src/lib.rs)与[已保存结果](../../../../cargo-verify/tests/end-to-end/simple/result.txt)：`max`、`strict_sub`、`square` 的基本契约验证。

## 测试覆盖

前端集成样例包含前置条件、后置条件、断言、泛型结构体不变式，以及全称量词、蕴含和容器访问的组合。`to_sorted` 的函数体是返回空向量的占位实现，用于观察提取结果，不是已经验证的排序算法。

`old` 的用例包括可变标量、量词中的切片，以及一个单独的被拒绝用法样例 crate。另有一个样例 crate 覆盖 `==>`、`each!`、`exists!` 和 `contract_assume!`，并使用与 `cargo verify` 相同的参数提取。

后端已保存的端到端结果覆盖 `max`、`strict_sub` 和 `square` 的基本前置／后置条件。断言、假设和量词已有翻译实现，但尚未包含在这些端到端用例中。
