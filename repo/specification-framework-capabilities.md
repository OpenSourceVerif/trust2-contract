# trust2-contract 前端规约能力说明

当前规约框架提供函数前置条件、后置条件、类型不变式、断言和假设。条件以 Rust 表达式为基础，可以使用量词、蕴含及受限的旧状态标记；框架将这些规约提取到 LLBC，并保存它们与函数、类型或程序位置的关联。

本文只说明**规约框架的能力**：提供哪些规约、条件中能表达什么，以及针对 Rust 数据和程序构造能描述哪些性质。判断依据是规约的声明、表达和提取实现；Charon 能翻译某种 Rust 构造，本身不能证明规约框架支持描述该构造的性质。后端验证能力和 PCG 不在本文范围内。

## 1 支持的规约类型

规约类型决定一个条件约束程序的哪个部分。

| 规约类型 | 适用 Rust 对象 | 含义 | 当前接口与范围 |
| --- | --- | --- | --- |
| 前置条件 | 函数定义 | 约束函数被调用时的参数及相关数据 | `#[precondition(P)]`；条件返回 `bool`；同一函数可声明多个前置条件 |
| 后置条件 | 函数定义 | 约束函数返回值及其与输入数据的关系 | `#[postcondition(\|result\| Q)]`；以闭包参数表示返回值；同一函数可声明多个后置条件 |
| 类型不变式 | 类型声明 | 描述类型实例应满足的数据关系 | `#[invariant(I)]`；通过 `self` 访问数据；支持结构体、枚举和联合体；保留类型参数与约束 |
| 断言 | 函数体内的程序位置 | 声明指定程序位置应当成立的条件 | `contract_assert!(P)`；保留断言及其所在位置 |
| 假设 | 函数体内的程序位置 | 声明指定程序位置引入的假设 | `contract_assume!(P)`；保留假设及其所在位置，与断言分别表示 |

函数契约要求有函数体，适用于普通函数、嵌套函数、固有方法、关联函数、trait 默认方法和 impl 中的方法；规约表达式仍须满足第 2.4 节的类型约束。只有签名的 trait 方法或外部函数声明不支持直接附加这类契约。当前也没有模块契约或外部接口补充规约机制。

验证模式下，`const fn` 不支持这种契约展开：宏插入的内部标记函数不是 `const fn`。异步函数不支持前端提取，原因是其协程类型和操作尚未实现。

例如，下面的函数同时带有输入约束和输出约束：

```rust
#[precondition(x < 16)]
#[postcondition(|result| result >= x)]
pub fn square(x: u8) -> u8 {
    x * x
}
```

类型不变式支持条件的附着与提取；当前没有自动在对象构造、字段修改或借用结束处插入不变式建立与恢复义务的机制。类型别名不支持 `#[invariant]` 标注。同一类型不能重复使用该标注生成多个 `TypeInvariant` 实现，需要在一个条件中组合多个要求。

**尚未提供的专门规约类型：**循环不变式、循环与递归的终止度量、读写足迹与修改集合、权限及借用契约、异常退出契约。函数体能够包含循环，不等于已经提供循环不变式。

## 2 规约表达能力

表达能力决定前置条件、后置条件等规约内部可以写什么。

### 2.1 程序数据与运算

| 表达对象 | 当前能力 | 示例 |
| --- | --- | --- |
| 参数与返回值 | 规约可以引用外层参数，后置条件可以引用返回值 | `result >= a` |
| 字段与引用 | 可以读取结构体字段和引用指向的数据 | `self.start <= self.end` |
| 数值运算 | 使用 Rust 数值类型及其算术、比较运算；相关运算和检查进入规约体 | `i + 1 < n` |
| 函数调用 | 可以在规约中调用普通函数 | `decuple(9) == 90` |
| 方法与索引 | 已覆盖切片长度查询、`Vec` 索引及 trait 比较调用 | `a.len()`、`b[i]` |

数值使用 Rust 的类型与运算表示，当前没有独立的数学整数或实数接口。容器通过 Rust 方法及索引访问，当前没有独立的逻辑序列、集合、映射或数据模型映射接口。

规约中的普通函数调用会保留到中间表示，但外部函数体可能是不透明的。当前没有专门的纯逻辑函数、逻辑谓词或 ghost 数据声明机制。

### 2.2 逻辑组合与量词

| 逻辑构造 | 写法 | 当前范围 |
| --- | --- | --- |
| 布尔组合与比较 | Rust 布尔表达式、相等与大小比较 | 支持 Rust 布尔组合和比较表达式 |
| 蕴含 | `implies(P, Q)` | 识别并保留为专门标记；参数计算不具有自动短路保护 |
| 全称量词 | `forall!(\|x: T\| P)` | 以 Rust 类型 `T` 声明量化变量；可在后置条件中引用输入和返回数据 |
| 存在量词 | `exists!(\|x: T\| P)` | 已实现宏展开与专门的量词提取 |

已有前端用例将全称量词、蕴含、切片长度、返回容器的索引及元素比较组合在一个后置条件中。

`implies(P, Q)` 以普通函数调用编码，其参数中的计算会先进入规约体。因此，不能把 `implies(i < len, a[i] > 0)` 直接视为能够保护索引访问的短路条件。

### 2.3 旧状态引用

后置条件中提供 `old` 标记，其接口为：

