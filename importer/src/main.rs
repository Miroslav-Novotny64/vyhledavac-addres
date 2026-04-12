use std::fs;
use std::string::String as StdString;

use anyhow::{Context, Result};
use core_db::{create_pool, normalize, pad_token, Adresa};
use encoding_rs::WINDOWS_1250;
use encoding_rs_io::DecodeReaderBytesBuilder;
use serde::Deserialize;
use sqlx::{MySqlPool, types::chrono::NaiveDateTime};

#[derive(Debug, Deserialize)]
struct CsvAdresa {
    #[serde(rename = "Kód ADM")]
    kod_adm: i32,
    #[serde(rename = "Kód obce")]
    kod_obce: i32,
    #[serde(rename = "Název obce")]
    nazev_obce: String,
    #[serde(rename = "Kód MOMC")]
    kod_momc: Option<i32>,
    #[serde(rename = "Název MOMC")]
    nazev_momc: Option<String>,
    #[serde(rename = "Kód obvodu Prahy")]
    kod_obvodu_prahy: Option<i32>,
    #[serde(rename = "Název obvodu Prahy")]
    nazev_obvodu_prahy: Option<String>,
    #[serde(rename = "Kód části obce")]
    kod_casti_obce: Option<i32>,
    #[serde(rename = "Název části obce")]
    nazev_casti_obce: Option<String>,
    #[serde(rename = "Kód ulice")]
    kod_ulice: Option<i32>,
    #[serde(rename = "Název ulice")]
    nazev_ulice: Option<String>,
    #[serde(rename = "Typ SO")]
    typ_so: String,
    #[serde(rename = "Číslo domovní")]
    cislo_domovni: i32,
    #[serde(rename = "Číslo orientační")]
    cislo_orientacni: Option<i32>,
    #[serde(rename = "Znak čísla orientačního")]
    znak_cisla_orientacniho: Option<String>,
    #[serde(rename = "PSČ")]
    psc: i32,
    #[serde(rename = "Souřadnice Y")]
    souradnice_y: Option<f64>,
    #[serde(rename = "Souřadnice X")]
    souradnice_x: Option<f64>,
    #[serde(rename = "Platí Od")]
    plati_od: NaiveDateTime,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = create_pool().await.context("Failed to connect to database")?;

    sqlx::query("DROP TABLE IF EXISTS adresa").execute(&pool).await?;
    sqlx::query(include_str!("../../core/schema.sql")).execute(&pool).await?;

    import(&pool).await?;
    Ok(())
}

