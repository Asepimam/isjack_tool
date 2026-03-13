use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SimLog {
    pub ts: String,
    pub dir: LogDir,
    pub raw_hex: String,
    pub summary: String,
}

#[derive(Clone, PartialEq)]
pub enum LogDir {
    Recv,
    Send,
    Info,
    Error,
}

impl LogDir {
    pub fn label(&self) -> &'static str {
        match self {
            LogDir::Recv => "▼ RECV",
            LogDir::Send => "▲ SEND",
            LogDir::Info => "INFO",
            LogDir::Error => "ERROR",
        }
    }
}

pub struct SimState {
    pub logs: Vec<SimLog>,
    pub running: bool,
    pub connection_count: usize,
    pub tx_count: usize,
}

impl SimState {

    pub fn new() -> Self {
        Self {
            logs: Vec::new(),
            running: false,
            connection_count: 0,
            tx_count: 0,
        }
    }

    pub fn log(&mut self, dir: LogDir, raw_hex: &str, summary: &str) {

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let secs = now.as_secs() % 86400;

        let hh = secs / 3600;
        let mm = (secs % 3600) / 60;
        let ss = secs % 60;

        self.logs.push(SimLog {
            ts: format!("{:02}:{:02}:{:02}", hh, mm, ss),
            dir,
            raw_hex: raw_hex.to_string(),
            summary: summary.to_string(),
        });

        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
    }
}

#[derive(Clone)]
pub enum Framing {
    Binary2,
    Ascii4,
    None,
}

pub enum ServerCmd {
    Stop,
}

pub struct SimServer {
    pub state: Arc<Mutex<SimState>>,
    cmd_tx: Option<Sender<ServerCmd>>,
}

impl SimServer {

    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(SimState::new())),
            cmd_tx: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.state.lock().map(|s| s.running).unwrap_or(false)
    }

    pub fn start(&mut self, port: u16, framing: Framing, rules: AutoResponse) {
        let state = Arc::clone(&self.state);
        self.start_with_state(port, framing, rules, state);
    }

    pub fn start_with_state(
        &mut self,
        port: u16,
        framing: Framing,
        rules: AutoResponse,
        state: Arc<Mutex<SimState>>,
    ) {

        let (tx, rx):(Sender<ServerCmd>,Receiver<ServerCmd>) = channel();
        self.cmd_tx = Some(tx);
        self.state = Arc::clone(&state);

        thread::spawn(move || {

            let addr = format!("0.0.0.0:{}", port);
            let listener = TcpListener::bind(&addr).unwrap();
            listener.set_nonblocking(true).ok();

            if let Ok(mut s) = state.lock() {
                s.running = true;
                s.log(LogDir::Info,"",&format!("Server listening on {}",addr));
            }

            loop {

                if rx.try_recv().is_ok() {
                    break;
                }

                match listener.accept() {

                    Ok((stream, peer)) => {

                        let state2 = Arc::clone(&state);
                        let frame2 = framing.clone();
                        let rules2 = rules.clone();

                        if let Ok(mut s) = state.lock() {
                            s.connection_count += 1;
                            s.log(LogDir::Info,"",&format!("Connection {}",peer));
                        }

                        thread::spawn(move || {
                            handle_connection(stream,state2,frame2,rules2);
                        });

                    }

                    Err(ref e) if e.kind()==std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(100));
                    }

                    Err(_) => break
                }
            }

            if let Ok(mut s)=state.lock(){
                s.running=false;
                s.log(LogDir::Info,"","Server stopped");
            }

        });

    }

    pub fn stop(&mut self){
        if let Some(tx)=&self.cmd_tx{
            let _ = tx.send(ServerCmd::Stop);
        }
    }
}

#[derive(Clone)]
pub struct AutoResponse;

impl AutoResponse {

    pub fn default_rules() -> Self {
        AutoResponse
    }

    pub fn build_response(&self, req_hex:&str)->Option<String>{

        if req_hex.len()<8{
            return None;
        }

        let mti_hex = &req_hex[0..8];

        let mti_bytes:Vec<u8>=(0..8)
            .step_by(2)
            .filter_map(|i|u8::from_str_radix(&mti_hex[i..i+2],16).ok())
            .collect();

        let mti = String::from_utf8_lossy(&mti_bytes);

        let resp_mti = match mti.as_ref() {
            "0200"=>"0210",
            "0400"=>"0410",
            "0800"=>"0810",
            _=>"0210",
        };

        let resp_mti_hex:String =
            resp_mti.bytes().map(|b|format!("{:02X}",b)).collect();

        let mut resp = format!("{}{}",resp_mti_hex,&req_hex[8..]);

        resp.push_str("3030");

        Some(resp)
    }
}

fn bytes_to_hex(data:&[u8])->String{
    data.iter().map(|b|format!("{:02X}",b)).collect()
}

