extern crate rust_coreaudio;
use rust_coreaudio::utils;
use rust_coreaudio::stream;

fn show_result() {
    for id in utils::get_all_device_ids().unwrap() {
        show_device_info(id);
    }
}

fn show_device_info(id: u32) {
    print!("Device {}: ", id);
    let mut info: String = String::from("");
    let scopes = vec![utils::Scope::Input, utils::Scope::Output];
    for scope in &scopes {
        if !utils::in_scope(id, scope).unwrap() {
            continue;
        }
        if !info.is_empty() {
            info += ", ";
        }
        info += get_device_info(id, scope).as_ref();
    }
    println!("{}", info);
}

fn get_device_info(id: u32, scope: &utils::Scope) -> String {
    let default_id = utils::get_default_device_id(scope).unwrap();
    let mut info = String::from(if id == default_id { "(default " } else { "(" });
    info += (to_string(scope) + ") ").as_ref();
    info += utils::get_device_label(id, scope).unwrap().as_ref();
    info
}

fn to_string(scope: &utils::Scope) -> String {
    if scope == &utils::Scope::Input {
        "Input".to_string()
    } else {
        "Output".to_string()
    }
}

fn change_default_devices() {
    change_default_device(&utils::Scope::Input);
    change_default_device(&utils::Scope::Output);
}

fn change_default_device(scope: &utils::Scope) {
    let devices = utils::get_device_ids(scope).unwrap_or_default();
    if devices.len() < 2 {
        return;
    }

    let current_device = utils::get_default_device_id(scope).unwrap();
    let new_device = devices
        .into_iter()
        .find(|&device| device != current_device)
        .unwrap();
    assert!(utils::set_default_device(new_device, scope).is_ok());
}

struct SynthesizedData(f64);
impl From<SynthesizedData> for f32 {
    fn from(d: SynthesizedData) -> f32 {
        d.0 as f32
    }
}
impl From<SynthesizedData> for i16 {
    fn from(d: SynthesizedData) -> i16 {
        (d.0 * 32767.0) as i16
    }
}

struct Synthesizer {
    channels: u32,
    rate: f64,
    volume: f64,
    phase: Vec<f64>,
}
impl Synthesizer {
    fn new(channels: u32, rate: f64, volume: f64) -> Self {
        let phase = vec![0.0; channels as usize];
        Synthesizer {
            channels,
            rate,
            volume,
            phase,
        }
    }

    fn run<T> (&mut self, buffer: &mut rust_coreaudio::stream::Buffer<T>, frames: usize)
    where T: std::convert::From<SynthesizedData> {
        for frame in 0..frames {
            for channel in 0..self.channels {
                let data = SynthesizedData(self.phase[channel as usize].sin() * self.volume);
                buffer.write(frame, channel, data.into());
                self.phase[channel as usize] += self.get_increment(channel);
            }
        }
    }

    fn get_increment(&self, channel: u32) -> f64 {
        use std::f64::consts::PI;
        let freq = 220.0 * ((channel + 1) as f64);
        2.0 * PI * freq / self.rate
    }
}

fn play_sound() {
    use stream::{Buffer, CallbackArgs, Format, Stream};
    use std::f64::consts::PI;

    const CHANNELS: u32 = 2;
    const RATE: f64 = 44_100.0;
    let mut synthesizer = Synthesizer::new(CHANNELS, RATE, 0.5);

    // let format = Format::F32LE;
    // type Args = CallbackArgs<Buffer<f32>>;
    let format = Format::S16LE;
    type Args = CallbackArgs<Buffer<i16>>;
    let callback = move |args| {
        let Args { mut data, frames } = args;
        synthesizer.run(&mut data, frames);
    };
    let stm = Stream::new(CHANNELS, format, RATE, callback).unwrap();

    stm.start().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(1000));

    stm.stop().unwrap();
}

fn main() {
    play_sound();
    // show_result();
    // change_default_devices();
}
