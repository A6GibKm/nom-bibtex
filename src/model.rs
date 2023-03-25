use crate::error::BibtexError;
use crate::parser;
use crate::parser::{mkspan, Entry, Span};
use nom::error::VerboseError;
use std::collections::HashMap;
use std::result;
use std::str;

type Result<T> = result::Result<T, BibtexError>;

const TABLE_MONTHS: [(&'static str, &'static str); 12] = [
    ("jan", "January"),
    ("feb", "February"),
    ("mar", "March"),
    ("apr", "April"),
    ("may", "May"),
    ("jun", "June"),
    ("jul", "July"),
    ("aug", "August"),
    ("sep", "September"),
    ("oct", "October"),
    ("nov", "November"),
    ("dec", "December"),
];

/// A high-level definition of a bibtex file.
#[derive(Debug, PartialEq, Eq, Default)]
pub struct Bibtex {
    comments: Vec<String>,
    preambles: Vec<String>,
    const_map: HashMap<&'static str, &'static str>,
    variables: HashMap<String, String>,
    bibliographies: Vec<Bibliography>,
}

impl Bibtex {
    /// Create a new Bibtex instance from a *BibTeX* file content.
    pub fn parse(bibtex: &str) -> Result<Self> {
        let entries = Self::raw_parse(bibtex)?;

        let mut bibtex = Bibtex::default();

        Self::fill_constants(&mut bibtex)?;
        Self::fill_variables(&mut bibtex, &entries)?;

        for entry in entries {
            match entry {
                Entry::Variable(_) => continue, // Already handled.
                Entry::Comment(v) => bibtex.comments.push(v),
                Entry::Preamble(v) => {
                    let new_val = Self::expand_str_abbreviations(v, &bibtex)?;
                    bibtex.preambles.push(new_val);
                }
                Entry::Bibliography(entry_t, citation_key, tags) => {
                    let mut new_tags = HashMap::new();
                    for tag in tags {
                        let key = tag.key.to_lowercase();
                        new_tags.insert(
                            key,
                            Self::expand_str_abbreviations(tag.value, &bibtex)?,
                        );
                    }
                    bibtex
                        .bibliographies
                        .push(Bibliography::new(entry_t, citation_key, new_tags));
                }
            }
        }
        Ok(bibtex)
    }

    /// Get a raw vector of entries in order from the files.
    pub fn raw_parse(bibtex: &str) -> Result<Vec<Entry>> {
        let span = mkspan(bibtex);
        match parser::entries::<VerboseError<Span>>(span) {
            Ok((_, v)) => Ok(v),
            Err(e) => Err(BibtexError::with_context(bibtex, e)),
        }
    }

    /// Get preambles with expanded variables.
    pub fn preambles(&self) -> &[String] {
        &self.preambles
    }

    /// Get comments.
    pub fn comments(&self) -> &[String] {
        &self.comments
    }

    /// Get string variables with a tuple of key and expanded value.
    pub fn variables(&self) -> HashMap<String, String> {
        self.variables.clone()
    }

    /// Get bibliographies entry with variables expanded.
    pub fn bibliographies(&self) -> &Vec<Bibliography> {
        &self.bibliographies
    }

    fn fill_constants(bibtex: &mut Bibtex) -> Result<()> {
        for m in &TABLE_MONTHS {
            bibtex.const_map.insert(m.0, m.1);
        }
        Ok(())
    }

    fn fill_variables(bibtex: &mut Bibtex, entries: &[Entry]) -> Result<()> {
        let variables = entries
            .iter()
            .filter_map(|v| match v {
                &Entry::Variable(ref v) => Some(v),
                _ => None,
            })
            .collect::<Vec<_>>();

        for var in &variables {
            bibtex.variables.insert(
                var.key.clone(),
                Self::expand_variables_value(&var.value, &variables)?,
            );
        }

        Ok(())
    }

    fn expand_variables_value(
        var_values: &Vec<StringValueType>,
        variables: &Vec<&KeyValue>,
    ) -> Result<String> {
        let mut result_value = String::new();

        for chunck in var_values {
            match chunck.clone() {
                StringValueType::Str(v) => result_value.push_str(&v),
                StringValueType::Abbreviation(v) => {
                    let var = variables
                        .iter()
                        .find(|&x| *v == x.key)
                        .ok_or_else(|| BibtexError::StringVariableNotFound(v.into()))?;
                    result_value.push_str(&Self::expand_variables_value(&var.value, &variables)?);
                }
            }
        }
        Ok(result_value)
    }

    fn expand_str_abbreviations(value: Vec<StringValueType>, bibtex: &Bibtex) -> Result<String> {
        let mut result = String::new();

        for chunck in value {
            match chunck {
                StringValueType::Str(v) => result.push_str(&v),
                StringValueType::Abbreviation(v) => {
                    let var = bibtex.variables.iter().find(|&x| &v == x.0);
                    if let Some(res) = var {
                        result.push_str(res.1)
                    } else {
                        match bibtex.const_map.get(v.as_str()) {
                            Some(res) => result.push_str(res),
                            None => return Err(BibtexError::StringVariableNotFound(v.into())),
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}

/// This is the main representation of a bibliography.
#[derive(Debug, PartialEq, Eq)]
pub struct Bibliography {
    entry_type: String,
    citation_key: String,
    tags: HashMap<String, String>,
}

impl Bibliography {
    /// Create a new bibliography.
    pub fn new(
        entry_type: String,
        citation_key: String,
        tags: HashMap<String, String>,
    ) -> Bibliography {
        Bibliography {
            entry_type,
            citation_key,
            tags,
        }
    }

    /// Get the entry type.
    ///
    /// It represents the type of the publications such as article, book, ...
    pub fn entry_type(&self) -> &str {
        &self.entry_type
    }

    /// Get the citation key.
    ///
    /// The citation key is the the keyword used to reference the bibliography
    /// in a LaTeX file for example.
    pub fn citation_key(&self) -> &str {
        &self.citation_key
    }

    /// Get the tags.
    ///
    /// Tags are the specifics information about a bibliography
    /// such as author, date, title, ...
    pub fn tags(&self) -> HashMap<String, String> {
        self.tags.clone()
    }
}

/// Represent a Bibtex value which is composed of
///
/// - strings value
/// - string variable/abbreviation which will be expanded after parsing.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum StringValueType {
    /// Just a basic string.
    Str(String),
    /// An abbreviation that should match some string variable.
    Abbreviation(String),
}

/// Representation of a key-value.
///
/// Only used by parsing.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct KeyValue {
    pub key: String,
    pub value: Vec<StringValueType>,
}

impl KeyValue {
    pub fn new(key: String, value: Vec<StringValueType>) -> KeyValue {
        Self { key, value }
    }
}
