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

// use crate::action::Action;
// use crate::channels::{ActionChannel, EventChannel};
// use crate::event::Event;
// use crate::ui::ui_task;
use core::fmt::Write as _;
use cyw43::JoinOptions;
use cyw43_pio::{PioSpi, DEFAULT_CLOCK_DIVIDER};
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::join::join3;
use embassy_futures::select::{select, Either};
use embassy_net::tcp::TcpSocket;
use embassy_net::Ipv4Address;
use embassy_net::{Config, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::clocks::RoscRng;
use embassy_rp::flash::Async;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::PubSubChannel;
// use embassy_sync::blocking_mutex::raw::NoopRawMutex;
// use embassy_sync::pubsub::PubSubChannel;
// use embassy_time::{Delay, Duration, Timer};
use embassy_time::{Delay, Timer};
use embedded_io_async::Read;
use embedded_io_async::Write;
use heapless::String;
// use mountain_mqtt::client::{Client, ClientNoQueue, ConnectionSettings};
// use mountain_mqtt::client_state::ClientStateNoQueue;
// use mountain_mqtt::embedded_hal_async::DelayEmbedded;
// use mountain_mqtt::embedded_io_async::ConnectionEmbedded;
// use mountain_mqtt::packet_client::PacketClient;
use embedded_hal_async::delay::DelayNs;
use mountain_mqtt_embassy::mqtt_manager::Settings;
use rand::RngCore;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

// Required to start flash driver and get unique ID - not actually
// used for anything since we don't read/write actual flash, so
// we pick a default small value
const FLASH_SIZE: usize = 2 * 1024 * 1024;

const WIFI_NETWORK: &str = env!("WIFI_NETWORK");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");
const MQTT_HOST: &str = env!("MQTT_HOST");
const MQTT_PORT: &str = env!("MQTT_PORT");

static UID: StaticCell<String<64>> = StaticCell::new();
static CHIP_ID: StaticCell<String<64>> = StaticCell::new();
// static EVENT_CHANNEL: StaticCell<EventChannel> = StaticCell::new();
// static ACTION_CHANNEL: StaticCell<ActionChannel> = StaticCell::new();

#[embassy_executor::task]
async fn cyw43_task(
    runner: cyw43::Runner<'static, Output<'static>, PioSpi<'static, PIO0, 0, DMA_CH0>>,
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

    // Get unique id from flash
    let mut flash = embassy_rp::flash::Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH1);
    let mut uid = [0; 8];
    flash.blocking_unique_id(&mut uid).unwrap();
    let chip_id_handle = CHIP_ID.init(String::new());
    let uid_handle = UID.init(String::new());

    core::write!(
        chip_id_handle,
        "{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        uid[0],
        uid[1],
        uid[2],
        uid[3],
        uid[4],
        uid[5],
        uid[6],
        uid[7]
    )
    .unwrap();

    core::write!(uid_handle, "embassy-example-{}", chip_id_handle).unwrap();

    let mut rng = RoscRng;

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");
    // To make flashing faster for development, you may want to flash the firmwares independently
    // at hardcoded addresses, instead of baking them into the program with `include_bytes!`:
    //     probe-rs download 43439A0.bin --binary-format bin --chip RP2040 --base-address 0x10100000
    //     probe-rs download 43439A0_clm.bin --binary-format bin --chip RP2040 --base-address 0x10140000
    // let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 230321) };
    // let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

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
        p.DMA_CH0,
    );

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(cyw43_task(runner)));

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

    unwrap!(spawner.spawn(net_task(runner)));

    loop {
        match control
            .join(WIFI_NETWORK, JoinOptions::new(WIFI_PASSWORD.as_bytes()))
            .await
        {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");

    info!("waiting for link up...");
    while !stack.is_link_up() {
        Timer::after_millis(500).await;
    }
    info!("Link is up!");

    info!("waiting for stack to be up...");
    stack.wait_config_up().await;
    info!("Stack is up!");

    const B: usize = 1024;

    let mut rx_buffer = [0; B];
    let mut tx_buffer = [0; B];
    // let mut mqtt_buffer = [0; B];

    // let mut connection_index = 0u32;

    let host = MQTT_HOST.parse::<Ipv4Address>().unwrap();
    let port = MQTT_PORT.parse::<u16>().unwrap();

    let settings = Settings::new(host, port);

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(None);

        let remote_endpoint = (settings.address, settings.port);
        info!("MQTT socket connecting to {:?}...", remote_endpoint);
        if let Err(e) = socket.connect(remote_endpoint).await {
            warn!("MQTT socket connect error, will retry: {:?}", e);
            // Wait a while to try reconnecting
            Timer::after(settings.reconnection_delay).await;
            continue;
        }
        info!("MQTT socket connected!");

        // let connection = ConnectionEmbedded::new(socket);

        let rx_channel: Channel<NoopRawMutex, [u8; 8], 1> = Channel::new();
        let rx_channel_sender: embassy_sync::channel::Sender<'_, NoopRawMutex, [u8; 8], 1> =
            rx_channel.sender();
        let rx_channel_receiver = rx_channel.receiver();

        let tx_channel: Channel<NoopRawMutex, [u8; 8], 1> = Channel::new();
        let tx_channel_sender = tx_channel.sender();
        let tx_channel_receiver = tx_channel.receiver();

        let cancel_pubsub = PubSubChannel::<NoopRawMutex, bool, 1, 3, 3>::new();
        let rx_cancel_pub = cancel_pubsub.publisher().expect("rx_cancel_pub");
        let tx_cancel_pub = cancel_pubsub.publisher().expect("tx_cancel_pub");
        let poll_cancel_pub = cancel_pubsub.publisher().expect("poll_cancel_pub");

        let mut rx_cancel_sub = cancel_pubsub.subscriber().expect("rx_cancel_sub");
        let mut tx_cancel_sub = cancel_pubsub.subscriber().expect("tx_cancel_sub");
        let mut poll_cancel_sub = cancel_pubsub.subscriber().expect("poll_cancel_sub");

        let (mut rx, mut tx) = socket.split();

        let rx_fut = async {
            let mut buf = [0; 8];
            info!("rx_fut: Starting");

            loop {
                info!("rx_fut: Select on read_exact/cancel");
                match select(rx.read_exact(&mut buf), rx_cancel_sub.next_message_pure()).await {
                    Either::First(read) => match read {
                        Ok(_) => {
                            info!("rx_fut: Have read data {:?}, select on send/cancel", &buf);
                            match select(
                                rx_channel_sender.send(buf),
                                rx_cancel_sub.next_message_pure(),
                            )
                            .await
                            {
                                Either::First(_) => {
                                    info!("rx_fut: Sent data {:?} to channel", &buf)
                                }
                                Either::Second(_) => {
                                    info!("rx_fut: received cancellation (while sending to channel), will exit");
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            info!("rx_fut: Read error {}, stopping reading", e);
                            if let Err(e) = rx_cancel_pub.try_publish(true) {
                                info!("rx_fut: failed to cancel - already cancelled '{}'", e);
                            } else {
                                info!("rx_fut: cancelled");
                            }
                            break;
                        }
                    },
                    Either::Second(_) => {
                        info!(
                            "rx_fut: received cancellation (while reading from network), will exit"
                        );
                        break;
                    }
                }
            }
        };

        let tx_fut = async {
            info!("tx_fut: Starting");
            loop {
                info!("tx_fut: Select message");

                match select(
                    tx_channel_receiver.receive(),
                    tx_cancel_sub.next_message_pure(),
                )
                .await
                {
                    Either::First(write) => {
                        info!("tx_fut: Will write {:?}", &write);
                        match select(tx.write_all(&write), tx_cancel_sub.next_message_pure()).await
                        {
                            Either::First(write) => match write {
                                Ok(_) => {
                                    info!("txfut:...wrote data {:?}", &write);
                                }
                                Err(e) => {
                                    info!("txfut:...write error {}, stopping writing", e);
                                    if let Err(e) = tx_cancel_pub.try_publish(true) {
                                        info!(
                                            "tx_fut: failed to cancel - already cancelled '{}'",
                                            e
                                        );
                                    } else {
                                        info!("tx_fut: cancelled");
                                    }
                                    break;
                                }
                            },
                            Either::Second(_) => {
                                info!(
                                    "tx_fut: received cancellation (during write_all), will exit"
                                );
                                break;
                            }
                        }
                    }
                    Either::Second(_) => {
                        info!("tx_fut: received cancellation (during channel receive), will exit");
                        break;
                    }
                }
            }
        };

        let poll_fut = async {
            let mut index: u64 = 0;
            loop {
                info!(
                    "poll_fut: About to send data ({}){:?} (or cancel)",
                    &index,
                    &index.to_ne_bytes()
                );

                match select(
                    tx_channel_sender.send(index.to_ne_bytes()),
                    poll_cancel_sub.next_message_pure(),
                )
                .await
                {
                    Either::First(_) => {
                        info!("poll_fut: Sent data ({}){:?}", &index, &index.to_ne_bytes());
                    }
                    Either::Second(_) => {
                        info!("poll_fut: received cancellation (during sending data), will break");
                        break;
                    }
                }

                info!("poll_fut: About to receive data (or cancel)");

                match select(
                    rx_channel_receiver.receive(),
                    poll_cancel_sub.next_message_pure(),
                )
                .await
                {
                    Either::First(received) => {
                        info!("poll_fut: Received data {:?} from channel", &received);
                    }
                    Either::Second(_) => {
                        info!(
                            "poll_fut: received cancellation (during receiving data), will break"
                        );
                        break;
                    }
                }

                Delay.delay_ms(1000).await;

                index += 1;

                if index >= 10 {
                    info!("poll_fut: Enough data sent, cancelling...");
                    if let Err(e) = poll_cancel_pub.try_publish(true) {
                        info!("poll_fut: failed to cancel - already cancelled '{}'", e);
                    } else {
                        info!("poll_fut: cancelled");
                    }
                    break;
                }

                // match with_timeout(Duration::from_millis(5000), rx_channel_receiver.receive()).await
                // {
                //     Ok(buf) => info!("Polled data {:?} from channel", &buf),
                //     Err(_) => info!("!!! timeout"),
                // }
            }
        };

        info!("About to start tcp futures");

        join3(rx_fut, tx_fut, poll_fut).await;

        info!("rx and tx futures finished");

        info!("Finished network comms, will reconnect");

        // let delay = DelayEmbedded::new(Delay);
        // let timeout_millis = 5000; //settings.response_timeout.as_millis() as u32;

        // let packet_client = PacketClient::new(connection, &mut mqtt_buffer);
        // let client_state = ClientStateNoQueue::default();

        // let mut client = ClientNoQueue::new(
        //     connection,
        //     &mut mqtt_buffer,
        //     delay,
        //     timeout_millis,
        //     event_handler,
        // );

        // let connection_settings = ConnectionSettings::unauthenticated("poll-test-client");
        // client.connect(&settings).await;
    }

    // let client = ClientNoQueue::new(connection, buf, delay, timeout_millis, event_handler);

    // let event_channel = EVENT_CHANNEL.init(PubSubChannel::<NoopRawMutex, Event, 16, 4, 2>::new());
    // let event_pub_mqtt = event_channel.publisher().unwrap();
    // let event_sub_ui = event_channel.subscriber().unwrap();

    // let action_channel =
    //     ACTION_CHANNEL.init(PubSubChannel::<NoopRawMutex, Action, 16, 4, 4>::new());
    // let action_pub_ui = action_channel.publisher().unwrap();
    // let action_sub = action_channel.subscriber().unwrap();

    // let host = MQTT_HOST.parse::<Ipv4Address>().unwrap();
    // let port = MQTT_PORT.parse::<u16>().unwrap();

    // unwrap!(spawner.spawn(ui_task(event_sub_ui, action_pub_ui, p.PIN_12, control)));

    // example_mqtt_manager::init(
    //     &spawner,
    //     stack,
    //     uid_handle,
    //     event_pub_mqtt,
    //     action_sub,
    //     host,
    //     port,
    // )
    // .await;

    // loop {
    //     Timer::after(Duration::from_secs(5)).await;
    // }
}
