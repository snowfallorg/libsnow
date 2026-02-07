use anyhow::{Context, Result};
use log::debug;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::{fs, path::Path};
use tantivy::{
    Document, Index, Searcher, TantivyDocument, Term,
    query::{BooleanQuery, BoostQuery, Occur, Query, QueryParser, TermQuery},
    schema::{IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing, TextOptions},
};

pub struct DbSearcher {
    searcher: Searcher,
    schema: Schema,
    fuzzy_parser: QueryParser,
    attr_exact: tantivy::schema::Field,
    attr_ngram: tantivy::schema::Field,
}

struct SearchFields {
    attr_ngram: tantivy::schema::Field,
    version: tantivy::schema::Field,
    pname: tantivy::schema::Field,
    description: tantivy::schema::Field,
    broken: tantivy::schema::Field,
    insecure: tantivy::schema::Field,
    unfree: tantivy::schema::Field,
    attr_exact: tantivy::schema::Field,
    attr_default: tantivy::schema::Field,
    pname_default: tantivy::schema::Field,
    desc_default: tantivy::schema::Field,
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
                    if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
                } else {
                    Ok(None)
                }
            } else {
                Err(serde::de::Error::custom(
                    "Expected an array with one element",
                ))
            }
        }
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
                Err(serde::de::Error::custom(
                    "Expected an array with one element",
                ))
            }
        }
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
                match arr.remove(0) {
                    Value::String(b) => Ok(b == "1" || b.eq_ignore_ascii_case("true")),
                    Value::Number(n) => Ok(n.as_u64().unwrap_or(0) != 0),
                    Value::Bool(b) => Ok(b),
                    _ => Ok(false),
                }
            } else {
                Err(serde::de::Error::custom(
                    "Expected an array with one element",
                ))
            }
        }
        _ => Err(serde::de::Error::custom("Expected an array")),
    }
}

#[derive(Debug)]
pub struct SearchQuery<'a> {
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

fn build_schema() -> (Schema, SearchFields) {
    let ngram_indexing = TextFieldIndexing::default()
        .set_tokenizer("ngram3")
        .set_index_option(IndexRecordOption::WithFreqs);
    let ngram_opts = TextOptions::default().set_indexing_options(ngram_indexing);

    let default_indexing = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqs);
    let default_opts = TextOptions::default().set_indexing_options(default_indexing);

    let mut schema_builder = Schema::builder();

    let attr_ngram = schema_builder.add_text_field("attribute", ngram_opts | STORED);
    let version = schema_builder.add_text_field("version", STORED);
    let pname = schema_builder.add_text_field("pname", STORED);
    let description = schema_builder.add_text_field("description", STORED);
    let broken = schema_builder.add_i64_field("broken", STORED);
    let insecure = schema_builder.add_i64_field("insecure", STORED);
    let unfree = schema_builder.add_i64_field("unfree", STORED);

    let attr_exact = schema_builder.add_text_field("attribute_exact", STRING);
    let attr_default = schema_builder.add_text_field("attribute_default", default_opts.clone());
    let pname_default = schema_builder.add_text_field("pname_default", default_opts.clone());
    let desc_default = schema_builder.add_text_field("description_default", default_opts);

    let schema = schema_builder.build();

    (
        schema,
        SearchFields {
            attr_ngram,
            version,
            pname,
            description,
            broken,
            insecure,
            unfree,
            attr_exact,
            attr_default,
            pname_default,
            desc_default,
        },
    )
}

