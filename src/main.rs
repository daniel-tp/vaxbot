use anyhow::Result;
use human_format::Formatter;
use log::{error, info};

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
            let message = format!("💉💉💉\n\
                🇬🇧 {} {:.2}% (+{}, {:.2}%) have had a first dose, \n{} {:.2}% (+{} 📈, {:.2}%) are completely vaccinated. ({})\n🇨🇦 {} {:.2}% (+{}, {:.2}%) have had a first dose, \n{} {:.2}% (+{} 📈, {:.2}%) are completely vaccinated. ({})",
                Formatter::new().format(vacced_uk.first.count as f64), vacced_uk.first.count_prcnt, vacced_uk.first.diff, vacced_uk.first.diff_prcnt,
                Formatter::new().format(vacced_uk.full.count as f64), vacced_uk.full.count_prcnt, vacced_uk.full.diff, vacced_uk.full.diff_prcnt, vacced_uk.date,
                Formatter::new().format(vacced_can.first.count as f64), vacced_can.first.count_prcnt, vacced_can.first.diff, vacced_can.first.diff_prcnt,
                Formatter::new().format(vacced_can.full.count as f64), vacced_can.full.count_prcnt, vacced_can.full.diff, vacced_can.full.diff_prcnt, vacced_can.date,
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

struct VaccDayData {
    first: VaccedCount,
    full: VaccedCount,
    date: String,
}

struct VaccedCount {
    count: u32,
    count_prcnt: f64,
    diff: u32,
    diff_prcnt: f64,
}

impl VaccedCount {
    fn new(count: u32, diff: u32, population: u32) -> VaccedCount {
        VaccedCount {
            count,
            count_prcnt: count as f64 * (100 as f64 / population as f64),
            diff,
            diff_prcnt: diff as f64 * (100 as f64 / population as f64),
        }
    }
}

async fn get_vacced_count(country: Country) -> Result<VaccDayData> {
    let first: VaccedCount;
    let full: VaccedCount;
    let date: String;
    match country {
        Country::UK => {
            let url = "https://api.coronavirus.data.gov.uk/v2/data";
            let params = vec![
                ("areaType", "overview"),
                ("metric", "cumPeopleVaccinatedFirstDoseByPublishDate"),
                ("metric", "cumPeopleVaccinatedCompleteByPublishDate"),
                ("metric", "newPeopleVaccinatedFirstDoseByPublishDate"),
                ("metric", "newPeopleVaccinatedCompleteByPublishDate"),
            ];
            let retrieved = get_api_content(url, params).await?;
            let first_count = gjson::get(
                &retrieved,
                "body.0.cumPeopleVaccinatedFirstDoseByPublishDate",
            )
            .u32();
            let first_diff = gjson::get(
                &retrieved,
                "body.0.newPeopleVaccinatedFirstDoseByPublishDate",
            )
            .u32();
            first = VaccedCount::new(first_count, first_diff, 66800000);
            let full_count = gjson::get(
                &retrieved,
                "body.0.cumPeopleVaccinatedCompleteByPublishDate",
            )
            .u32();
            let full_diff = gjson::get(
                &retrieved,
                "body.0.newPeopleVaccinatedCompleteByPublishDate",
            )
            .u32();
            full = VaccedCount::new(full_count, full_diff, 66800000);
            date = gjson::get(&retrieved, "body.0.date").to_string();
        }
        Country::CAN => {
            let url = "https://api.covid19tracker.ca/summary";
            let retrieved = get_api_content(url, vec![]).await?;
            let full_count = gjson::get(&retrieved, "data.0.total_vaccinated").u32();
            let full_diff = gjson::get(&retrieved, "data.0.change_vaccinated").u32();
            full = VaccedCount::new(full_count, full_diff, 37590000);
            let first_count =
                gjson::get(&retrieved, "data.0.total_vaccinations").u32() - full_count;
            let first_diff = gjson::get(&retrieved, "data.0.change_vaccinations").u32() - full_diff;
            first = VaccedCount::new(first_count, first_diff, 37590000);
            date = gjson::get(&retrieved, "data.0.latest_date").to_string();
        }
    }

    Ok(VaccDayData { first, full, date })
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
        assert!(vacced_uk.first.count > 30000000);
        Ok(())
    }

    #[tokio::test]
    async fn check_can() -> Result<()> {
        let vacced_can = get_vacced_count(Country::CAN).await?;
        assert!(vacced_can.first.count > 4000000);
        Ok(())
    }
}
