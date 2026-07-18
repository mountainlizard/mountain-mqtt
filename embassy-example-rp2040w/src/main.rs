//! This example uses the RP Pico W board Wifi chip (cyw43).
//! Connects to Wifi network and makes a web request to get the current time.

#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

mod action;
mod channels;
mod event;
mod example_mqtt_manager;
mod ui;

use crate::action::Action;
use crate::channels::{ActionChannel, EventChannel};
use crate::event::Event;
use crate::ui::ui_task;
use cyw43::{aligned_bytes, JoinOptions};
use cyw43_pio::{PioSpi, DEFAULT_CLOCK_DIVIDER};
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::Ipv4Address;
use embassy_net::{Config, StackResources};
use embassy_rp::clocks::RoscRng;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, DMA_CH1, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::{bind_interrupts, dma};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pubsub::PubSubChannel;
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    // Wifi on 0 and 1
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

const WIFI_NETWORK: &str = env!("WIFI_NETWORK");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
const MQTT_HOST: &str = env!("MQTT_HOST");
const MQTT_PORT: &str = env!("MQTT_PORT");

const UID: &str = "embassy-poll-example-uid";

static EVENT_CHANNEL: StaticCell<EventChannel> = StaticCell::new();
static ACTION_CHANNEL: StaticCell<ActionChannel> = StaticCell::new();

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<
        'static,
        cyw43::SpiBus<Output<'static>, PioSpi<'static, PIO0, 0>>,
        cyw43::Cyw43439,
    >,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, cyw43::NetDriver<'static>>) -> ! {
    runner.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let p = embassy_rp::init(Default::default());

    //
    // WiFi setup...
    //

    let mut rng = RoscRng;

    let fw = aligned_bytes!("../cyw43-firmware/43439A0.bin");
    // Note this is the correct file for both 2040 and 235xx
    let nvram = aligned_bytes!("../cyw43-firmware/nvram_rp2040.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(
        &mut pio.common,
        pio.sm0,
        DEFAULT_CLOCK_DIVIDER,
        pio.irq0,
        cs,
        p.PIN_24,
        p.PIN_29,
        dma::Channel::new(p.DMA_CH0, Irqs),
        dma::Channel::new(p.DMA_CH1, Irqs),
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw, nvram).await;
    spawner.spawn(unwrap!(cyw43_task(runner)));

    let clm = aligned_bytes!("../cyw43-firmware/43439A0_clm.bin");
    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());

    // Use static IP configuration instead of DHCP
    //let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
    //    address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 69, 2), 24),
    //    dns_servers: Vec::new(),
    //    gateway: Some(Ipv4Address::new(192, 168, 69, 1)),
    //});

    // Generate random seed
    let seed = rng.next_u64();

    // Init network stack
    static RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();
    let (stack, runner) = embassy_net::new(
        net_device,
        config,
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(unwrap!(net_task(runner)));

    while let Err(err) = control
        .join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
        .await
    {
        info!("join failed: {:?}", err);
    }
    info!("...Joined {}...", WIFI_NETWORK);

    info!("Waiting for link...");
    stack.wait_link_up().await;
    info!("...Link is now up!");

    info!("Waiting for DHCP...");
    stack.wait_config_up().await;
    info!("...DHCP is now up!");

    info!("WiFi active!");

    //
    // Now we have a WiFi connection, we can connect to the MQTT server
    //

    let event_channel = EVENT_CHANNEL.init(PubSubChannel::<NoopRawMutex, Event, 16, 4, 2>::new());
    let event_pub_mqtt = event_channel.publisher().unwrap();
    let event_sub_ui = event_channel.subscriber().unwrap();

    let action_channel =
        ACTION_CHANNEL.init(PubSubChannel::<NoopRawMutex, Action, 16, 4, 4>::new());
    let action_pub_ui = action_channel.publisher().unwrap();
    let action_sub = action_channel.subscriber().unwrap();

    let host = MQTT_HOST.parse::<Ipv4Address>().unwrap();
    let port = MQTT_PORT.parse::<u16>().unwrap();

    spawner.spawn(unwrap!(ui_task(
        event_sub_ui,
        action_pub_ui,
        p.PIN_12,
        control
    )));

    example_mqtt_manager::init(
        &spawner,
        stack,
        &UID,
        event_pub_mqtt,
        action_sub,
        host,
        port,
    )
    .await;

    loop {
        Timer::after(Duration::from_secs(5)).await;
    }
}
