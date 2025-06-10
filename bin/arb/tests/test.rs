use anyhow::Result;
use enum_dispatch::enum_dispatch;

struct A;
struct B;

impl XTrait for A {
    async fn run(&mut self) -> Result<u32> {
        Ok(10)
    }
}
// impl XTrait for B {
//     async fn run(&mut self) -> Result<u64> {
//         Ok(20)
//     }
// }

#[enum_dispatch]
enum X {
    A,
    // B,
}

#[enum_dispatch(X)]
trait XTrait {
    async fn run(&mut self) -> Result<u32>;
}

#[tokio::test]
async fn main() {
    let mut a: X = A.into();
    // let mut b: X = B.into();
    assert_eq!(10, a.run().await.unwrap());
}
