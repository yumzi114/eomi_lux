use std::sync::{Arc, Mutex};

use embedded_graphics::{
    mono_font::{ascii::{FONT_9X15_BOLD}, MonoTextStyleBuilder}, pixelcolor::BinaryColor, prelude::*, primitives::{PrimitiveStyle, Rectangle}, text::{Baseline, Text}
};
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use esp_idf_hal::{delay::{Delay, FreeRtos}, gpio::{Input, PinDriver, Pull}, i2c::{I2cConfig, I2cDriver}, prelude::Peripherals, task::block_on, timer::{TimerConfig, TimerDriver}};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_hal::prelude::*;
use veml7700::Veml7700;
use esp32_nimble::{enums::{AuthReq, SecurityIOCap}, utilities::BleUuid, uuid128, BLEAdvertisementData, BLECharacteristic, BLEDevice, NimbleProperties};
use esp_idf_svc::sys::{self as _};


const DEVICE_NUM:u8 = 1;
#[derive(Copy, Clone,PartialEq)]
enum AppMode {
    LUX,
    RAW
}

static BUTTON_STACK_SIZE: usize = 4000;
fn main() -> anyhow::Result<()>{
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let delay: Delay = Default::default();
    esp_idf_svc::log::EspLogger::initialize_default();
    let ble_device = BLEDevice::take();
    let mut app_mod = Arc::new(Mutex::new(AppMode::LUX));
    let i2c_conf = I2cConfig::new().baudrate(600.kHz().into());
    //sensor config
    let s_sda = peripherals.pins.gpio7;
    let s_scl = peripherals.pins.gpio6;
    let mut s_i2c_driver = I2cDriver::new(peripherals.i2c1, s_sda, s_scl, &i2c_conf)?;
    let mut veml7700_device = Veml7700::new(s_i2c_driver);
    veml7700_device.enable().unwrap();

    let sda = peripherals.pins.gpio5;
    let scl = peripherals.pins.gpio4;
    let mut i2c_driver = I2cDriver::new(peripherals.i2c0, sda, scl, &i2c_conf)?;
    let interface = I2CDisplayInterface::new(i2c_driver);
    let mut btn_pin: PinDriver<esp_idf_hal::gpio::Gpio17, Input>= PinDriver::input(peripherals.pins.gpio17)?;
    btn_pin.set_pull(Pull::Up)?;
    
    //BLE CONFIG
    let ble_advertising = ble_device.get_advertising();
    ble_device
        .security()
        .set_auth(AuthReq::Bond)
        .set_passkey(123412)
        .set_io_cap(SecurityIOCap::NoInputNoOutput)
        .resolve_rpa();
    let server = ble_device.get_server();
    server.on_connect(|server, desc| {
        ::log::info!("Client connected: {:?}", desc);
    server
        .update_conn_params(desc.conn_handle(), 24, 48, 0, 60)
        .unwrap();

    if server.connected_count() < (esp_idf_svc::sys::CONFIG_BT_NIMBLE_MAX_CONNECTIONS as _) {
        ::log::info!("Multi-connect support: start advertising");
        ble_advertising.lock().start().unwrap();
    }

    });
    server.on_disconnect(|_desc, reason| {
    ::log::info!("Client disconnected ({:?})", reason);
    });
    let service = server.create_service(uuid128!("fafafafa-fafa-fafa-fafa-fafafafafafa"));
    let static_characteristic = service.lock().create_characteristic(
        uuid128!("d4e0e0d0-1a2b-11e9-ab14-d663bd873d93"),
        NimbleProperties::READ,
        );
        static_characteristic
        .lock()
        .set_value("EOMi esp32 lux".as_bytes());
    let notifying_characteristic = service.lock().create_characteristic(
        uuid128!("a3c87500-8ed3-4bdf-8a39-a01bebede295"),
        NimbleProperties::READ | NimbleProperties::NOTIFY,
    );
    notifying_characteristic.lock().set_value(b"Initial value.");
    ble_advertising.lock().set_data(
        BLEAdvertisementData::new()
          .name(format!("[eomi]-lux{}",DEVICE_NUM).as_str())
          .add_service_uuid(BleUuid::Uuid16(0xABCD)),
      )?;
    ble_advertising.lock().start()?;
    server.ble_gatts_show_local();
    let white_mem: Arc<Mutex<u16>>=Arc::new(Mutex::new(0 as u16));
    let lux_mem: Arc<Mutex<f32>>=Arc::new(Mutex::new(0 as f32));
    let raw_mem: Arc<Mutex<u16>>=Arc::new(Mutex::new(0 as u16));
    // let ble = ble_advertising.clone();
    //BT Thread
    let mem=app_mod.clone();
    let _button_thread = std::thread::Builder::new()
        .stack_size(BUTTON_STACK_SIZE)
        .spawn(move || button_thread_function(btn_pin,mem))
        ?;
    let white_mem_c=white_mem.clone();
    let lux_mem_c=lux_mem.clone();
    let raw_mem_c=raw_mem.clone();
    let mem=app_mod.clone();
    let _button_thread = std::thread::Builder::new()
        .stack_size(BUTTON_STACK_SIZE)
        .spawn(move || ble_thread_function(
            white_mem_c,
            lux_mem_c,
            raw_mem_c,
            notifying_characteristic,
            mem
        ))
    ?;
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,
        DisplayRotation::Rotate0,
    ).into_buffered_graphics_mode();
    display.init().unwrap();
    // let _ = display.clear(BinaryColor::On);
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_9X15_BOLD)
        .text_color(BinaryColor::On)
        .build();
    let clear_style = PrimitiveStyle::with_fill(BinaryColor::Off);
    let mut white_flag=0;
    let mut lux_flag=0.;
    let mut raw_flag=0;
    
    loop{
        Text::with_baseline("[EOMI]LUX", Point::zero(), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        display.flush().unwrap();
        Rectangle::new(Point::new(0, 16), Size::new(128, 16)) // (x, y), (width, height)
            .into_styled(clear_style)
            .draw(&mut display)
            .unwrap();
        match *app_mod.lock().unwrap() {
            AppMode::LUX=>{
                Text::with_baseline(format!("LUX MODE").as_str(), Point::new(0, 16), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                display.flush().unwrap();
            },
            AppMode::RAW=>{
                Text::with_baseline(format!("RAW MODE").as_str(), Point::new(0, 16), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                display.flush().unwrap();

            }
        }
        
        let white = veml7700_device.read_white().unwrap();
        
        // // esp_println::println!("Init!");
        let lux = veml7700_device.read_lux().unwrap();
        let raw = veml7700_device.read_raw().unwrap();
        *white_mem.lock().unwrap()=white;
        *lux_mem.lock().unwrap()=lux;
        *raw_mem.lock().unwrap()=raw;
        // display.clear_buffer();
        Text::with_baseline(format!("WHITE : ").as_str(), Point::new(0, 32), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        if white_flag!=white{
            Rectangle::new(Point::new(70, 32), Size::new(128, 16)) // (x, y), (width, height)
                .into_styled(clear_style)
                .draw(&mut display)
                .unwrap();
            Text::with_baseline(format!("{}",white).as_str(), Point::new(70, 32), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
            white_flag=white;
        }
        Rectangle::new(Point::new(0, 48), Size::new(30, 16)) // (x, y), (width, height)
                .into_styled(clear_style)
                .draw(&mut display)
                .unwrap();
        match *app_mod.lock().unwrap() {
            AppMode::LUX=>{
                Text::with_baseline(format!("LUX : ").as_str(), Point::new(0, 48), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                if lux_flag!=lux{

                    Rectangle::new(Point::new(50, 48), Size::new(128, 16)) // (x, y), (width, height)
                        .into_styled(clear_style)
                        .draw(&mut display)
                        .unwrap();
                    Text::with_baseline(format!("{:.2}",lux).as_str(), Point::new(50, 48), text_style, Baseline::Top)
                        .draw(&mut display)
                        .unwrap();
                    lux_flag=lux;
                }
            },
            AppMode::RAW=>{
                Text::with_baseline(format!("RAW : ").as_str(), Point::new(0, 48), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                if raw_flag!=raw{

                    Rectangle::new(Point::new(50, 48), Size::new(128, 16)) // (x, y), (width, height)
                        .into_styled(clear_style)
                        .draw(&mut display)
                        .unwrap();
                    Text::with_baseline(format!("{}",raw).as_str(), Point::new(50, 48), text_style, Baseline::Top)
                        .draw(&mut display)
                        .unwrap();
                    raw_flag=raw;
                }
            },
        }
        
        
        display.flush().unwrap();
        FreeRtos::delay_ms(1);
    }
    Ok(())
}


fn button_thread_function(
    btn_pin: PinDriver<esp_idf_hal::gpio::Gpio17, Input>,
    app_state:Arc<Mutex<AppMode>>
) {
    let mut flag = 0;
    loop {
        if btn_pin.is_low(){
            if flag==0{
                flag=1;
                if *app_state.lock().unwrap() ==AppMode::LUX{
                    *app_state.lock().unwrap()=AppMode::RAW;
                }
                else{
                    *app_state.lock().unwrap()=AppMode::LUX
                }
                FreeRtos::delay_ms(100);
            }
        }else{
            flag=0;
        }
        FreeRtos::delay_ms(100);
    }
}
fn ble_thread_function(
    white_mem: Arc<Mutex<u16>>,
    lux_mem: Arc<Mutex<f32>>,
    raw_mem: Arc<Mutex<u16>>,
    ble_ctrol:Arc<esp32_nimble::utilities::mutex::Mutex<BLECharacteristic>>,
    app_state:Arc<Mutex<AppMode>>
){
    loop{
        let data = if *app_state.lock().unwrap()==AppMode::RAW {
            encode_data(
                true,
                DEVICE_NUM,
                *white_mem.lock().unwrap(),
                *raw_mem.lock().unwrap(),
            )
        }else{
            l_encode_data(
                false, 
                DEVICE_NUM, 
                *white_mem.lock().unwrap(), 
                *lux_mem.lock().unwrap()
            )
        };
        ble_ctrol
        .lock()
        .set_value(&data.to_be_bytes())
        .notify();
        let data = if *app_state.lock().unwrap()==AppMode::RAW {
            encode_data(
                true,
                DEVICE_NUM,
                *white_mem.lock().unwrap(), 
                *raw_mem.lock().unwrap(),
            )
        }else{
            l_encode_data(false, 
                DEVICE_NUM, 
                *white_mem.lock().unwrap(), 
                *lux_mem.lock().unwrap()
            )
        };
        match (data >> 63) & 1 == 1 {
            true =>{
                let data = decode_data(data);
                println!("Decoded: status = {}, device_number = {}, white = {}, raw = {}", data.0, data.1, data.2, data.3);
            },
            false=>{
                let data = l_decode_data(data);
                println!("Decoded: status = {}, device_number = {}, white = {}, lux = {:.2}", data.0, data.1, data.2, data.3);
            }
        };
        println!("BLE THREAD");
        FreeRtos::delay_ms(100);
    }
}
fn encode_data(status: bool, device_number: u8, white: u16, data: u16) -> u64 {
    let status_bit = if status { 1 } else { 0 }; 
    let device_bits = device_number;
    
    let encoded: u64 = ((status_bit as u64) << 63) 
        | ((device_bits as u64) << 56)
        | ((white as u64) << 40)
        | (data as u64);
    encoded
}
fn l_encode_data(status: bool, device_number: u8, white: u16, data: f32) -> u64 {
    let status_bit = if status { 1 } else { 0 }; 
    let device_bits = device_number;
    
    let l_encoded: u64 = ((status_bit as u64) << 63) 
        | ((device_bits as u64) << 56)
        | ((white as u64) << 40)
        | (data as u64);
    l_encoded
}


fn decode_data(encoded_data: u64) -> (bool, u8, u16, u32) {
    let status = (encoded_data >> 63) & 1 == 1;
    let device_number = (encoded_data >> 56) & 0x01;
    let white = (encoded_data >> 40) & 0xFFFF;
    let data = (encoded_data & 0xFFFFFFFF) as u32;
    
    (status, device_number as u8, white as u16, data)
}
fn l_decode_data(encoded_data: u64) -> (bool, u8, u16, f32) {
    let status = (encoded_data >> 63) & 1 == 1;
    let device_number = (encoded_data >> 56) & 0x01;
    let white = (encoded_data >> 40) & 0xFFFF;
    let data = (encoded_data & 0xFFFFFFFF) as f32;
    
    (status, device_number as u8, white as u16, data)
}