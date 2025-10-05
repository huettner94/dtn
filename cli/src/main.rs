// Copyright (C) 2023 Felix Huettner
//
// This file is part of DTRD.
//
// DTRD is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// DTRD is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::io::Write;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};
use dtrd_client::Client;
use futures_util::StreamExt;
use tabular::{Table, row};
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
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
    Node {
        #[clap(subcommand)]
        command: NodeCommands,
    },
    Route {
        #[clap(subcommand)]
        command: RouteCommands,
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
        #[clap(long, help = "If bundle should be traced", required = false)]
        debug: bool,
    },
    Listen {
        #[clap(short, long, help = "The endpoint to listen on")]
        endpoint: String,
        #[clap(value_enum,
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

#[derive(Subcommand)]
enum NodeCommands {
    List,
    Add {
        #[clap(short, long, help = "The remote address of the target node")]
        address: String,
    },
    Remove {
        #[clap(short, long, help = "The remote address of the target node")]
        address: String,
    },
}

#[derive(Subcommand)]
enum RouteCommands {
    List,
    Add {
        #[clap(short, long, help = "The target the route should apply to")]
        target: String,
        #[clap(short, long, help = "The next hop of the traffic")]
        nexthop: String,
    },
    Remove {
        #[clap(short, long, help = "The target the route should apply to")]
        target: String,
        #[clap(short, long, help = "The next hop of the traffic")]
        nexthop: String,
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
                format!("Error using url to connect to DTRD: {e:?}"),
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
                debug,
            } => {
                command_bundle_submit(&mut client, destination, lifetime, data, data_file, debug)
                    .await;
            }
            BundleCommands::Listen {
                endpoint,
                output_mode,
            } => command_bundle_listen(&mut client, endpoint, output_mode).await,
            BundleCommands::Receive { endpoint, file } => {
                command_bundle_receive(&mut client, endpoint, file).await;
            }
        },
        Commands::Node { command } => match command {
            NodeCommands::List => command_node_list(&mut client).await,
            NodeCommands::Add { address } => command_node_add(&mut client, address).await,
            NodeCommands::Remove { address } => command_node_remove(&mut client, address).await,
        },
        Commands::Route { command } => match command {
            RouteCommands::List => command_route_list(&mut client).await,
            RouteCommands::Add { target, nexthop } => {
                command_route_add(&mut client, target, nexthop).await;
            }
            RouteCommands::Remove { target, nexthop } => {
                command_route_remove(&mut client, target, nexthop).await;
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
    debug: bool,
) {
    if data.is_none() == data_file.is_none() {
        let mut cmd = Cli::command();
        cmd.error(
            ErrorKind::ArgumentConflict,
            "Either data or data_file must be specified",
        )
        .exit();
    }
    let payload = if let Some(data) = data {
        data.as_bytes().to_vec()
    } else {
        fs::read(data_file.as_ref().unwrap())
            .await
            .map_err(|e| {
                let mut cmd = Cli::command();
                cmd.error(
                    ErrorKind::InvalidValue,
                    format!("Error reading data from file: {e:?}"),
                )
                .exit();
            })
            .unwrap()
    };
    match client
        .submit_bundle(&destination, lifetime, &payload, debug)
        .await
    {
        Ok(()) => {
            println!("Bundle submitted successfully");
        }
        Err(e) => {
            println!("Error submitting bundle: {e:?}");
        }
    }
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
                                    println!("Successfully parsed administrative record: {ar:?}");
                                }
                                Err(_) => {
                                    println!(
                                        "Is no administrative record. This is the output as string.\n<<<BEGIN\n{}\n<<<END",
                                        String::from_utf8_lossy(&data)
                                    );
                                }
                            }
                        }
                        OutputMode::Hex => println!("Received bundle: {data:?}"),
                        OutputMode::Raw => {
                            let mut stdout = std::io::stdout();
                            stdout.write_all(&data).unwrap();
                            stdout.flush().unwrap();
                        }
                    },
                    Err(e) => {
                        println!("Error receiving bundle: {e:?}");
                        break;
                    }
                }
            }
            println!("Server closed the connection");
        }
        Err(e) => {
            println!("Error listening for bundles: {e:?}");
        }
    }
}

async fn command_bundle_receive(client: &mut Client, endpoint: String, file: Option<String>) {
    match client.receive_bundle(&endpoint).await {
        Ok(data) => {
            if let Some(path) = file {
                fs::write(path, data).await.unwrap();
            } else {
                let mut stdout = std::io::stdout();
                stdout.write_all(&data).unwrap();
                stdout.flush().unwrap();
            }
        }
        Err(e) => {
            println!("Error receiving bundle: {e:?}");
        }
    }
}

async fn command_node_list(client: &mut Client) {
    match client.list_nodes().await {
        Ok(data) => {
            let mut table = Table::new("{:<}  {:<}  {:<}  {:<}");
            table.add_row(row!("URL", "Status", "Endpoint", "Temporary"));
            for node in data {
                table.add_row(row!(
                    node.url,
                    node.status,
                    node.endpoint,
                    if node.temporary { "temporary" } else { "" }
                ));
            }
            print!("{table}");
        }
        Err(e) => {
            println!("Error receiving node list: {e:?}");
        }
    }
}

async fn command_node_add(client: &mut Client, url: String) {
    match client.add_node(url).await {
        Ok(()) => {}
        Err(e) => {
            println!("Error adding node: {e:?}");
        }
    }
}

async fn command_node_remove(client: &mut Client, url: String) {
    match client.remove_node(url).await {
        Ok(()) => {}
        Err(e) => {
            println!("Error adding node: {e:?}");
        }
    }
}

async fn command_route_list(client: &mut Client) {
    match client.list_routes().await {
        Ok(data) => {
            let mut table = Table::new("{:<}  {:<}  {:<}  {:<}  {:<}  {:<}");
            table.add_row(row!(
                "Target",
                "Nexthop",
                "Status",
                "Prefrered",
                "Available",
                "Bundle size limit"
            ));
            for route in data {
                table.add_row(row!(
                    &route.route.as_ref().unwrap().target,
                    &route.route.as_ref().unwrap().next_hop,
                    route.r#type().as_str_name(),
                    route.preferred,
                    route.available,
                    route.max_bundle_size
                ));
            }
            print!("{table}");
        }
        Err(e) => {
            println!("Error receiving route list: {e:?}");
        }
    }
}

async fn command_route_add(client: &mut Client, target: String, nexthop: String) {
    match client.add_route(target, nexthop).await {
        Ok(()) => {}
        Err(e) => {
            println!("Error adding route: {e:?}");
        }
    }
}

async fn command_route_remove(client: &mut Client, target: String, nexthop: String) {
    match client.remove_route(target, nexthop).await {
        Ok(()) => {}
        Err(e) => {
            println!("Error adding route: {e:?}");
        }
    }
}