fn extract_mti_summary(data:&[u8])->String{

    if data.len()<4{
        return format!("[{} bytes]",data.len());
    }

    let mti:String=data[0..4]
        .iter()
        .map(|&b| if b.is_ascii_graphic(){b as char}else{'?'})
        .collect();

    if mti.chars().all(|c|c.is_ascii_digit()){
        format!("MTI={}",mti)
    }else{
        "MTI=?".to_string()
    }
}

fn read_framed(stream:&mut TcpStream,framing:&Framing)->Result<Vec<u8>,String>{

    match framing {

        Framing::Binary2 => {

            let mut len_buf=[0u8;2];
            stream.read_exact(&mut len_buf).map_err(|e|e.to_string())?;

            let len=u16::from_be_bytes(len_buf) as usize;

            let mut data=vec![0u8;len];
            stream.read_exact(&mut data).map_err(|e|e.to_string())?;

            Ok(data)
        }

        Framing::Ascii4 => {

            let mut len_buf=[0u8;4];
            stream.read_exact(&mut len_buf).map_err(|e|e.to_string())?;

            let len_str=String::from_utf8_lossy(&len_buf);
            let len=len_str.trim().parse::<usize>().unwrap_or(0);

            let mut data=vec![0u8;len];
            stream.read_exact(&mut data).map_err(|e|e.to_string())?;

            Ok(data)
        }

        Framing::None => {

            let mut buf=vec![0u8;4096];
            let n=stream.read(&mut buf).map_err(|e|e.to_string())?;
            buf.truncate(n);

            Ok(buf)
        }
    }
}

fn write_framed(stream:&mut TcpStream,data:&[u8],framing:&Framing)->Result<(),String>{

    match framing {

        Framing::Binary2 => {

            let len=(data.len() as u16).to_be_bytes();
            stream.write_all(&len).map_err(|e|e.to_string())?;
            stream.write_all(data).map_err(|e|e.to_string())?;
        }

        Framing::Ascii4 => {

            let header=format!("{:04}",data.len());
            stream.write_all(header.as_bytes()).map_err(|e|e.to_string())?;
            stream.write_all(data).map_err(|e|e.to_string())?;
        }

        Framing::None => {
            stream.write_all(data).map_err(|e|e.to_string())?;
        }
    }

    stream.flush().map_err(|e|e.to_string())
}

fn handle_connection(
    mut stream:TcpStream,
    state:Arc<Mutex<SimState>>,
    framing:Framing,
    rules:AutoResponse
){

    loop {

        let data = match read_framed(&mut stream,&framing){
            Ok(d) if d.is_empty()=>break,
            Ok(d)=>d,
            Err(_)=>break
        };

        let hex=bytes_to_hex(&data);
        let summary=extract_mti_summary(&data);

        if let Ok(mut s)=state.lock(){
            s.tx_count+=1;
            s.log(LogDir::Recv,&hex,&summary);
        }

        if let Some(resp_hex)=rules.build_response(&hex){

            let resp_bytes:Vec<u8>=(0..resp_hex.len())
                .step_by(2)
                .filter_map(|i|u8::from_str_radix(&resp_hex[i..i+2],16).ok())
                .collect();

            if write_framed(&mut stream,&resp_bytes,&framing).is_err(){
                break;
            }

            if let Ok(mut s)=state.lock(){
                s.log(LogDir::Send,&resp_hex,"AUTO RESPONSE");
            }
        }
    }
}

pub fn send_message(
    host:&str,
    port:u16,
    message_hex:&str,
    framing_type:&str
)->Result<(String,String),String>{

    let addr=format!("{}:{}",host,port);

    let mut stream=TcpStream::connect(&addr)
        .map_err(|e|format!("connect error {}",e))?;

    let framing=match framing_type{
        "binary2"=>Framing::Binary2,
        "ascii4"=>Framing::Ascii4,
        _=>Framing::None
    };

    let clean:String=message_hex
        .chars()
        .filter(|c|!c.is_whitespace())
        .collect();

    let msg_bytes:Vec<u8>=(0..clean.len())
        .step_by(2)
        .filter_map(|i|u8::from_str_radix(&clean[i..i+2],16).ok())
        .collect();

    write_framed(&mut stream,&msg_bytes,&framing)?;

    let resp=read_framed(&mut stream,&framing)?;

    let resp_hex=bytes_to_hex(&resp);
    let summary=extract_mti_summary(&resp);

    Ok((resp_hex,summary))
}

pub fn format_logs(logs:&[SimLog])->String{

    if logs.is_empty(){
        return "(no activity)\n".to_string();
    }

    let mut out=String::new();

    for log in logs.iter().rev().take(200){

        let preview=if log.raw_hex.len()>60{
            format!("{}…",&log.raw_hex[..60])
        }else{
            log.raw_hex.clone()
        };

        out.push_str(&format!(
            "[{}] {} {}\n",
            log.ts,
            log.dir.label(),
            log.summary
        ));

        if !log.raw_hex.is_empty(){
            out.push_str(&format!("           {}\n",preview));
        }
    }

    out
}