use std::sync::{Arc, Mutex};

use embedded_graphics::{
    mono_font::{ascii::{FONT_9X15_BOLD}, MonoTextStyleBuilder}, pixelcolor::BinaryColor, prelude::*, primitives::{PrimitiveStyle, Rectangle}, text::{Baseline, Text}
};
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use esp_idf_hal::{delay::{Delay, FreeRtos}, gpio::{Input, PinDriver, Pull}, i2c::{I2cConfig, I2cDriver}, prelude::Peripherals};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_hal::prelude::*;
use veml7700::Veml7700;
use esp_idf_svc::sys::{self as _};

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
    // let button = peripherals.pins.gpio15.into_pull_up_input();
    let mem=app_mod.clone();
    // btn_pin.set_low();
    let _button_thread = std::thread::Builder::new()
        .stack_size(BUTTON_STACK_SIZE)
        .spawn(move || button_thread_function(btn_pin,mem))
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