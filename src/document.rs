use std::{fs, io, path::Path};

use crate::tokenizer::tokenize;

#[derive(Debug, PartialEq, Eq)]
pub struct Document {
    pub id: usize,
    pub name: String,
    pub title: String,
    pub url: Option<String>,
    pub text: String,
    pub tokens: Vec<String>,
}

impl Document {
    pub fn from_file(id: usize, path: &Path) -> io::Result<Self> {
        let text = fs::read_to_string(path)?;
        let name = path
            .file_name()
            .expect("a corpus path should have a file name")
            .to_string_lossy()
            .into_owned();

        Ok(Self {
            id,
            name,
            title: path
                .file_name()
                .expect("a corpus path should have a file name")
                .to_string_lossy()
                .into_owned(),
            url: None,
            text: text.clone(),
            tokens: tokenize(&text),
        })
    }
}