fn fields_from_schema(schema: &Schema) -> Result<SearchFields> {
    let attr_ngram = schema
        .get_field("attribute")
        .context("missing field: attribute")?;
    let version = schema
        .get_field("version")
        .context("missing field: version")?;
    let pname = schema.get_field("pname").context("missing field: pname")?;
    let description = schema
        .get_field("description")
        .context("missing field: description")?;
    let broken = schema
        .get_field("broken")
        .context("missing field: broken")?;
    let insecure = schema
        .get_field("insecure")
        .context("missing field: insecure")?;
    let unfree = schema
        .get_field("unfree")
        .context("missing field: unfree")?;

    let attr_exact = schema
        .get_field("attribute_exact")
        .context("missing field: attribute_exact")?;
    let attr_default = schema
        .get_field("attribute_default")
        .context("missing field: attribute_default")?;
    let pname_default = schema
        .get_field("pname_default")
        .context("missing field: pname_default")?;
    let desc_default = schema
        .get_field("description_default")
        .context("missing field: description_default")?;

    Ok(SearchFields {
        attr_ngram,
        version,
        pname,
        description,
        broken,
        insecure,
        unfree,
        attr_exact,
        attr_default,
        pname_default,
        desc_default,
    })
}

fn register_tokenizers(index: &Index) -> Result<()> {
    index.tokenizers().register(
        "ngram3",
        tantivy::tokenizer::NgramTokenizer::new(3, 3, false)?,
    );
    Ok(())
}

fn fill_index(
    db: &rusqlite::Connection,
    fields: &SearchFields,
    index_writer: &mut tantivy::IndexWriter,
) -> Result<()> {
    let mut stmt = db.prepare(
        "SELECT pkgs.attribute, pkgs.version, pkgs.pname, \
         meta.description, \
         meta.broken, meta.insecure, meta.unfree \
         FROM pkgs JOIN meta ON pkgs.attribute = meta.attribute",
    )?;
    let meta_iter = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1),
            row.get::<_, Option<String>>(2),
            row.get::<_, Option<String>>(3),
            row.get::<_, Option<i64>>(4),
            row.get::<_, Option<i64>>(5),
            row.get::<_, Option<i64>>(6),
        ))
    })?;

    for meta in meta_iter {
        let (attr, ver, pnm, desc, brk, insec, unfr) = meta?;
        let mut doc = TantivyDocument::default();

        doc.add_text(fields.attr_ngram, &attr);
        if let Ok(Some(v)) = &ver {
            doc.add_text(fields.version, v);
        }
        if let Ok(Some(pn)) = &pnm {
            doc.add_text(fields.pname, pn);
        }
        if let Ok(Some(d)) = &desc {
            doc.add_text(fields.description, d);
        }
        if let Ok(Some(b)) = brk {
            doc.add_i64(fields.broken, b);
        }
        if let Ok(Some(i)) = insec {
            doc.add_i64(fields.insecure, i);
        }
        if let Ok(Some(u)) = unfr {
            doc.add_i64(fields.unfree, u);
        }

        doc.add_text(fields.attr_exact, &attr);
        let attr_tokens = attr.replace(['.', '-', '_'], " ");
        doc.add_text(fields.attr_default, &attr_tokens);

        if let Ok(Some(pn)) = &pnm {
            let pname_tokens = pn.replace(['-', '_'], " ");
            doc.add_text(fields.pname_default, &pname_tokens);
        }
        if let Ok(Some(d)) = &desc {
            doc.add_text(fields.desc_default, d);
        }

        index_writer.add_document(doc)?;
    }

    Ok(())
}

fn build_searcher_from_index(index: &Index) -> Result<DbSearcher> {
    let schema = index.schema();
    let fields = fields_from_schema(&schema)?;

    let reader = index.reader()?;
    let searcher = reader.searcher();

    let mut fuzzy_parser = QueryParser::for_index(
        index,
        vec![
            fields.attr_default,
            fields.pname_default,
            fields.desc_default,
        ],
    );
    fuzzy_parser.set_field_fuzzy(fields.attr_default, true, 1, true);
    fuzzy_parser.set_field_fuzzy(fields.pname_default, true, 1, true);
    fuzzy_parser.set_field_fuzzy(fields.desc_default, false, 1, true);
    fuzzy_parser.set_field_boost(fields.attr_default, 200.0);
    fuzzy_parser.set_field_boost(fields.pname_default, 150.0);
    fuzzy_parser.set_field_boost(fields.desc_default, 5.0);

    Ok(DbSearcher {
        searcher,
        schema,
        fuzzy_parser,
        attr_exact: fields.attr_exact,
        attr_ngram: fields.attr_ngram,
    })
}

