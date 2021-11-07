use eframe::{egui, epi};
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::fs;
use std::net::{TcpStream, Shutdown};
use rfd::FileDialog;
use std::path::PathBuf;

struct Nest {
    url:String,
    current_content_type:u8,
    current_response_bytes:Vec<u8>
}

impl Default for Nest {
    fn default() -> Self {
        Self {
            url: "".to_string(),
            current_content_type: 0xFF,
            current_response_bytes: vec![],
        }
    }
}

impl epi::App for Nest {
    fn name(&self) -> &str {
        "Nest"
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        let Self { .. } = self;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized([500.,15.], egui::TextEdit::singleline(&mut self.url));
                if ui.button("Browse!").clicked() {
                    let mut cuprt:Vec<&str> = self.url.split("://").collect();
                    let mut parts:Vec<&str> = cuprt[1].split("/").collect();
                    //build socket
                    let hostname = format!("{}:60",parts[0]);
                    let mut stream = TcpStream::connect(hostname).unwrap();
                    //build request
                    let mut request:Vec<u8> = vec![];
                    parts.remove(0); //yeet the hostname!
                    let uri = format!("/{}",parts.join("/"));
                    request.extend((uri.len() as u16).to_le_bytes());
                    request.extend(uri.into_bytes());
                    //send request
                    let _bwrit = stream.write(&*request);
                    stream.flush();
                    //read response
                    let mut reader = io::BufReader::new(&mut stream);
                    let mut received: Vec<u8> = reader.fill_buf().unwrap().to_vec();
                    reader.consume(received.len());
                    self.current_content_type = received[0];
                    received.remove(0);
                    //discard content len, we don't need it.
                    //(unelegantly)
                    received.remove(0);
                    received.remove(0);
                    received.remove(0);
                    received.remove(0);
                    received.remove(0);
                    received.remove(0);
                    received.remove(0);
                    received.remove(0);
                    self.current_response_bytes = received;

                    stream.shutdown(Shutdown::Both);
                }
                ui.label(format!("Response Content Type: 0x{:X}",self.current_content_type));
            });
            match self.current_content_type{
                0x00 => {
                    //ui.label(std::str::from_utf8(&*self.current_response_bytes).unwrap());
                    ui.text_edit_multiline(& mut std::str::from_utf8(&*self.current_response_bytes).unwrap());
                },
                0x10 | 0x11 | 0x12 => {
                    ui.label("File is ready for Download:");
                    if ui.button("Download").clicked(){
                        let files = FileDialog::new().save_file().unwrap();
                        let mut file = File::create(files).unwrap();
                        file.write_all(&*self.current_response_bytes);
                        self.current_content_type = 0xF1;
                        self.current_response_bytes.clear();
                    }
                }
                0x22 => {
                    ui.label("0x22 Resource Not Found");
                },
                0xF1 => {
                    ui.label("Finished file download");
                },
                0xFF => {
                    ui.label("Awaiting browse activity.");
                },
                _ => {
                    ui.label("Unknown Content Type!");
                }
            }
        });
        // Resize the native window to be just the size we need it to be:
        frame.set_window_size(ctx.used_size());
    }
}

fn main() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(Nest::default()), options);
}