use clap::Parser;
use std::io::Write;
use std::os::unix::net::UnixStream;

#[derive(Parser)]
struct Args {
  action: String,
}

fn get_op (name: &str) -> Option<u8> {
  match name {
    "show" => Some (common::OPCODE_SHOW),
    "stop" | "stop-server" => Some (common::OPCODE_STOP),
    "reload" | "rebuild" | "rebuild-cache" => Some (common::OPCODE_REBUILD_CACHE),
    _ => None,
  }
}

fn main () {
  let args = Args::parse ();
  let op = if let Some (op) = get_op (&args.action) {
    op
  } else {
    eprintln! ("invalid action: {}", args.action);
    return;
  };
  let mut sock = match UnixStream::connect (common::SOCKET_PATH) {
    Ok (sock) => sock,
    Err (_) => {
      eprintln! ("server not running");
      return;
    }
  };
  if let Err (error) = sock.write (&[op]) {
    eprintln! ("warning: socket writing error: {error}");
  }
}
