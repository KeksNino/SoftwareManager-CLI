use aria2_ws::{Callbacks, Client, TaskOptions};
use dialoguer::FuzzySelect;
use futures::FutureExt;
use select::document::Document;
use select::predicate::Name;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use tokio::{spawn, sync::Semaphore};

#[derive(Debug, Deserialize)]
struct Software {
    //author: String,
    title: String,
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ascii = r#"
  _________       _____  __                                     _____                                             
 /   _____/ _____/ ____\/  |___  _  _______ _______   ____     /     \ _____    ____ _____     ____   ___________ 
 \_____  \ /  _ \   __\\   __\ \/ \/ /\__  \\_  __ \_/ __ \   /  \ /  \\__  \  /    \\__  \   / ___\_/ __ \_  __ \
 /        (  <_> )  |   |  |  \     /  / __ \|  | \/\  ___/  /    Y    \/ __ \|   |  \/ __ \_/ /_/  >  ___/|  | \/
/_______  /\____/|__|   |__|   \/\_/  (____  /__|    \___  > \____|__  (____  /___|  (____  /\___  / \___  >__|   
        \/                                 \/            \/          \/     \/     \/     \//_____/      \/       
        "#;
    println!("{ascii}");

    let server_url = "https://api.michijackson.xyz/search/".to_owned();
    let mut input = String::new();

    eprint!("Search: ");
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

    let selection = FuzzySelect::new()
        .with_prompt("Pick your software")
        .items(&titles)
        .interact()
        .unwrap();

    let html_res = reqwest::get(&items[selection].url).await?;
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

    let _command = Command::new("/usr/bin/aria2c")
        .arg("--enable-rpc")
        .arg("--disable-ipv6")
        .arg("--rpc-listen-all")
        .arg("--rpc-listen-port=6800")
        //.arg("--rpc-secret=0")
        .spawn();

    thread::sleep(std::time::Duration::from_millis(500));

    aria2_ws(magnet.unwrap()).await;

    Ok(())
}

async fn aria2_ws(items: &str) {
    let client = Client::connect("ws://127.0.0.1:6800/jsonrpc", None)
        .await
        .unwrap();
    let options = TaskOptions {
        //split: Some(2),
        //extra_options: json!({"max-download-limit": "200K"})
        //    .as_object()
        //    .unwrap()
        //    .clone(),
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
