{{#rule contract.specifications}}

# 规约语言

规约描述函数调用、程序状态和数据应满足的条件，以 Rust 属性和宏调用的形式书写。

{{#syntax Specification}}

[词法规则](notations.md)定义本参考手册使用的记号和 Rust 语法约定；[逻辑表达式](expressions.md)定义构成规约条件的表达式。

后续章节说明这些条件的作用位置：

- [函数契约](function-contracts.md)：调用时的要求和返回时的保证。
- [语句标注](statements.md)：函数体内某个位置的断言和假设。
- [类型不变式](invariants.md)：与类型关联的条件。

每种构造分别说明语法、语义、示例和当前实现状态。[文法索引](grammar.md)汇总全部产生式。
