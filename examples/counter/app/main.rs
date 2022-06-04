// Shim for providing async main
extern crate alloc;

#[allow(unused_imports)]
use self::main::*;

mod main {
    include!("../src/main.rs");

    #[allow(clippy::module_inception)]
    pub(super) mod main {
        pub(crate) async fn main(e: alloc::sync::Weak<pasts::Executor>) {
            super::main(&e).await
        }
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "none"),
    wasm_bindgen(start)
)]
pub fn main() {
    let executor = alloc::sync::Arc::new(pasts::Executor::default());
    executor.spawn(Box::pin(self::main::main::main(
        alloc::sync::Arc::downgrade(&executor),
    )));
}
