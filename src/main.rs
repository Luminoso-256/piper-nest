use eframe::{egui, epi};
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::{fs, thread};
use std::net::{TcpStream, Shutdown};
use rfd::FileDialog;
use std::sync::mpsc;
use std::path::PathBuf;

use std::sync::mpsc::{Sender, Receiver, RecvError};

struct Gemline{
    content:String,
    rendertype:GemRenderType,
    metadata:String
}
enum GemRenderType{
    NORMAL,
    HEADING,
    SUBHEADING,
    SUBSUBHEADING,
    LIST,
    QUOTE,
    LINK,
    MONOSPACE
}

struct Nest {
    url:String,
    current_content_type:u8,
    current_response_bytes:Vec<u8>,
    current_gemtxt_pg:Vec<Gemline>,
    sender:Sender<String>,
    receiver:Receiver<QueryResponse>,
    is_waiting_on_query:bool
}

struct QueryResponse{
    contenttype:u8,
    data:Vec<u8>
}

impl Default for Nest {
    fn default() -> Self {
        let (s1, r1) = mpsc::channel();
        let (s2, r2) = mpsc::channel();
        thread::spawn(move || {
            loop {
                let mut path:String = "".to_string();
                loop {
                    match r1.recv() {
                        Ok(r) => {
                            path = r;
                            //println!("running simulation");
                            break;
                        }
                        Err(_) => {}
                    }
                }

                let mut res = QueryResponse{
                    contenttype:0,
                    data: vec![]
                };
                let mut cuprt:Vec<&str> = path.split("://").collect();
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
                res.contenttype = received[0];
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
                res.data = received;
                stream.shutdown(Shutdown::Both);
                s2.send(res).unwrap();
            }
        });
        println!("Initialized Browser Thread");
        Self {
            url: "".to_string(),
            current_content_type: 0xFF,
            current_response_bytes: vec![],
            current_gemtxt_pg: vec![],
            sender: s1,
            receiver: r2,
            is_waiting_on_query: false
        }
    }
}

impl epi::App for Nest {
    fn name(&self) -> &str {
        "Nest"
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        let Self { .. } = self;

        //check thread results
        let mut has_new_data = true;

        let data = match self.receiver.try_recv(){
            Ok(res) => res,
            Err(_) => {has_new_data = false; QueryResponse{ contenttype: 0, data: vec![] }}
        };

        //gemini parsing.
        //while 0x01 is the proper content type for gemini, internally to help manage state we'll take advantage of the 0xE series
        //content type 0x01 is data waiting to be parsed, 0xE0 is ready to display Gemini.
        if self.current_content_type == 0x01{
            self.current_gemtxt_pg.clear();
            let lines:Vec<&str> = std::str::from_utf8(&*self.current_response_bytes).unwrap().split("\n").collect();
            let mut in_monospace_mode = false;
            for line in lines.into_iter(){
                //check for each gemtxt directive in turn

                //start with monospace directive, since it governs if the others work or not
                if line.starts_with("```"){
                    in_monospace_mode = !in_monospace_mode;
                    continue;
                }
                //then do the rest
                if in_monospace_mode{
                    self.current_gemtxt_pg.push(Gemline{
                        content: line.clone().parse().unwrap(),
                        rendertype: GemRenderType::MONOSPACE,
                        metadata: "".to_string()
                    });
                } else{
                    if line.starts_with("# "){
                        self.current_gemtxt_pg.push(Gemline{
                            content: line.replace("# ",""),
                            rendertype: GemRenderType::HEADING,
                            metadata: "".to_string()
                        });
                    } else if line.starts_with("## "){
                        self.current_gemtxt_pg.push(Gemline{
                            content: line.replace("## ",""),
                            rendertype: GemRenderType::SUBHEADING,
                            metadata: "".to_string()
                        });
                    } else if line.starts_with("### "){
                        self.current_gemtxt_pg.push(Gemline{
                            content: line.replace("### ",""),
                            rendertype: GemRenderType::SUBSUBHEADING,
                            metadata: "".to_string()
                        });
                    } else if line.starts_with("> "){
                        self.current_gemtxt_pg.push(Gemline{
                            content: line.replace("> ",""),
                            rendertype: GemRenderType::QUOTE,
                            metadata: "".to_string()
                        });
                    } else if line.starts_with("* "){
                        self.current_gemtxt_pg.push(Gemline{
                            content: line.replace("* ",""),
                            rendertype: GemRenderType::LIST,
                            metadata: "".to_string()
                        });
                    } else if line.starts_with("=> "){
                        //TODO: Properly parse links!!
                        let clean = line.replace("=> ","");
                        let mut parts:Vec<&str> = clean.split(" ").collect();
                        let url = parts[0];
                        parts.remove(0);
                        let text = parts.join(" ");

                        self.current_gemtxt_pg.push(Gemline{
                            content: text,
                            rendertype: GemRenderType::LINK,
                            metadata: url.parse().unwrap()
                        });
                    } else {
                        self.current_gemtxt_pg.push(Gemline{
                            content: line.clone().parse().unwrap(),
                            rendertype: GemRenderType::NORMAL,
                            metadata: "".to_string()
                        });
                    }
                }

            }
            self.current_content_type = 0xE0;
            self.current_response_bytes.clear();
        }

        if has_new_data {
            self.current_response_bytes = data.data;
            self.current_content_type = data.contenttype;
            self.is_waiting_on_query = false;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add_sized([500.,15.], egui::TextEdit::singleline(&mut self.url));
                if ui.button("Browse!").clicked() {
                    self.is_waiting_on_query = true;
                    self.sender.send(self.url.clone());
                }
                if !self.is_waiting_on_query {
                    if self.current_content_type == 0xE0{
                        //hardcoded hack for making things "look right"
                        ui.label("Response Content Type: 0x01");
                    } else {
                        ui.label(format!("Response Content Type: 0x{:X}", self.current_content_type));
                    }
                } else{
                    ui.label("Loading...");
                }
            });
            match self.current_content_type{
                0x00 => {
                    //ui.label(std::str::from_utf8(&*self.current_response_bytes).unwrap());
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.text_edit_multiline(& mut std::str::from_utf8(&*self.current_response_bytes).unwrap());
                    });
                },
                0x01 => {
                    ui.label("Parsing Gemtxt...");
                }
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
                0xE0 => {
                    for gmln in self.current_gemtxt_pg.iter(){
                        match gmln.rendertype{
                            GemRenderType::NORMAL => {ui.label(gmln.content.clone());},
                            GemRenderType::HEADING => {ui.add(egui::Label::new(gmln.content.clone()).strong().text_style(egui::TextStyle::Heading));},
                            GemRenderType::SUBHEADING =>{ui.add(egui::Label::new(gmln.content.clone()).strong().text_style(egui::TextStyle::Button));},
                            GemRenderType::SUBSUBHEADING => {ui.strong(gmln.content.clone());},
                            GemRenderType::LIST => {ui.label(format!("• {}",gmln.content.clone()));},
                            GemRenderType::QUOTE => {ui.add(egui::Label::new(gmln.content.clone()).italics().strong());},
                            GemRenderType::LINK => {
                                if ui.add(egui::Label::new(gmln.content.clone()).sense(egui::Sense::click())).clicked() {
                                  //check what type of URL it is
                                    if gmln.metadata.starts_with("piper://"){
                                        self.is_waiting_on_query = true;
                                        self.sender.send(gmln.metadata.clone());
                                    } else {
                                        //TODO: Open in sys browser or smth?
                                    }
                                }
                            },
                            GemRenderType::MONOSPACE => {ui.code(gmln.content.clone());}
                        };
                    }
                }
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