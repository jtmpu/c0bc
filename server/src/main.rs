use std::error::Error;
use structopt::StructOpt;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(short, long)]
    verbose: bool,
    #[structopt(short, long, default_value = "0.0.0.0:9999")]
    address: String,
}

async fn process_user_io(tx: broadcast::Sender<String>, mut rx: broadcast::Receiver<String>) {
    let input = io::stdin();
    let mut input_lines = BufReader::new(input).lines();
    let mut output = tokio::io::stdout();
    loop {
        tokio::select! {
            io_input_res = input_lines.next_line() => {
                tracing::info!("[process_user_io] received user input");
                let line = match io_input_res {
                    Ok(Some(x)) => x,
                    _ => break,
                };
                let _ = tx.send(line);
            }
            node_input_res = rx.recv() => {
                tracing::info!("[process_user_io] received network input");
                let data = match node_input_res {
                    Ok(val) => val,
                    _ => break,
                };
                let _ = output.write_all(data.as_bytes()).await;
            }
        };
    }
}

async fn process_socket(
    mut socket: TcpStream,
    mut io_rx: broadcast::Receiver<String>,
    node_tx: broadcast::Sender<String>,
) -> Result<(), Box<dyn Error>> {
    loop {
        let mut buf = [0; 1024];
        tokio::select! {
            io_input = io_rx.recv() => {
                tracing::info!("[process_socket] received user input");
                let line = io_input?;
                socket.write_all(line.as_bytes()).await?;
            }
            net_input = socket.read(&mut buf) => {
                tracing::info!("[process_socket] received network input");
                let n = net_input?;
                if n == 0 {
                    return Ok(());
                }
                let resp: String = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = node_tx.send(resp)?;
            }
        };
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
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

    let (io_tx, _) = broadcast::channel::<String>(16);
    let (node_tx, _) = broadcast::channel::<String>(16);
    let handle = tokio::spawn(process_user_io(io_tx.clone(), node_tx.subscribe()));

    let listener = TcpListener::bind(opts.address).await?;
    let (socket, addr) = listener.accept().await?;
    tracing::info!("Received connection from {}", addr);
    process_socket(socket, io_tx.subscribe(), node_tx.clone()).await?;
    tracing::info!("Connection terminated");

    std::mem::drop(handle);
    std::process::exit(0);
}
