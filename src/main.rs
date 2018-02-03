#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate reqwest;
extern crate scraper;
extern crate select;
extern crate url;
extern crate dotenv;
extern crate serde_json;

use std::string::String;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::fs;
use std::fs::File;
use std::path::Path;
use scraper::{Html, Selector};
use url::Url;

mod steamapi;

trait SteamScraper {
    fn props(&self) -> HashMap<String, String>;
    fn screenshots(&self) -> Vec<String>;
}

impl SteamScraper for scraper::Html {
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

fn main() {
    dotenv::dotenv().ok();

    let url = Url::parse("http://store.steampowered.com/app/678950/DRAGON_BALL_FighterZ/")
        .expect("Invalid url");

    let cache_id = steamurl_appid(&url).expect("Invalid steam app url");
    let cache_path = Path::new("cache").join(cache_id);

    let body = match url_fetch_body(&url, &cache_path.join("index.html")) {
        Ok(body) => body,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let doc = Html::parse_document(body.as_str());
    let props = doc.props();
    println!("{:?}", props);

    if let Some(imageurl) = props.get("image") {
        wget_to_dir(imageurl, &cache_path).unwrap();
    }

    for imageurl in doc.screenshots() {
        let imageurl = imageurl.replace(".116x65.jpg", ".jpg");
        wget_to_dir(imageurl, &cache_path).unwrap();
    }
}

fn steamurl_appid(url: &Url) -> Option<&str> {
    match url.path_segments() {
        Some(mut parts) => {
            match parts.next() {
                Some("app") => (),
                Some(_) | None => return None,
            }
            parts.next()
        }
        None => None,
    }
}

fn url_fetch_body<S: AsRef<str>>(url: S, cache_path: &Path) -> Result<String, String> {
    if cache_path.to_str().unwrap_or("") != "" {
        if let Ok(body) = file_get_string_contents(cache_path) {
            println!("Found page in cache");
            return Ok(body);
        }
    }

    let url = url.as_ref();
    println!("Fetching url {}", url);

    let mut resp = match reqwest::get(url) {
        Ok(resp) => resp,
        Err(e) => return Err(e.to_string()),
    };
    if !resp.status().is_success() {
        return Err(String::from("Failed to retrieve steam store page"));
    }
    let body = match resp.text() {
        Ok(body) => body,
        Err(e) => return Err(e.to_string()),
    };

    if cache_path.to_str().unwrap_or("") != "" {
        if let Err(why) = file_put_contents(cache_path, body.as_bytes()) {
            println!("Couldn't save page body to offline cache: {}", why);
        }
    }

    Ok(body)
}

fn wget<S: AsRef<str>>(url: S, filename: &Path, force: bool) -> Result<(), String> {
    if !force && filename.exists() {
        return Ok(());
    }

    let url = url.as_ref();
    println!("Fetching URL {}", url);

    let mut resp = match reqwest::get(url) {
        Ok(resp) => resp,
        Err(e) => return Err(e.to_string()),
    };
    if !resp.status().is_success() {
        return Err(String::from("Failed to retrieve URL"));
    }
    if let Err(why) = file_put_bytes(filename, &mut resp) {
        println!("Couldn't write file to disk: {}", why);
    }

    Ok(())
}

fn wget_to_dir<S: AsRef<str>>(url: S, dir: &Path) -> Result<(), String> {
    let url_s = url.as_ref();
    let url = match Url::parse(url_s) {
        Ok(url) => url,
        Err(_) => return Err(String::from("Invalid URL")),
    };

    if let Some(segments) = url.path_segments() {
        if let Some(name) = segments.last() {
            let name = dir.join(name);
            return wget(url_s, &name, false);
        }
    }

    Err(String::from("Invalid URL"))
}

fn file_get_string_contents(filename: &Path) -> io::Result<String> {
    let mut file = File::open(filename)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn file_put_contents(filename: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = filename.parent() {
        fs::create_dir_all(parent)?;
    }
    File::create(filename)?.write_all(contents)
}

fn file_put_bytes<R: ?Sized>(filename: &Path, bytes: &mut R) -> io::Result<()>
where
    R: io::Read,
{
    if let Some(parent) = filename.parent() {
        fs::create_dir_all(parent)?;
    }
    match io::copy(bytes, &mut File::create(filename)?) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
