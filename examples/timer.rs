use core::time::Duration;

use async_std::task;

fn main() {
    pasts::Executor::default().spawn(async {
        println!("Waiting 2 seconds…");

        task::sleep(Duration::new(2, 0)).await;

        println!("Waited 2 seconds.");
    });
}
