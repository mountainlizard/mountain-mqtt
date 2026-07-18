//! This example uses the RP Pico W board Wifi chip (cyw43).
//! Connects to Wifi network and makes a web request to get the current time.

#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

use embassy_executor::Spawner;
use embassy_poll_example::run_example;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    run_example(spawner).await;
}
