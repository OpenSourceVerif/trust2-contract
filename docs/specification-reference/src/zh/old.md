{{#rule contract.old}}

# 旧状态表达式

{{#syntax OldExpression}}

{{#rule contract.old.intro}}

## 预期语义

后置条件中的旧状态表达式引用函数的前状态，即进入函数时的状态，用于描述返回时状态与先前状态之间的关系。

当前 `old` 构造为此提供一个标记，其接口受下述限制。它尚未提供对 Rust 表达式的通用快照操作。

{{#rule contract.old.example}}

## 语法示例

```rust,ignore
*old(p)
```

该片段将旧状态标记与解引用组合。参数 `p` 必须符合可变引用接口。片段能否用于后置条件，还取决于外层闭包的捕获和借用。

{{#rule contract.old.boundary}}

## 实现状态 <a id="contract.old.boundary-title"></a>

后置条件宏改写名称 `old`，前端识别并保留 `SpecOld` 标记。其他契约宏不执行这一改写。

内部签名为：

```rust,ignore
pub fn old<T>(x: &mut T) -> &mut T
```

后置条件仍须满足 `Fn + Copy`，可变捕获受到这些要求的限制。对整数 `x`，`old(x + 1)` 即使表面结构符合 `OldExpression`，也不符合该接口。

名称替换测试覆盖改写，前端具有标记识别能力。这些阶段不提供任意值或可达堆的快照，也不提供保持未修改内存区域的机制。所列集成样例尚未覆盖完整的旧状态规约。
