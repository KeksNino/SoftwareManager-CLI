use aria2_ws::{Callbacks, Client, TaskOptions};
use clap::Parser;
use dialoguer::FuzzySelect;
use futures::FutureExt;
use owo_colors::OwoColorize;
use select::document::Document;
use select::predicate::Name;
use serde::Deserialize;
use serde_json::json;
use serde_json::Value;
use std::io;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use tokio::{spawn, sync::Semaphore};

#[derive(Debug, Deserialize)]
struct Software {
    author: String,
    title: String,
    url: String,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 0)]
    speed_limit: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let ascii = r#"
  _________       _____  __                                     _____                                             
 /   _____/ _____/ ____\/  |___  _  _______ _______   ____     /     \ _____    ____ _____     ____   ___________ 
 \_____  \ /  _ \   __\\   __\ \/ \/ /\__  \\_  __ \_/ __ \   /  \ /  \\__  \  /    \\__  \   / ___\_/ __ \_  __ \
 /        (  <_> )  |   |  |  \     /  / __ \|  | \/\  ___/  /    Y    \/ __ \|   |  \/ __ \_/ /_/  >  ___/|  | \/
/_______  /\____/|__|   |__|   \/\_/  (____  /__|    \___  > \____|__  (____  /___|  (____  /\___  / \___  >__|   
        \/                                 \/            \/          \/     \/     \/     \//_____/      \/       
        "#.yellow();
    println!("{ascii}");

    let server_url = "https://api.michijackson.xyz/search?q=".to_owned();
    let mut input = String::new();

    eprint!("{}", "Search: ".bright_yellow());
    io::stdin()
        .read_line(&mut input)
        .expect("failed to read input");

    let res = reqwest::get(server_url + &input).await?;
    println!("Status: {}", res.status());
    let text = res.text().await?;

    let v: Value = serde_json::from_str(&text)?;
    let data = &v["data"];

    let items: Vec<Software> =
        serde_json::from_value(data.clone()).expect("Failed to parse JSON into Software");

    let titles: Vec<&str> = items.iter().map(|s| s.title.as_str()).collect();
    let author: Vec<&str> = items.iter().map(|s| s.author.as_str()).collect();

    let items2: Vec<String> = titles
        .iter()
        .zip(author.iter())
        .map(|(t, a)| format!("{:<80} by {}", t, a))
        .collect();

    let selection = FuzzySelect::new()
        .with_prompt(format!("{}", "Pick your software".bright_yellow()))
        .items(&items2)
        .interact()
        .unwrap();

    let html_res = reqwest::get(&items[selection].url).await?;
    while !html_res.status().is_success() {
        println!("Failed to fetch the page, retrying...");
        thread::sleep(std::time::Duration::from_millis(500));
        let html_res = reqwest::get(&items[selection].url).await?;
        if html_res.status().is_success() {
            break;
        }
    }
    let html_text = html_res.text().await?;
    let doc = Document::from(html_text.as_str());
    let links = doc
        .find(Name("a"))
        .filter_map(|n| n.attr("href"))
        .collect::<Vec<_>>();

    let mut magnet: Option<&str> = None;
    for link in links {
        if link.starts_with("magnet:") {
            println!("magnet link: {link:?}");
            magnet = Some(link);
        }
    }

    let command = Command::new("/usr/bin/aria2c")
        .arg("--enable-rpc")
        .arg("--disable-ipv6")
        .arg("--rpc-listen-all")
        .arg("--rpc-listen-port=6800")
        //.arg("--rpc-secret=0")
        .spawn();

    thread::sleep(std::time::Duration::from_millis(500));

    if let Some(magnet) = magnet {
        aria2_ws(magnet, args).await;
    } else {
        println!("Failed to fetch magnet link");
        command
            .unwrap()
            .kill()
            .expect("Failed to kill aria2c process");
    }

    Ok(())
}

async fn aria2_ws(items: &str, args: Args) {
    eprint!("{}K", args.speed_limit);
    let client = Client::connect("ws://127.0.0.1:6800/jsonrpc", None)
        .await
        .unwrap();
    let options = TaskOptions {
        split: Some(2),
        extra_options: json!({"max-download-limit": format!("{}K", args.speed_limit)})
            .as_object()
            .unwrap()
            .clone(),
        ..Default::default()
    };

    let semaphore = Arc::new(Semaphore::new(0));
    client
        .add_uri(
            vec![items.to_string()],
            Some(options.clone()),
            None,
            Some(Callbacks {
                on_download_complete: Some({
                    let s = semaphore.clone();
                    async move {
                        s.add_permits(1);
                        println!("Task 1 completed!");
                    }
                    .boxed()
                }),
                on_error: Some({
                    let s = semaphore.clone();
                    async move {
                        s.add_permits(1);
                        println!("Task 1 error!");
                    }
                    .boxed()
                }),
            }),
        )
        .await
        .unwrap();

    let mut not = client.subscribe_notifications();

    spawn(async move {
        while let Ok(msg) = not.recv().await {
            println!("Received notification {:?}", &msg);
        }
    });

    let _ = semaphore.acquire_many(2).await.unwrap();

    client.shutdown().await.unwrap();
}
