#![feature(proc_macro_hygiene, decl_macro)]

// use for websocket
use websocket::sync::Server;
use websocket::OwnedMessage;

extern crate i2cdev;
use std::borrow::Borrow;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

type TemperatureList = Arc<Mutex<Vec<Vec<f32>>>>;

#[cfg(not(any(target_os = "linux")))]
fn set_thermal(t: TemperatureList) {
    println!("set thermal");
    use std::time::{SystemTime, UNIX_EPOCH};
    loop {
        thread::sleep(Duration::from_millis(100)); // sleep 0.1 second
        let now = SystemTime::now();
        let ms = (now.duration_since(UNIX_EPOCH).unwrap().as_millis() % 100_000) as f32 / 1000.0;
        let mut values = t.lock().unwrap();
        *values = vec![
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
            vec![ms, ms + 0.1, ms + 0.2, ms + 0.3, ms + 0.4, ms + 0.5, ms + 0.6, ms + 0.7, ],
        ];
    }
}

#[cfg(any(target_os = "linux"))]
fn set_thermal(t: TemperatureList) {
    use amg88xx::amg88xx::{AMG88XX, SLAVE_ADDR_PRIMARY};
    use i2cdev::linux::LinuxI2CDevice;

    let device = "/dev/i2c-1";
    let amg88xx_i2cdev = LinuxI2CDevice::new(device, SLAVE_ADDR_PRIMARY).unwrap();
    let mut amg88xx = AMG88XX::new(amg88xx_i2cdev).unwrap();

    loop {
        thread::sleep(Duration::from_millis(100));
        let pixels = amg88xx.pixels().unwrap();
        let mut values = t.lock().unwrap();
        *values = pixels;
    }
}

fn start_web_socket(temp_matrix: TemperatureList) {
    println!("start web socket");
    let ws = Server::bind("0.0.0.0:8102").unwrap();

    for request in ws.filter_map(Result::ok) {
        let matrix = Arc::clone(&temp_matrix);
        thread::spawn(move || {
            if !request.protocols().contains(&"thermal-rs".to_string()) {
                request.reject().unwrap();
                println!("Connection rejected.");
                return;
            }
            let client = request.use_protocol("thermal-rs").accept().unwrap();
            let ip = client.peer_addr().unwrap();
            println!("Connection from {}", ip);

            let (mut receiver, mut sender) = client.split().unwrap();
            for message in receiver.incoming_messages() {
                let message = message.unwrap();
                match message {
                    OwnedMessage::Close(_) => {
                        let message = OwnedMessage::Close(None);
                        sender.send_message(&message).unwrap();
                        println!("Client {} disconnected", ip);
                        return;
                    }
                    OwnedMessage::Ping(ping) => {
                        let message = OwnedMessage::Pong(ping);
                        sender.send_message(&message).unwrap();
                    }
                    _ => {
                        let v = matrix.lock().unwrap();
                        serde_json::to_string(&v.clone())
                            .map(|s| {
                                sender.send_message(OwnedMessage::Text(s).borrow()).unwrap();
                            })
                            .unwrap()
                    }
                }
            }
        });
    }
}

fn start_set_thermal(temp_matrix: TemperatureList) {
    thread::spawn(|| {
        set_thermal(temp_matrix);
    });
}


fn main() {
    let temp_matrix: TemperatureList = Arc::new(Mutex::new(vec![]));
    start_set_thermal(Arc::clone(&temp_matrix));
    start_web_socket(temp_matrix);
}
