{{#rule contract.assumption}}

# 假设

{{#syntax Assumption}}

## 语义

`contract_assume!(P);` 在当前位置将布尔条件 `P` 引入为推理假设，不产生证明 `P` 的义务。

使用这一假设得到的结论以该假设成立为前提。不一致的假设会使后续证明义务空泛成立。

## 示例

```rust,ignore
use trust2_contract::prelude::*;

pub fn assumed_bound(x: u8) {
    contract_assume!(x <= 10);
    contract_assert!(x < 11);
}
```

在假定的上界下可以推出断言。该标注既不检查，也不为调用者建立这个上界。

## 实现状态

宏和内部接口将条件表示为 `Fn() -> bool + Copy` 闭包。专门的提取分支将假设与断言区分开。现有前端集成样例尚未覆盖假设。

假设在普通构建中会被移除，不约束运行时的值，也不拒绝输入。
