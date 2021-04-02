use anyhow::Result;
use human_format::Formatter;
use log::{error, info};

use serde::Deserialize;
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
            let first_msg = msg
                .channel_id
                .say(&ctx.http, "Loading vaccination stats...")
                .await;
            let vacced_uk = tokio::spawn(async { get_vacced_count(Country::UK).await.unwrap() });
            let vacced_can = tokio::spawn(async { get_vacced_count(Country::CAN).await.unwrap() });

            let vacced_uk = vacced_uk.await.unwrap();
            let vacced_can = vacced_can.await.unwrap();
            let message = format!(
                "🇬🇧 {} have had a first dose which is {:.2}% of population, {} are completely vaccinated. ({})\n🇨🇦 {} have had a first dose which is {:.2}% of population, {} are completely vaccinated. ({})",
                Formatter::new().format(vacced_uk.first_count as f64),
                vacced_uk.prcnt,  Formatter::new().format(vacced_uk.fully_count as f64), vacced_uk.date,
                Formatter::new().format(vacced_can.first_count as f64),
                vacced_can.prcnt,  Formatter::new().format(vacced_can.fully_count as f64), vacced_can.date
            );
            if let Ok(mut msg) = first_msg {
                msg.edit(&ctx.http, |m| m.content(message)).await.unwrap();
            }
        }
        if msg.content.to_lowercase().starts_with("!version") {
            if let Err(why) = msg
                .channel_id
                .say(&ctx.http, format!("Vaxbot {}", env!("CARGO_PKG_VERSION")))
                .await
            {
                println!("Error sending message: {:?}", why);
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let mut client = Client::builder(token)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
    Ok(())
}

#[derive(Deserialize, Debug)]
struct VaccCount {
    first_count: u32,
    fully_count: u32,
    date: String,
    prcnt: f64,
}

async fn get_vacced_count(country: Country) -> Result<VaccCount> {
    let first_count: u32;
    let fully_count: u32;
    let date: String;
    let prcnt;
    match country {
        Country::UK => {
            let url = "https://api.coronavirus.data.gov.uk/v2/data";
            let params = vec![
                ("areaType", "overview"),
                ("metric", "cumPeopleVaccinatedFirstDoseByPublishDate"),
                ("metric", "cumPeopleVaccinatedCompleteByPublishDate"),
            ];
            let retrieved = get_api_content(url, params).await?;
            first_count = gjson::get(
                &retrieved,
                "body.0.cumPeopleVaccinatedFirstDoseByPublishDate",
            )
            .u32();
            fully_count = gjson::get(
                &retrieved,
                "body.0.cumPeopleVaccinatedCompleteByPublishDate",
            )
            .u32();
            date = gjson::get(&retrieved, "body.0.date").to_string();
            prcnt = first_count as f64 * (100 as f64 / 66800000 as f64);
        }
        Country::CAN => {
            let url = "https://api.covid19tracker.ca/summary";
            let retrieved = get_api_content(url, vec![]).await?;
            fully_count = gjson::get(&retrieved, "data.0.total_vaccinated").u32();
            first_count = gjson::get(&retrieved, "data.0.total_vaccinations").u32()
                - fully_count;
            date = gjson::get(&retrieved, "data.0.latest_date").to_string();
            prcnt = first_count as f64 * (100 as f64 / 37590000 as f64);
        }
    }
    //info!("Json: {:#?}", retrieved);

    Ok(VaccCount { first_count, fully_count, date, prcnt })
}

async fn get_api_content(url: &str, params: Vec<(&str, &str)>) -> Result<String> {
    let client = reqwest::Client::new();
    let request = client.get(url).query(&params).send().await?;
    let content = request.text().await;
    Ok(content?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn check_uk() -> Result<()> {
        let vacced_uk = get_vacced_count(Country::UK).await?;
        assert!(vacced_uk.first_count > 30000000);
        Ok(())
    }

    #[tokio::test]
    async fn check_can() -> Result<()> {
        let vacced_can = get_vacced_count(Country::CAN).await?;
        assert!(vacced_can.first_count > 4000000);
        Ok(())
    }
}
