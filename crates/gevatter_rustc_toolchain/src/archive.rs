use liblzma::read::XzDecoder;
use std::fs;
use std::io;
use std::path::Path;
use tar::Archive;

pub type EntryItem<'a> = tar::Entry<'a, XzDecoder<ureq::BodyReader<'static>>>;

pub fn download_read(
    url: &str,
    mut f: impl FnMut(EntryItem<'_>) -> io::Result<()>,
) -> io::Result<()> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| io::Error::other(format!("failed to download {url}: {e}")))?;
    let mut archive = Archive::new(XzDecoder::new(response.into_body().into_reader()));
    archive.entries()?.try_for_each(|entry| {
        let entry = entry?;
        f(entry)
    })
}

pub fn unpack(dest: &Path) -> impl Fn(EntryItem<'_>) -> io::Result<()> {
    |mut entry| {
        let path = entry.path()?.into_owned();
        let dest = dest.join(&path);
        fs::create_dir_all(dest.parent().unwrap())?;
        entry.unpack(&dest).map(|_| ())
    }
}