async fn import(pool: &MySqlPool) -> Result<()> {
    let paths = fs::read_dir("./data/").context("Failed to read directory")?;

    for path_result in paths {
        let entry = path_result.context("Failed to read directory entry")?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("csv") {
            continue;
        }

        println!("Importing: {}", path.display());

        let file = fs::File::open(&path)?;
        let transcoded = DecodeReaderBytesBuilder::new()
            .encoding(Some(WINDOWS_1250))
            .build(file);

        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b';')
            .from_reader(transcoded);

        let mut batch = Vec::with_capacity(20000);

        for result in rdr.deserialize() {
            let record: CsvAdresa = result.context("Failed to parse CSV record")?;

            let mut raw_terms: Vec<String> = Vec::new();

            if let Some(v) = record.nazev_ulice.as_deref() {
                raw_terms.push(v.to_string());
            }
            raw_terms.push(record.nazev_obce.clone());
            if let Some(v) = record.nazev_casti_obce.as_deref() {
                raw_terms.push(v.to_string());
            }
            if let Some(v) = record.nazev_obvodu_prahy.as_deref() {
                raw_terms.push(v.to_string());
            }
            if let Some(v) = record.nazev_momc.as_deref() {
                raw_terms.push(v.to_string());
            }

            let domovni = record.cislo_domovni.to_string();
            raw_terms.push(domovni.clone());

            if let Some(or) = record.cislo_orientacni {
                let znak = record
                    .znak_cisla_orientacniho
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase();

                let orientacni_znak = if znak.is_empty() {
                    or.to_string()
                } else {
                    format!("{or}{znak}")
                };

                // orientacni+znak
                raw_terms.push(orientacni_znak.clone());
                // domovni_orientacni+znak
                raw_terms.push(format!("{}/{}", domovni, orientacni_znak));
                // orientacni+znak_domovni
                raw_terms.push(format!("{}/{}", orientacni_znak, domovni));

                if !znak.is_empty() {
                    raw_terms.push(znak);
                }
            }

            let psc_str = record.psc.to_string();
            raw_terms.push(psc_str.clone());
            if psc_str.len() == 5 {
                raw_terms.push(psc_str[0..3].to_string());
                raw_terms.push(psc_str[3..5].to_string());
            }

            let mut search = StdString::new();

            for term in raw_terms {
                for tok in term.split_whitespace() {
                    let normalized = normalize(tok);
                    if normalized.is_empty() {
                        continue;
                    }

                    let padded = pad_token(normalized.as_str());
                    if padded.is_empty() {
                        continue;
                    }

                    if !search.is_empty() {
                        search.push(' ');
                    }
                    search.push_str(padded.as_str());
                }
            }

            batch.push(Adresa {
                kod_adm: record.kod_adm,
                kod_obce: record.kod_obce,
                nazev_obce: record.nazev_obce,
                kod_momc: record.kod_momc,
                nazev_momc: record.nazev_momc,
                kod_obvodu_prahy: record.kod_obvodu_prahy,
                nazev_obvodu_prahy: record.nazev_obvodu_prahy,
                kod_casti_obce: record.kod_casti_obce,
                nazev_casti_obce: record.nazev_casti_obce,
                kod_ulice: record.kod_ulice,
                nazev_ulice: record.nazev_ulice,
                typ_so: record.typ_so,
                cislo_domovni: record.cislo_domovni,
                cislo_orientacni: record.cislo_orientacni,
                znak_cisla_orientacniho: record.znak_cisla_orientacniho,
                psc: record.psc,
                souradnice_y: record.souradnice_y,
                souradnice_x: record.souradnice_x,
                plati_od: record.plati_od,
                search,
            });

            if batch.len() >= 2000 {
                insert_batch(pool, &batch).await?;
                batch.clear();
            }
        }

        if !batch.is_empty() {
            insert_batch(pool, &batch).await?;
        }
    }

    Ok(())
}

pub async fn insert_batch(pool: &MySqlPool, batch: &[Adresa]) -> Result<()> {
    let mut query_builder = sqlx::QueryBuilder::new(
        "INSERT INTO adresa (
            kod_adm, kod_obce, nazev_obce, kod_momc, nazev_momc,
            kod_obvodu_prahy, nazev_obvodu_prahy, kod_casti_obce, nazev_casti_obce,
            kod_ulice, nazev_ulice, typ_so, cislo_domovni, cislo_orientacni,
            znak_cisla_orientacniho, psc,
            souradnice_y, souradnice_x, plati_od, search
        ) ",
    );

    query_builder.push_values(batch, |mut b, adresa| {
        b.push_bind(adresa.kod_adm)
            .push_bind(adresa.kod_obce)
            .push_bind(&adresa.nazev_obce)
            .push_bind(adresa.kod_momc)
            .push_bind(&adresa.nazev_momc)
            .push_bind(adresa.kod_obvodu_prahy)
            .push_bind(&adresa.nazev_obvodu_prahy)
            .push_bind(adresa.kod_casti_obce)
            .push_bind(&adresa.nazev_casti_obce)
            .push_bind(adresa.kod_ulice)
            .push_bind(&adresa.nazev_ulice)
            .push_bind(&adresa.typ_so)
            .push_bind(adresa.cislo_domovni)
            .push_bind(adresa.cislo_orientacni)
            .push_bind(&adresa.znak_cisla_orientacniho)
            .push_bind(adresa.psc)
            .push_bind(adresa.souradnice_y)
            .push_bind(adresa.souradnice_x)
            .push_bind(&adresa.plati_od)
            .push_bind(&adresa.search);
    });

    let query = query_builder.build();
    query
        .execute(pool)
        .await
        .context("Failed to insert batch of Adresa records")?;

    Ok(())
}