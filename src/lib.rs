mod artifact;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[derive(PartialEq, Debug)]
pub enum Error {
    ParseArtifactError(String),
    UrlError(url::ParseError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
