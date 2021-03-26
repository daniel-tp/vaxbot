use anyhow::Result;
use human_format::Formatter;
use log::{error, info};

use serde::{de::value::StrDeserializer, Deserialize};
use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*,
};
use std::env;

enum Country {
    UK,
    CAN,
}
struct Handler;

#[async_trait]
impl EventHandler for Handler {

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content.to_lowercase().starts_with("!vacced") {

            let first_msg = msg.channel_id.say(&ctx.http, "Loading vaccination stats...").await;
            let vacced_uk = tokio::spawn(async {
                get_vacced_count(Country::UK).await.unwrap()
            });
            let vacced_can = tokio::spawn(async {
                get_vacced_count(Country::CAN).await.unwrap()
            });

            let vacced_uk = vacced_uk.await.unwrap();
            let vacced_can = vacced_can.await.unwrap();
            let message = format!(
                "🇬🇧 {} which is {:.2}% of population\n🇨🇦 {} which is {:.2}% of population",
                Formatter::new().format(vacced_uk.count as f64),
                vacced_uk.prcnt,
                Formatter::new().format(vacced_can.count as f64),
                vacced_can.prcnt
            );
            if let Ok(mut msg) = first_msg {
                msg.edit(&ctx.http, |m| m.content(message)).await.unwrap();
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let token = env::var("DISCORD_TOKEN")
    .expect("Expected a token in the environment");

    let mut client = Client::builder(token)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
    Ok(())
}
#[derive(Deserialize, Debug)]
struct VaccCount {
    count: u32,
    date: String,
    prcnt: f64,
}

async fn get_vacced_count(country: Country) -> Result<VaccCount> {
    let count;
    let date;
    let prcnt;
    match country {
        Country::UK => {
            let url = "https://api.coronavirus.data.gov.uk/v2/data";
            let params = vec![
                ("areaType", "overview"),
                ("metric", "cumPeopleVaccinatedFirstDoseByPublishDate"),
            ];
            let retrieved = get_api_content(url, params).await?;
            count = retrieved["body"][0]["cumPeopleVaccinatedFirstDoseByPublishDate"]
                .as_u64()
                .unwrap() as u32;
            date = retrieved["body"][0]["date"].as_str().unwrap().to_string();
            prcnt = count as f64 * (100 as f64 / 66800000 as f64);
        }
        Country::CAN => {
            let url = "https://api.covid19tracker.ca/summary";
            let retrieved = get_api_content(url, vec![]).await?;
            date = retrieved["data"][0]["latest_date"]
                .as_str()
                .unwrap()
                .to_string();
            count = retrieved["data"][0]["total_vaccinations"]
                .as_str()
                .unwrap()
                .parse::<u32>()
                .unwrap()
                - retrieved["data"][0]["total_vaccinated"]
                    .as_str()
                    .unwrap()
                    .parse::<u32>()
                    .unwrap();
            prcnt = count as f64 * (100 as f64 / 37590000 as f64);
        }
    }
    //info!("Json: {:#?}", retrieved);

    Ok(VaccCount { count, date, prcnt })
}

async fn get_api_content(url: &str, params: Vec<(&str, &str)>) -> Result<serde_json::Value> {
    let client = reqwest::Client::new();
    let request = client.get(url).query(&params).send().await?;
    Ok(request.json::<serde_json::Value>().await?)
}
