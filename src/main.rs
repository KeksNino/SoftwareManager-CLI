use dialoguer::FuzzySelect;
use serde::Deserialize;
use serde_json::Value;
use std::io;

#[derive(Debug, Deserialize)]
struct Software {
    author: String,
    id: i64,
    title: String,
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_url = "https://api.michijackson.xyz/search/".to_owned();
    let mut input = String::new();

    eprint!("Search: ");
    io::stdin()
        .read_line(&mut input)
        .expect("failed to read input");

    let res = reqwest::get(server_url + &input).await?;
    println!("Status: {}", res.status());

    let body = res.text().await?;
    let v: Value = serde_json::from_str(&body)?;
    let data = &v["data"];

    let items: Vec<Software> =
        serde_json::from_value(data.clone()).expect("Failed to parse JSON into Software");

    let titles: Vec<&str> = items.iter().map(|s| s.title.as_str()).collect();

    let selection = FuzzySelect::new()
        .with_prompt("Pick your software")
        .items(&titles)
        .interact()
        .unwrap();

    println!("You selected: {}", items[selection].title);
    println!("Author: {}", items[selection].author);
    println!("ID: {}", items[selection].id);
    println!("URL: {}", items[selection].url);

    Ok(())
}
