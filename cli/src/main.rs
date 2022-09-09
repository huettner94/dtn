use std::io::Write;

use clap::{ArgEnum, ErrorKind, IntoApp, Parser, Subcommand};
use dtrd_client::Client;
use futures_util::StreamExt;
use tokio::fs;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(
        short,
        long,
        help = "The grpc endpoint of the DTRD",
        default_value = "http://localhost:50051",
        global = true
    )]
    url: String,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum OutputMode {
    Parse,
    Hex,
    Raw,
}

#[derive(Subcommand)]
enum Commands {
    Bundle {
        #[clap(subcommand)]
        command: BundleCommands,
    },
}

#[derive(Subcommand)]
enum BundleCommands {
    Submit {
        #[clap(short, long, help = "The destination url of the bundle")]
        destination: String,
        #[clap(
            short,
            long,
            default_value_t = 3600,
            help = "The lifetime of the bundle in seconds"
        )]
        lifetime: u64,
        #[clap(long, help = "The data to be sent (as a string)")]
        data: Option<String>,
        #[clap(
            short = 'f',
            long,
            help = "The data to be sent (read from the specified file)"
        )]
        data_file: Option<String>,
    },
    Listen {
        #[clap(short, long, help = "The endpoint to listen on")]
        endpoint: String,
        #[clap(arg_enum,
            short,
            long,
            help = "Parse the bundles and prette-print them",
            default_value_t = OutputMode::Parse
        )]
        output_mode: OutputMode,
    },
    Receive {
        #[clap(short, long, help = "The endpoint to listen on")]
        endpoint: String,
        #[clap(short, long, help = "The file to write the bundle to")]
        file: Option<String>,
    },
}

#[tokio::main]
pub async fn main() {
    let cli = Cli::parse();

    let mut client = Client::new(&cli.url)
        .await
        .map_err(|e| {
            let mut cmd = Cli::command();
            cmd.error(
                ErrorKind::InvalidValue,
                format!("Error using url to connect to DTRD: {:?}", e),
            )
            .exit();
        })
        .unwrap();

    match cli.command {
        Commands::Bundle { command } => match command {
            BundleCommands::Submit {
                destination,
                lifetime,
                data,
                data_file,
            } => {
                command_bundle_submit(&mut client, destination, lifetime, data, data_file).await;
            }
            BundleCommands::Listen {
                endpoint,
                output_mode,
            } => command_bundle_listen(&mut client, endpoint, output_mode).await,
            BundleCommands::Receive { endpoint, file } => {
                command_bundle_receive(&mut client, endpoint, file).await
            }
        },
    }
}

async fn command_bundle_submit(
    client: &mut Client,
    destination: String,
    lifetime: u64,
    data: Option<String>,
    data_file: Option<String>,
) {
    if data.is_none() == data_file.is_none() {
        let mut cmd = Cli::command();
        cmd.error(
            ErrorKind::ArgumentConflict,
            "Either data or data_file must be specified",
        )
        .exit();
    }
    let payload = if data.is_some() {
        data.unwrap().as_bytes().to_vec()
    } else {
        fs::read(data_file.as_ref().unwrap())
            .await
            .map_err(|e| {
                let mut cmd = Cli::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    format!("Error reading data from file: {:?}", e),
                )
                .exit();
            })
            .unwrap()
    };
    match client.submit_bundle(&destination, lifetime, &payload).await {
        Ok(_) => {
            println!("Bundle submitted successfully");
        }
        Err(e) => {
            println!("Error submitting bundle: {:?}", e);
        }
    };
}

async fn command_bundle_listen(client: &mut Client, endpoint: String, output_mode: OutputMode) {
    match client.listen_bundles(&endpoint).await {
        Ok(mut stream) => {
            println!("Now listening for bundles. Press CTRL+C to abort");
            while let Some(data) = stream.next().await {
                match data {
                    Ok(data) => match output_mode {
                        OutputMode::Parse => {
                            match bp7::administrative_record::AdministrativeRecord::try_from(&data)
                            {
                                Ok(ar) => {
                                    println!("Successfully parsed administrative record: {:?}", ar);
                                }
                                Err(_) => {
                                    println!("Is no administrative record. This is the output as string.\n<<<BEGIN\n{}\n<<<END", String::from_utf8_lossy(&data));
                                }
                            }
                        }
                        OutputMode::Hex => println!("Received bundle: {:?}", data),
                        OutputMode::Raw => {
                            let mut stdout = std::io::stdout();
                            stdout.write(&data).unwrap();
                            stdout.flush().unwrap();
                        }
                    },
                    Err(e) => {
                        println!("Error receiving bundle: {:?}", e);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            println!("Error listening for bundles: {:?}", e);
        }
    }
}

async fn command_bundle_receive(client: &mut Client, endpoint: String, file: Option<String>) {
    match client.receive_bundle(&endpoint).await {
        Ok(data) => match file {
            Some(path) => {
                fs::write(path, data).await.unwrap();
            }
            None => {
                let mut stdout = std::io::stdout();
                stdout.write(&data).unwrap();
                stdout.flush().unwrap()
            }
        },
        Err(e) => {
            println!("Error receiving bundle: {:?}", e);
        }
    }
}
