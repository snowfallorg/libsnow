use anyhow::Result;
use log::debug;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use tantivy::{query::QueryParser, schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions, FAST, STORED, STRING, TEXT}, tokenizer::{SimpleTokenizer, TokenFilter}, DocId, Document, Index, Score, Searcher, SegmentReader, TantivyDocument};

pub struct DbSearcher {
    searcher: Searcher,
    schema: Schema,
    query_parser: QueryParser,
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchResult {
    #[serde(deserialize_with = "deserialize_string")]
    pub attribute: String,
    #[serde(deserialize_with = "deserialize_string_option")]
    pub version: Option<String>,
    #[serde(deserialize_with = "deserialize_string_option")]
    pub pname: Option<String>,
    #[serde(deserialize_with = "deserialize_string_option")]
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_bool")]
    pub broken: bool,
    #[serde(deserialize_with = "deserialize_bool")]
    pub insecure: bool,
    #[serde(deserialize_with = "deserialize_bool")]
    pub unfree: bool,
    #[serde(skip_deserializing)]
    pub score: f32,
}

fn deserialize_string_option<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(mut arr) => {
            if arr.len() == 1 {
                if let Value::String(s) = arr.remove(0) {
                    if s.is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(s))
                    }
                } else {
                    Ok(None)
                }
            } else {
                Err(serde::de::Error::custom("Expected an array with one element"))
            }
        },
        _ => Err(serde::de::Error::custom("Expected an array")),
    }
}

fn deserialize_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(mut arr) => {
            if arr.len() == 1 {
                if let Value::String(s) = arr.remove(0) {
                    Ok(s)
                } else {
                    Err(serde::de::Error::custom("Expected a string"))
                }
            } else {
                Err(serde::de::Error::custom("Expected an array with one element"))
            }
        },
        _ => Err(serde::de::Error::custom("Expected an array")),
    }
}

fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Array(mut arr) => {
            if arr.len() == 1 {
                if let Value::String(b) = arr.remove(0) {
                    Ok(b == "1")
                } else {
                    Ok(false)
                }
            } else {
                Err(serde::de::Error::custom("Expected an array with one element"))
            }
        },
        _ => Err(serde::de::Error::custom("Expected an array")),
    }
}

#[derive(Debug)]
pub struct SearchQuery <'a> {
    pub query: &'a str,
    pub limit: usize,
    pub score_threshold: f32,
}

impl<'a> Default for SearchQuery<'a> {
    fn default() -> Self {
        Self {
            query: "",
            limit: 10,
            score_threshold: 10.0,
        }
    }
}

pub fn get_searcher(db: &rusqlite::Connection) -> Result<DbSearcher> {

    let text_field_indexing = TextFieldIndexing::default()
        .set_tokenizer("ngram3")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_field_indexing);

    // Create a Tantivy schema
    let mut schema_builder = Schema::builder();
    let attribute = schema_builder.add_text_field("attribute", text_options.clone() | STORED );
    let version = schema_builder.add_text_field("version", STORED);
    let pname = schema_builder.add_text_field("pname", text_options.clone() | STORED);
    let description = schema_builder.add_text_field("description", text_options.clone() | STORED);
    let long_description = schema_builder.add_text_field("longdescription", text_options);
    let broken = schema_builder.add_u64_field("broken", STORED);
    let insecure = schema_builder.add_u64_field("insecure", STORED);
    let unfree = schema_builder.add_u64_field("unfree", STORED);

    let schema = schema_builder.build();

    // Create an index in a temporary directory
    let index = Index::create_in_ram(schema.clone());
    index
        .tokenizers()
        .register("ngram3", tantivy::tokenizer::NgramTokenizer::new(3, 3, false)?);

    let mut index_writer = index.writer(50_000_000)?;

    // Query to select data from SQLite
    let mut stmt = db.prepare("SELECT pkgs.attribute, pkgs.version, pkgs.pname, meta.description, meta.long_description, meta.broken, meta.insecure, meta.unfree FROM pkgs JOIN meta ON pkgs.attribute = meta.attribute")?;
    let meta_iter = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1),
            row.get::<_, Option<String>>(2),
            row.get::<_, Option<String>>(3),
            row.get::<_, Option<String>>(4),
            row.get::<_, Option<u64>>(5),
            row.get::<_, Option<u64>>(6),
            row.get::<_, Option<u64>>(7),
        ))
    })?;

    // Add documents to the index
    for meta in meta_iter {
        let (attr, ver, pnm, desc, long_desc, brk, insec, unfr) = meta?;
        let mut doc = TantivyDocument::default();
        doc.add_text(attribute, &attr);
        if let Ok(Some(v)) = ver {
            doc.add_text(version, &v);
        }
        if let Ok(Some(pn)) = pnm {
            doc.add_text(pname, &pn);
        }
        if let Ok(Some(d)) = desc {
            doc.add_text(description, &d);
        }
        if let Ok(Some(ld)) = long_desc {
            doc.add_text(long_description, &ld);
        }
        if let Ok(Some(b)) = brk {
            doc.add_text(broken, &b);
        }
        if let Ok(Some(i)) = insec {
            doc.add_text(insecure, &i);
        }
        if let Ok(Some(u)) = unfr {
            doc.add_text(unfree, &u);
        }
        index_writer.add_document(doc)?;
    }
    index_writer.commit()?;

    // Search in the index
    let reader = index.reader()?;
    let searcher = reader.searcher();
    let mut query_parser =
        QueryParser::for_index(&index, vec![attribute, description, long_description]);
    query_parser.set_field_boost(attribute, 100.0);
    Ok(DbSearcher {
        searcher,
        schema,
        query_parser,
    })
}

pub fn search(sq: &SearchQuery, dbsearcher: &DbSearcher) -> Result<Vec<SearchResult>> {
    let DbSearcher { searcher, schema, query_parser } = dbsearcher;

    let (query, _) = query_parser.parse_query_lenient(&sq.query.trim());
    let top_docs: Vec<(f32, tantivy::DocAddress)> =
        searcher.search(&query, &tantivy::collector::TopDocs::with_limit(sq.limit))?;

    let mut results = Vec::new();

    for (score, doc_address) in top_docs {
        if score < sq.score_threshold {
            break;
        }
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
        debug!("Search result: {}", retrieved_doc.to_json(&schema));
        let search_result: SearchResult = serde_json::from_str(&retrieved_doc.to_json(&schema))?;
        results.push(SearchResult {
            score,
            ..search_result
        });
    }
    Ok(results)
}