```rust
pub fn old<T>(x: &mut T) -> &mut T
```

前端将其识别为 `SpecOld`。支持该标记的识别与保留；不提供接受任意表达式的旧值快照或可达堆快照机制。其参数必须是 `&mut T`，使用时还必须满足规约闭包的 `Fn + Copy` 约束。

### 2.4 表达式约束

前置条件、后置条件、断言、假设和量词中的闭包必须返回 `bool`，并满足 `Fn + Copy`。因此，捕获变量、移动数据和修改外部状态都受 Rust 类型规则及这些闭包约束限制。类型不变式通过返回 `bool` 的 `invariant(&self)` 方法表达。

## 3 针对 Rust 程序构造的规约能力

下表列出针对 Rust 数据和程序构造可以表达的性质及具体限制。

| Rust 数据或程序构造 | 当前能写的规约 | 当前限制 |
| --- | --- | --- |
| 结构体字段、引用指向的数据 | 读取数据，描述字段之间、输入与返回值之间的关系；通过 `self` 声明类型实例的不变式 | 没有独立的数据抽象模型接口；不变式建立与恢复义务尚未在构造、修改或借用结束处自动插入 |
| 切片、`Vec` 等容器数据 | 使用长度、索引、元素比较和量词描述元素性质，例如返回容器中相邻元素的大小关系 | 使用 Rust 容器操作及索引边界检查；没有独立的逻辑序列、集合、映射接口，也没有配套的完整标准库契约 |
| 泛型数据与 trait 约束 | 沿用所属声明的泛型上下文，使用 trait 约束提供的运算；例如为 `T: PartialOrd` 的引用字段声明大小关系 | 没有 trait 接口契约的统一声明、继承与细化机制 |
| 可变局部变量及其值 | 在断言或假设所在位置引用变量，描述该位置的值，例如 `contract_assert!(x >= 0)` | 闭包捕获须满足 `Fn + Copy`；跨程序位置的值关系需要显式表达 |
| 可变引用及修改前后的状态 | 通过共享读取描述所指数据；后置条件中的 `old` 接受 `&mut T`，提取为 `SpecOld` 标记，见第 2.3 节 | 没有任意表达式的旧值快照、可达堆快照、修改集合或未修改区域保持不变的规约机制 |
| 借用、重借用及借用结束 | 使用值条件描述借用数据的内容 | 没有借用关系、访问权限、权限转移或借用结束时恢复要求的专门规约接口 |
| 分支与循环中的程序位置 | 在指定位置声明断言或假设，保存条件与该位置的关联 | 没有循环不变式或循环终止度量接口 |
| 递归函数 | 使用前置条件和后置条件，描述输入与返回值的关系 | 没有递归终止度量接口 |
| trait 方法与外部接口 | 对有函数体的方法声明其自身的前后置条件 | 契约宏要求有函数体；没有契约继承或外部接口补充规约机制 |

例如，已有用例为带泛型、trait 约束和引用字段的类型声明了不变式：

```rust
#[invariant(self.start <= self.end)]
pub struct RefRange<'a, T: PartialOrd> {
    start: &'a T,
    end: &'a T,
}
```

该不变式描述两个字段指向的数据满足 `start <= end`。规约沿用类型的生命周期参数 `'a`，并通过 `T: PartialOrd` 提供的比较运算表达字段关系。

## 附录 实现与依据

### 中间表示

函数前后置条件保存在 `FunSpecs` 中，类型不变式关联到 `TypeSpecs.invariants`。断言与假设由 `ContractAssert` 区分，量词由 `Quant` 区分。蕴含和旧状态引用分别保留为 `SpecImplies`、`SpecOld`。

规约闭包保存计算体及捕获信息，用来关联规约变量与原程序数据。这些信息随前端处理进入 LLBC。

### 源码与用例

| 依据 | 对应内容 |
| --- | --- |
| [宏实现](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/trust2-contract-macros/src/impl.rs)与[内部接口](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/trust2-contract/src/internal.rs) | 规约语法、展开、参数类型与闭包约束 |
| [函数体规约提取](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/src/transform/collect_specs/unstructured_body.rs)与[类型不变式提取](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/src/transform/collect_specs/non_body.rs) | 规约与函数、程序点和类型的关联 |
| [标记识别](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/src/bin/charon-driver/translate/translate_functions.rs) | 识别蕴含、旧状态等规约操作 |
| [前端集成用例](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/tests/cargo/trust2-contract-sample/src/lib.rs)与[LLBC 输出快照](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/tests/cargo/trust2-contract-sample.out) | 规约与 Rust 结构组合的具体输出 |
| [规约体结构](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/src/ast/gast.rs)与[类型规约结构](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/src/ast/types.rs) | 函数契约、规约捕获及类型不变式的保存方式 |
| [规约体变换](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/src/transform/ctx.rs)与[LLBC 转换](/Users/lujiayi/Repositories/trust2-contract-workspace/repo/charon/charon/src/transform/control_flow/ullbc_to_llbc.rs) | 规约的泛型上下文，以及断言、假设和量词进入 LLBC 的处理 |

整理依据为 2026 年 10 月 21 日工作区中的规约宏、内部接口、规约提取与中间表示实现，并对照已有前端测试快照。本次为文档整理，未重新运行测试。
