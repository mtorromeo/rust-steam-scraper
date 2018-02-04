use std::env;
use std::collections::HashMap;
use reqwest;
use reqwest::header::{Cookie, Headers};
use std::str::FromStr;
use serde_json::Value;
use url::Url;
use scraper::{Html, Selector};
use std::path::{Path, PathBuf};
use failure::Error;

pub struct Api {
    key: String,
}

impl Api {
    pub fn new<S: Into<String>>(key: S) -> Api {
        Api { key: key.into() }
    }

    pub fn from_env() -> Result<Api, Error> {
        Ok(Api::new(env::var("STEAM_API_KEY")?))
    }

    fn call<S: AsRef<str>>(
        &self,
        path: S,
        options: &mut HashMap<String, String>,
    ) -> Result<Value, Error> {
        let url = format!("https://api.steampowered.com/{}", path.as_ref());
        options.insert(String::from("key"), self.key.to_owned());

        let http = reqwest::Client::new();
        let mut resp = http.get(url.as_str()).query(options).send()?;
        if resp.status().is_success() {
            Ok(resp.json()?)
        } else {
            format_err!("Invalid response returned by the Steam API")
        }
    }

    pub fn resolve_vanity_url<S: Into<String>>(&self, username: S) -> Result<String, Error> {
        let mut options = HashMap::new();
        options.insert(String::from("vanityurl"), username.into());
        let response = self.call("ISteamUser/ResolveVanityURL/v0001/", &mut options)?;
        let response = response["response"]["steamid"].clone();
        match response {
            Value::String(steamid) => Ok(steamid),
            _ => bail!("Invalid response returned by the Steam API"),
        }
    }

    pub fn get_owned_games<S: Into<String>>(&self, steamid: S) -> Result<Vec<u64>, Error> {
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
            _ => bail!("Invalid response returned by the Steam API"),
        }
    }
}

pub fn appid_from_url(url: &Url) -> Result<u64, Error> {
    match url.path_segments() {
        Some(mut parts) => {
            match parts.next() {
                Some("app") => (),
                Some(_) | None => bail!("Invalid Steam game URL"),
            }
            match parts.next() {
                Some(id) => Ok(u64::from_str(id)?),
                None => bail!("Invalid Steam game URL"),
            }
        }
        None => bail!("Invalid Steam game URL"),
    }
}

#[derive(Debug)]
pub struct Page {
    appid: u64,
    cache_path: PathBuf,
    props: HashMap<String, String>,
    screenshots: Vec<String>,
}

impl Page {
    pub fn scrape(appid: u64) -> Result<Self, Error> {
        let url = Url::parse(&format!("http://store.steampowered.com/app/{}/", appid))?;
        Self::scrape_url(&url)
    }

    pub fn scrape_url(url: &Url) -> Result<Self, Error> {
        let appid = appid_from_url(url)?;
        let cache_path = Path::new("cache").join(&format!("{}", appid));
        let body = Self::fetch(&url, &cache_path.join("index.html"))?;

        let doc = Html::parse_document(body.as_str());
        let props = doc.props();
        let screenshots = doc.screenshots();

        Ok(Page {
            appid: appid,
            cache_path: cache_path,
            props: props,
            screenshots: screenshots,
        })
    }

    fn fetch<S: AsRef<str>>(url: S, cache_path: &Path) -> Result<String, Error> {
        if cache_path.to_str().unwrap_or("") != "" {
            if let Ok(body) = super::utils::file_get_string_contents(cache_path) {
                debug!("Found page in cache");
                return Ok(body);
            }
        }

        let url = url.as_ref();
        info!("Fetching url {}", url);

        let mut headers = Headers::new();
        let mut cookie = Cookie::new();
        cookie.append("birthtime", "400000000");
        cookie.append("mature_content", "1");
        headers.set(cookie);

        let http = reqwest::Client::new();

        let mut resp = http.get(url).headers(headers).send()?;
        ensure!(
            resp.status().is_success(),
            "Failed to retrieve steam store page"
        );
        let body = resp.text()?;

        if cache_path.to_str().unwrap_or("") != "" {
            if let Err(why) = super::utils::file_put_contents(cache_path, body.as_bytes()) {
                error!("Couldn't save page body to offline cache: {}", why);
            }
        }

        Ok(body)
    }

    pub fn fetch_images(&self) {
        if let Some(imageurl) = self.props.get("image") {
            if let Err(error) = super::utils::wget_to_dir(imageurl, &self.cache_path) {
                warn!("{:?}", error);
            }
        }

        for imageurl in &self.screenshots {
            let imageurl = imageurl.replace(".116x65.jpg", ".jpg");
            if let Err(error) = super::utils::wget_to_dir(imageurl, &self.cache_path) {
                warn!("{:?}", error);
            }
        }
    }
}

trait SteamScraper {
    fn props(&self) -> HashMap<String, String>;
    fn screenshots(&self) -> Vec<String>;
}

impl SteamScraper for Html {
    fn props(&self) -> HashMap<String, String> {
        let mut props = HashMap::new();

        let itemprops = Selector::parse("[itemprop]").unwrap();
        for item in self.select(&itemprops) {
            let prop = item.value().attr("itemprop").unwrap();
            if let Some(content) = {
                if let Some(content) = item.value().attr("content") {
                    Some(content.to_string())
                } else if item.children().find(|child| child.value().is_element()) == None {
                    Some(item.inner_html())
                } else {
                    None
                }
            } {
                props.insert(prop.to_string(), content);
            }
        }

        props
    }

    fn screenshots(&self) -> Vec<String> {
        let mut images = vec![];

        let imgs = Selector::parse("div.highlight_strip_screenshot > img[src]").unwrap();
        for img in self.select(&imgs) {
            let src = img.value().attr("src").unwrap();
            images.push(src.to_owned());
        }
        images
    }
}
