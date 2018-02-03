use std::env;
use std::collections::HashMap;
use reqwest;
use std::result;
use std::error::Error;
use serde_json::Value;

pub struct Api {
    key: String,
}

#[derive(Debug)]
pub struct ApiError {
    reason: String,
}

impl ApiError {
    fn new<S: Into<String>>(reason: S) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

pub type Result<T> = result::Result<T, ApiError>;

impl From<reqwest::Error> for ApiError {
    fn from(error: reqwest::Error) -> Self {
        println!("{:?}", error);
        ApiError::new(error.description())
    }
}

impl Api {
    pub fn new<S: Into<String>>(key: S) -> Api {
        Api { key: key.into() }
    }

    pub fn from_env() -> result::Result<Api, env::VarError> {
        Ok(Api::new(env::var("STEAM_API_KEY")?))
    }

    fn call<S: AsRef<str>>(&self, path: S, options: &mut HashMap<String, String>) -> Result<Value> {
        let url = format!("https://api.steampowered.com/{}", path.as_ref());
        options.insert(String::from("key"), self.key.to_owned());

        let http = reqwest::Client::new();
        let mut resp = http.get(url.as_str()).query(options).send()?;
        if resp.status().is_success() {
            Ok(resp.json()?)
        } else {
            Err(ApiError::new("Steam API returned an invalid response code"))
        }
    }

    pub fn resolve_vanity_url<S: Into<String>>(&self, username: S) -> Result<String> {
        let mut options = HashMap::new();
        options.insert(String::from("vanityurl"), username.into());
        let response = self.call("ISteamUser/ResolveVanityURL/v0001/", &mut options)?;
        let response = response["response"]["steamid"].clone();
        match response {
            Value::String(steamid) => Ok(steamid),
            _ => Err(ApiError::new("Steam API returned an invalid response")),
        }
    }

    pub fn get_owned_games<S: Into<String>>(&self, steamid: S) -> Result<Vec<u64>> {
        let mut options = HashMap::new();
        options.insert(String::from("steamid"), steamid.into());
        options.insert(String::from("format"), String::from("json"));
        let response = self.call("IPlayerService/GetOwnedGames/v0001/", &mut options)?;
        let response = response["response"]["games"].clone();
        match response {
            Value::Array(games) => {
                let games = games
                    .iter()
                    .filter_map(|game| match game["appid"] {
                        Value::Number(ref num) => num.as_u64(),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                Ok(games)
            }
            _ => Err(ApiError::new("Steam API returned an invalid response")),
        }
    }
}
