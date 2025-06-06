use crate::resolver::*;
use bytes::Bytes;
use reqwest::Client;
use std::io::Cursor;
use url::Url;

impl AsyncHttp for Client {
    async fn get(&self, url: Url) -> Result<Cursor<Bytes>, ResolveError> {
        let builder = self.get(url.clone());
        let response = builder.send().await?;
        if response.status().is_success() {
            let bytes = response.bytes().await?;
            let cursor = Cursor::new(bytes);
            Ok(cursor)
        } else {
            Err(ResolveError::GenericHttpError {
                url: url.clone(),
                status: response.status().as_u16(),
            })
        }
    }
}
