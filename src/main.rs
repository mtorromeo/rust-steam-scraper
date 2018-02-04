#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

#[macro_use]
extern crate failure;

#[macro_use]
extern crate log;
extern crate pretty_env_logger;

#[macro_use(values_t)]
extern crate clap;

extern crate dotenv;
extern crate reqwest;
extern crate scraper;
extern crate select;
extern crate serde_json;
extern crate url;

mod steam;
mod utils;

use clap::{App, Arg, ArgGroup};
use failure::{Error, ResultExt};

fn main() {
    if let Err(ref e) = run() {
        for cause in e.causes() {
            error!("{}", cause);
        }
        ::std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    dotenv::dotenv().ok();
    pretty_env_logger::init();

    let args = App::new("SteamScrape")
        .version("1.0")
        .author("Massimiliano Torromeo <massimiliano.torromeo@gmail.com>")
        .about("Steam store web scraper")
        .arg(
            Arg::with_name("user")
                .short("u")
                .long("user")
                .value_name("USER")
                .help("Scrape this user's whole library")
                .takes_value(true)
                .empty_values(false),
        )
        .arg(
            Arg::with_name("gameids")
                .short("g")
                .long("gameid")
                .value_name("ID")
                .help("Scrape the steam page for the game with this id")
                .multiple(true)
                .takes_value(true)
                .empty_values(false),
        )
        .group(
            ArgGroup::with_name("games")
                .args(&["user", "gameids"])
                .required(true),
        )
        .get_matches();

    if let Some(games) = {
        if let Some(user) = args.value_of("user") {
            let api = steam::Api::from_env().context(
                "No steam api key provided. Set one in the STEAM_API_KEY environment variable.",
            )?;
            let steamid = api.resolve_vanity_url(user)
                .context(format!("Couldn't find steamid for {}", user))?;
            info!("Resolved vanity name to: {}", steamid);
            match api.get_owned_games(steamid) {
                Ok(games) => Some(games),
                Err(_) => None,
            }
        } else if args.is_present("gameids") {
            Some(values_t!(args.values_of("gameids"), u64).unwrap_or_else(|e| e.exit()))
        } else {
            None
        }
    } {
        for game in games {
            if let Ok(page) = steam::Page::scrape(game) {
                debug!("{:?}", page);
                page.fetch_images();
            }
        }
    }

    Ok(())
}
