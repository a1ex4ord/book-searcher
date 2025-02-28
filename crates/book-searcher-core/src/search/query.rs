use crate::Searcher;
use serde::Deserialize;
use tantivy::{
    query::{BooleanQuery, BoostQuery, Occur, PhraseQuery, Query, QueryParserError, TermQuery},
    schema::{Field, IndexRecordOption},
    tokenizer::TextAnalyzer,
    Term,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Filter,
    Explore,
}

impl Default for SearchMode {
    fn default() -> Self {
        Self::Filter
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct SearchQuery {
    pub title: Option<String>,
    pub author: Option<String>,
    pub publisher: Option<String>,
    pub extension: Option<String>,
    pub language: Option<String>,
    pub isbn: Option<String>,
    pub id: Option<u64>,

    pub query: Option<String>,
    #[serde(default)]
    pub mode: SearchMode,
}

impl SearchQuery {
    pub fn new_query(query: String) -> Self {
        Self {
            query: Some(query),
            ..Default::default()
        }
    }

    pub fn parse(&self, searcher: &Searcher) -> Result<Box<dyn Query>, QueryParserError> {
        // If query is specified, use QueryParser to parse
        if let Some(ref raw_query) = self.query {
            return match self.mode {
                SearchMode::Filter => {
                    let mut query_parser = searcher.query_parser.clone();
                    query_parser.set_conjunction_by_default();
                    query_parser.parse_query(raw_query)
                }
                SearchMode::Explore => searcher.query_parser.parse_query(raw_query),
            };
        }

        // else construct Query
        let mut queries: Vec<Box<dyn Query>> = Vec::with_capacity(4);

        if let Some(ref title) = self.title {
            let terms = get_positions_and_terms(searcher.title, title, &searcher.tokenizer);
            if let Some(query) = phrase_or_term_query(terms) {
                let query = BoostQuery::new(Box::new(query), 3.0);
                queries.push(Box::new(query));
            }
        }

        if let Some(ref author) = self.author {
            let terms = get_positions_and_terms(searcher.author, author, &searcher.tokenizer);
            if let Some(query) = phrase_or_term_query(terms) {
                let query = BoostQuery::new(Box::new(query), 2.0);
                queries.push(Box::new(query));
            }
        }

        if let Some(ref publisher) = self.publisher {
            let terms = get_positions_and_terms(searcher.publisher, publisher, &searcher.tokenizer);
            if let Some(query) = phrase_or_term_query(terms) {
                queries.push(Box::new(query));
            }
        }

        if let Some(ref extension) = self.extension {
            let term =
                Term::from_field_text(searcher.extension, extension.to_ascii_lowercase().trim());
            let query = TermQuery::new(term, IndexRecordOption::Basic);
            queries.push(Box::new(query));
        }

        if let Some(ref language) = self.language {
            let term =
                Term::from_field_text(searcher.language, language.to_ascii_lowercase().trim());
            let query = TermQuery::new(term, IndexRecordOption::WithFreqsAndPositions);
            queries.push(Box::new(query));
        }

        if let Some(ref isbn) = self.isbn {
            let term = Term::from_field_text(searcher.isbn, isbn.to_ascii_lowercase().trim());
            let query = TermQuery::new(term, IndexRecordOption::WithFreqsAndPositions);
            queries.push(Box::new(query));
        }

        if let Some(id) = self.id {
            let term = Term::from_field_u64(searcher.id, id);
            let query = TermQuery::new(term, IndexRecordOption::Basic);
            queries.push(Box::new(query));
        }

        let query = match self.mode {
            SearchMode::Filter => {
                BooleanQuery::new(queries.into_iter().map(|q| (Occur::Must, q)).collect())
            }
            SearchMode::Explore => {
                BooleanQuery::new(queries.into_iter().map(|q| (Occur::Should, q)).collect())
            }
        };

        Ok(Box::new(query))
    }
}

pub(crate) fn get_positions_and_terms(
    field: Field,
    value: &str,
    text_analyzer: &TextAnalyzer,
) -> Vec<(usize, Term)> {
    let mut positions_and_terms = Vec::new();
    let mut token_stream = text_analyzer.token_stream(value);
    token_stream.process(&mut |token| {
        let term = Term::from_field_text(field, &token.text);
        positions_and_terms.push((token.position, term));
    });
    positions_and_terms
}

#[allow(dead_code)]
pub(crate) fn get_terms(field: Field, value: &str, text_analyzer: &TextAnalyzer) -> Vec<Term> {
    let mut terms = Vec::new();
    let mut token_stream = text_analyzer.token_stream(value);
    token_stream.process(&mut |token| {
        let term = Term::from_field_text(field, &token.text);
        terms.push(term);
    });
    terms
}

pub(crate) fn phrase_or_term_query(terms: Vec<(usize, Term)>) -> Option<Box<dyn Query>> {
    if terms.is_empty() {
        return None;
    }

    Some(if terms.len() > 1 {
        Box::new(PhraseQuery::new_with_offset_and_slop(terms, 30))
    } else {
        Box::new(TermQuery::new(
            terms[0].1.clone(),
            IndexRecordOption::WithFreqsAndPositions,
        ))
    })
}
