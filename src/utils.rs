use std::io;
use std::io::prelude::*;
use std::fs;
use std::fs::File;
use std::path::Path;
use url::Url;
use reqwest;
use failure::Error;

pub fn wget<S: AsRef<str>>(url: S, filename: &Path, force: bool) -> Result<(), Error> {
    if !force && filename.exists() {
        return Ok(());
    }

    let url = url.as_ref();
    info!("Fetching URL {}", url);

    let mut resp = reqwest::get(url)?;
    ensure!(resp.status().is_success(), "Failed to retrieve URL");

    if let Err(why) = file_put_bytes(filename, &mut resp) {
        error!("Couldn't write file to disk: {}", why);
    }

    Ok(())
}

pub fn wget_to_dir<S: AsRef<str>>(url: S, dir: &Path) -> Result<(), Error> {
    let url_s = url.as_ref();
    let url = Url::parse(url_s)?;

    if let Some(segments) = url.path_segments() {
        if let Some(name) = segments.last() {
            let name = dir.join(name);
            return wget(url_s, &name, false);
        }
    }

    format_err!("Invalid URL")
}

pub fn file_get_string_contents(filename: &Path) -> io::Result<String> {
    let mut file = File::open(filename)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn file_put_contents(filename: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = filename.parent() {
        fs::create_dir_all(parent)?;
    }
    File::create(filename)?.write_all(contents)
}

pub fn file_put_bytes<R: ?Sized>(filename: &Path, bytes: &mut R) -> io::Result<()>
where
    R: io::Read,
{
    if let Some(parent) = filename.parent() {
        fs::create_dir_all(parent)?;
    }
    io::copy(bytes, &mut File::create(filename)?)?;
    Ok(())
}
