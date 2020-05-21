#![feature(proc_macro_hygiene, decl_macro)]

use serde::Serialize;

// use for websocket
extern crate websocket;
use websocket::OwnedMessage;

// use for http
#[macro_use] extern crate rocket;
use rocket::request::Request;
use rocket::http::{Header, ContentType};
use rocket::response::{ self, Response, Responder};
use rocket::{Config, State};
use rocket::http::Status;
use rocket::config::Environment;


extern crate i2cdev;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::io::Cursor;
use std::borrow::Borrow;


type TemperatureList = Arc<Mutex<Vec<Vec<f32>>>>;
#[derive(Serialize)]
struct TemperatureData {
    list: Vec<Vec<f32>>,
}

#[cfg(not(any(target_os = "linux")))]
fn set_thermal(t: TemperatureList) {
    use std::time::{SystemTime, UNIX_EPOCH};
    loop {
        thread::sleep(Duration::from_millis(100)); // sleep 0.1 second
        let now = SystemTime::now();
        let ms = (now.duration_since(UNIX_EPOCH).unwrap().as_millis() % 100_000) as f32 / 1000.0;
        let mut values = t.lock().unwrap();
        *values = vec![
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7],
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7],
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7],
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7],
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7],
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7],
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7],
            vec![ms, ms + 0.1, ms + 0.2, ms+ 0.3, ms+0.4, ms+0.5, ms+0.6, ms+0.7]
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


#[derive(Responder)]
#[response(status = 200, content_type = "json")]
struct JsonResponderWithHeader {
    inner: TemperatureData,
    header: ContentType,
    acc_origin: Header<'static>,
    acc_header: Header<'static>,
    acc_method: Header<'static>,
}

#[get("/")]
fn index(t_list: State<TemperatureList>) -> TemperatureData {
    let list = Arc::clone(&t_list);
    let values = list.lock().unwrap();
    TemperatureData {
        list: values.clone(),
    }
}

impl<'a> Responder<'a> for TemperatureData {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        serde_json::to_string(&self).map(| s| {
            Response::build()
                .header(ContentType::JSON)
                .header(Header::new("Access-Control-Allow-Origin", "*"))
                .sized_body(Cursor::new(s)).finalize()
        }).map_err(|_e| {
            Status::InternalServerError
        })

    }
}

fn main() {
    let temp_matrix: TemperatureList = TemperatureList::new(Mutex::new(vec![]));
    {
        let matrix = Arc::clone(&temp_matrix);
        thread::spawn(move || set_thermal(matrix));
    }
    let ws = websocket::sync::Server::bind("0.0.0.0:8102").unwrap();
    {
        for request in ws.filter_map(Result::ok) {
            let matrix = Arc::clone(&temp_matrix);
            thread::spawn(move || {
                if !request.protocols().contains(&"thermal-rs".to_string()) {
                    request.reject().unwrap();
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
                            return
                        }
                        OwnedMessage::Ping(ping) => {
                            let message = OwnedMessage::Pong(ping);
                            sender.send_message(&message).unwrap();
                        }
                        _ => {
                            let v = matrix.lock().unwrap();
                            serde_json::to_string(&v.clone()).map(|s| {
                                sender.send_message(OwnedMessage::Text(s).borrow()).unwrap();
                            }).unwrap()
                        }
                    }
                }
            });
        }
    }
    let config = Config::build(Environment::Staging)
        .address("0.0.0.0")
        .port(8101)
        .finalize()
        .unwrap();

    rocket::custom(config)
        .manage(temp_matrix)
        .mount("/", routes![index])
        .launch();
}