pub fn build_search_index_in_dir(db: &rusqlite::Connection, index_dir: &Path) -> Result<()> {
    if index_dir.exists() {
        fs::remove_dir_all(index_dir)?;
    }
    fs::create_dir_all(index_dir)?;

    let (schema, fields) = build_schema();
    let index = Index::create_in_dir(index_dir, schema)?;
    register_tokenizers(&index)?;

    let mut index_writer = index.writer(100_000_000)?;
    fill_index(db, &fields, &mut index_writer)?;
    index_writer.commit()?;

    Ok(())
}

pub(crate) fn get_searcher_from_dir(index_dir: &Path) -> Result<DbSearcher> {
    let index = Index::open_in_dir(index_dir)?;
    register_tokenizers(&index)?;
    build_searcher_from_index(&index)
}

pub fn index_dir_for_db_path(db_path: &Path) -> std::path::PathBuf {
    db_path.with_extension("index")
}

fn build_ngram_query(field: tantivy::schema::Field, query: &str) -> Box<dyn Query> {
    let chars: Vec<char> = query.chars().collect();
    if chars.len() < 3 {
        return Box::new(BooleanQuery::new(vec![]));
    }
    let terms: Vec<(Occur, Box<dyn Query>)> = chars
        .windows(3)
        .map(|w| {
            let ngram: String = w.iter().collect();
            let term = Term::from_field_text(field, &ngram);
            let tq: Box<dyn Query> = Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
            (Occur::Should, tq)
        })
        .collect();
    Box::new(BooleanQuery::new(terms))
}

pub(crate) fn search(sq: &SearchQuery, dbsearcher: &DbSearcher) -> Result<Vec<SearchResult>> {
    let DbSearcher {
        searcher,
        schema,
        fuzzy_parser,
        attr_exact,
        attr_ngram,
    } = dbsearcher;

    let query_str = sq.query.trim();
    if query_str.is_empty() {
        return Ok(Vec::new());
    }

    let query_lower = query_str.to_lowercase();

    let exact_term = Term::from_field_text(*attr_exact, &query_lower);
    let exact_query: Box<dyn Query> = Box::new(BoostQuery::new(
        Box::new(TermQuery::new(exact_term, IndexRecordOption::Basic)),
        1000.0,
    ));

    let (fuzzy_query, _) = fuzzy_parser.parse_query_lenient(&query_lower);
    let ngram_query = build_ngram_query(*attr_ngram, &query_lower);

    let combined = BooleanQuery::new(vec![
        (Occur::Should, exact_query),
        (Occur::Should, fuzzy_query),
        (Occur::Should, ngram_query),
    ]);

    // Fetch extra candidates for post-hoc re-ranking by attribute length.
    let fetch_limit = sq.limit * 4;

    let top_docs: Vec<(f32, tantivy::DocAddress)> = searcher.search(
        &combined,
        &tantivy::collector::TopDocs::with_limit(fetch_limit),
    )?;

    let mut results = Vec::new();
    let query_len = query_str.len() as f32;

    for (score, doc_address) in top_docs {
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
        debug!("Search result: {}", retrieved_doc.to_json(schema));
        let search_result: SearchResult = serde_json::from_str(&retrieved_doc.to_json(schema))?;

        // Penalize long attribute names
        let attr_len = search_result.attribute.len().max(1) as f32;
        let ratio = (attr_len / query_len).max(1.0);
        let length_penalty = 1.0 + ratio.ln();
        let adjusted_score = score / length_penalty;

        if adjusted_score < sq.score_threshold {
            continue;
        }

        results.push(SearchResult {
            score: adjusted_score,
            ..search_result
        });
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(sq.limit);

    Ok(results)
}
