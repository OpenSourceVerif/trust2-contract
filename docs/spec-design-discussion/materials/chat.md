```rust
#[derive(Clone)]
struct NotCopy {
    val: i32
}

// #[check(ghost)]
#[pure]
fn consume(_x: NotCopy) -> bool{
    true
}

#[pure]
fn consume(_x: &NotCopy) -> bool {
    true
}

let x = NotCopy(0);
// prusti_assert: Fn -> bool
prusti_assert!(consume(x)); 
```

prusti 没办法打破 Rust 的 substructure type 的规则，即使它是一个 spec。

Is this a non-limitation or an explicit design? 当然是 limitation 还是 design choice 这个可以讨论。

但我所知道的话，verus 和 creusot 好像都是允许这种写法的。他们的 spec 里面可以打破 substructurual 规则。

而且引用和值本身它不完全相同。这可能涉及到一些特定的用例，需要区分两者。我们会不会遇到我们希望区分它是关于 NotCopy 的谓词，还是 NotCopy 的引用的谓词的情况？

如果说只能写成引用，是的，它很 Rust，很符合运行语义，但是它好像有一点 limited。

或者说你能不能找到这样一个用例，或者说就是所有的现实用例，prusti 这种做法都是可以处理的。

最重要的是，一个关于引用的谓词在拿到 & 引用之后解引用，是否能达到和关于值的谓词一样的效果？

```rust

struct Ast(());

// both runtime & spec
fn normalize(ast: Ast) -> Ast {
    todo!()
}

// spec
fn idempotent(ast: Ast) -> bool {
    normalize(normalize(ast)) == normalize(ast)
}

// v.s. 

fn normalize(ast: &Ast) -> Ast {
    todo!()
}

fn idempotent(ast: &Ast) -> bool {
    let normalized = normalize(ast);
    normalize(&normalized) == normalize(normalized)
}

```

某种意义上我觉得prusti的这套设计还是挺合理的。而且确实还挺接近rust语义。
它的snapshot编码是怎么做的？
这个方案出人意料的不 Hacky，我甚至感觉比 Verus 的都不 Hacky 多了
怎么办？我现在有点开始欣赏他了。他的做法确实和另外那两个不太一样。这个是比较忠实运行语义的
他甚至给 Pure 生成一个 VC 断言 Spec 等于执行。不过如果这个 VC 出现 unknown 的话，放普通编译器那应该也算是某种 internal compiler error 了。或者得用户手动去修，那也会感觉挺突然的。
好的，我所以我又进一步研究那个 Ast 的例子。现在看的话好像是，如果 Ghost<Ast> 能够直接 Deref 成 Ast ，而不是 &Ast 的话，prusti 应该是能完全表达这个例子的。

目前 Deref 成 &Ast，有一个workaround是用量词，不过有点丑陋。Deref 受rust trait接口限制，只能返回引用。

```rust
enum Ast {
    Literal(i32),
    AddZero(i32),
}

#[pure]
fn normalize(ast: Ast) -> Ast {
    match ast {
        Ast::Literal(n) => Ast::Literal(n),
        Ast::AddZero(n) => Ast::Literal(n),
    }
}

let ast = Ghost::new_ref(&ast);
normalize(normalize(ast)) === normalize(ast)

prusti_assert!(forall(|a: Ast, b: Ast|
    (a === b) ==> (normalize(normalize(a)) === normalize(b))
));
```

prusti的Ghost<T>和T的行为一致吗？

比如我们有这个函数，Normalized，它的运行时实现本身就采用了一个函数式的写法。但是为了证明幂等性，我们需要调用它两遍。我们又假设这个 AST 没有提供运行时的 clone，或者运行时的 clone 不一定是一个逻辑上 no-op 的操作。那这个时候我们需要使用 ghost。这个场景现实吗？那这个 ghost，它和普通的 Ast 是一样的吗？可以使用等号比较吗？

不过现在我觉得最大的疑惑还是集中在为什么要在逻辑代码里面维护 substructural type 的规则。
好像维护它也没有什么特别的好处，而且prusti也提供了很多机制绕过这个规则。
看起来pure/spec的部分也没有很...需要这个 borrow check 的结果。
那关键是它生成 mir 的时候，rustc 前端进行了 borrow checking，从而强制了这些规则；然后prusti消费的时候并不关心是MIR operand 是 copy 还是move。
除了 CFG 和 borrowck 以外，从THIR到MIR还经过了什么？Vyper是基于结构控制流还是CFG的？这可能在一定程度上决定了他们的取舍。或者说有没有办法降低到 MLR，但是不做borrow checking？
