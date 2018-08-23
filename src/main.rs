#[macro_use]
extern crate lazy_static;
extern crate clap;
extern crate sonos;
extern crate rand;
extern crate regex;

use std::collections::HashMap;
use std::sync::Mutex;
use std::net::IpAddr;
use std::time::Duration;
use std::thread;
use clap::{App, Arg};
use rand::prelude::*;
use regex::Regex;

#[derive(Clone)]
pub struct SpeakerState {
    pub volume: u8,
}

lazy_static! {
    static ref DEVICES: Mutex<Vec<sonos::Speaker>> = Mutex::new(Vec::new());
    static ref STATES: Mutex<HashMap<std::net::IpAddr, SpeakerState>> = Mutex::new(HashMap::new());
}

fn get_state(ip: std::net::IpAddr) -> Option<SpeakerState> {
    STATES.lock().unwrap().get(&ip).cloned()
}

fn set_state(ip: std::net::IpAddr, state: SpeakerState) {
    STATES.lock().unwrap().insert(ip, state);
}

fn main() {
    let matches = App::new("sonos-sabotage")
        .arg(Arg::with_name("interval")
            .help("The interval to check devices in ms")
            .short("i")
            .default_value("5000")
            .takes_value(true)
        )
        .arg(Arg::with_name("pattern")
            .help("Pattern to match in assassin mode")
            .short("p")
            .default_value("Ed Sheeran")
            .takes_value(true)
        )
        .arg(Arg::with_name("uri")
            .help("The URI to play in dictator mode")
            .short("u")
            .default_value("x-sonos-spotify:spotify:track:1wsRitfRRtWyEapl0q22o8")
            .takes_value(true)
        )
        .arg(Arg::with_name("ip")
            .help("Device IP address to target")
            .short("x")
            .takes_value(true)
        )
        .arg(Arg::with_name("devices")
            .help("Print out all devices found on the current network and exit")
            .short("y")
            .long("devices")
        )
        .arg(Arg::with_name("percent")
            .help("Percent of the time saboteur mode should take an action")
            .short("z")
            .default_value("5")
            .long("percent")
        )
        .arg(Arg::with_name("oldman")
             .help("This mode detects volume increases on devices and turns them down")
             .short("o")
             .long("oldman")
        )
        .arg(Arg::with_name("assassin")
            .help("This mode matches specific tracks and eliminates them")
            .short("a")
            .long("assassin")
            .conflicts_with_all(&[
                "dictator",
                "totalitarian"
            ])
        )
        .arg(Arg::with_name("dictator")
            .help("This mode enforces a specific track to be playing")
            .short("d")
            .long("dictator")
            .conflicts_with_all(&[
                "saboteur",
                "totalitarian"
            ])
        )
        .arg(Arg::with_name("saboteur")
            .help("This mode aims to completely disrupt playback")
            .short("s")
            .long("saboteur")
        )
        .arg(Arg::with_name("totalitarian")
            .help("This mode clears all queues and stops playing tracks")
            .short("t")
            .long("totalitarian")
        )
        .get_matches();

    thread::spawn(|| {
        loop {
            discover_devices();

            thread::sleep(Duration::from_millis(30000));
        }
    });

    let check_interval = matches.value_of("interval").unwrap();

    loop {
        let devices = DEVICES.lock().unwrap();

        for device in devices.iter() {
            println!("Checking device {} at {}", device.name, device.ip);

            if matches.is_present("ip") {
                let ip = matches.value_of("ip").unwrap().parse::<IpAddr>().unwrap();
                if device.ip != ip {
                    return
                }
            }

            let previous_state = get_state(device.ip);

            if matches.is_present("oldman") {
                old_man(device, previous_state);
            }

            if matches.is_present("assassin") {
                assassin(device, matches.value_of("pattern").unwrap());
            }

            if matches.is_present("dictator") {
                dictator(device, matches.value_of("uri").unwrap());
            }

            if matches.is_present("saboteur") {
                saboteur(device, matches.value_of("percent").unwrap());
            }

            if matches.is_present("totalitarian") {
                totalitarian(device);
            }

            set_state(device.ip, SpeakerState {
                volume: device.volume().unwrap(),
            });
        }

        thread::sleep(Duration::from_millis(check_interval.parse::<u64>().unwrap()));
    }
}

fn discover_devices() {
    println!("Scanning for Sonos devices...");

    let mut device_state = DEVICES.lock().expect("Could not lock device mutex");
    let devices = sonos::discover().unwrap();

    if devices.len() == 0 {
        println!("No devices found!");
        return;
    }

    println!("Found {} devices", devices.len());

    *device_state = devices;
}

fn old_man(device: &sonos::Speaker, previous_state: std::option::Option<SpeakerState>) {
    if previous_state.is_some() {
        let current_volume = device.volume().unwrap();
        let previous_volume = previous_state.unwrap().volume;

        if current_volume > previous_volume {
            let difference: u8 = current_volume - previous_volume;

            // TODO: Accept these params as arguments
            if difference > 5 {
                let reduction: i8 = current_volume as i8 - (difference as f32 * 1.3) as i8;

                if reduction > 0 {
                    println!("Detected volume increase of {} points! Decreasing to {}", difference, reduction);
                    device.set_volume(reduction as u8).unwrap();
                }
            }
        }
    }
}

fn assassin(device: &sonos::Speaker, pattern: &str) {
    if let Ok(track) = device.track() {
        let regex = Regex::new(pattern.trim()).unwrap();

        if regex.is_match(&track.title) || regex.is_match(&track.artist) {
            // TODO: Optional subtle fade out and skip mode
            println!("Detected matched track! Assassinating {} {}", track.title, track.artist);

            match device.next() {
                Ok(_) => println!("Skipped to next track in the queue"),
                Err(_) => {
                    device.stop().unwrap();
                    println!("Could not skip, stopping playback")
                }
            }
        }
    }
}

fn dictator(device: &sonos::Speaker, uri: &str) {
    if let Ok(track) = device.track() {
        if track.uri == uri {
            return
        }
        println!("Device playing forbidden track! {} {}", track.uri, uri);

        device.clear_queue().unwrap();

        match device.play_track(uri) {
            Ok(_) => println!("Corrected it"),
            Err(err) => println!("Could not dictate track, {}", err)
        }
    }
}

fn saboteur(device: &sonos::Speaker, percent: &str) {
    let mut rng = thread_rng();
    let action_chance = rng.gen_range(0, 100);

    if action_chance > percent.parse::<u32>().unwrap() {
        let action_choice = rng.gen_range(1, 4);

        // TODO: Add more choices and configurable weighted randomness
        match action_choice {
            1 => device.mute().unwrap(),
            2 => device.next().unwrap(),
            3 => device.set_volume(rng.gen_range(0, 100)).unwrap(),
            4 => device.pause().unwrap(),
            _ => {},
        }

        println!("Sabotage device with {}", action_choice);
    }
}

fn totalitarian(device: &sonos::Speaker) {
    let transport_state = device.transport_state().unwrap();

    if transport_state == sonos::TransportState::Playing {
        println!("Device is active, shutting it down");

        device.stop().unwrap();
        device.clear_queue().unwrap();
    }
}
