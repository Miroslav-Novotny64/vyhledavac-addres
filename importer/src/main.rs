use std::fs;
use anyhow::{Context, Result};
use sqlx::MySqlPool;
use core_db::{create_pool, Adresa, normalize, pad_token};

use encoding_rs::WINDOWS_1250;
use encoding_rs_io::DecodeReaderBytesBuilder;
use serde::Deserialize;
use sqlx::types::chrono::NaiveDateTime;

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
    psc: String,
    #[serde(rename = "Souřadnice Y")]
    souradnice_y: Option<f64>,
    #[serde(rename = "Souřadnice X")]
    souradnice_x: Option<f64>,
    #[serde(rename = "Platí Od")]
    plati_od: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let pool = create_pool().await.context("Failed to connect to database")?;

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

        let mut batch = Vec::with_capacity(1000);

        for result in rdr.deserialize() {
            let record: CsvAdresa = result.context("Failed to parse CSV record")?;
            
            let plati_od = NaiveDateTime::parse_from_str(&record.plati_od, "%Y-%m-%dT%H:%M:%S")
                .context("Failed to parse date")?;

            let mut search_parts = Vec::new();
            if let Some(ref ulice) = record.nazev_ulice {
                search_parts.push(normalize(ulice));
            }

            search_parts.push(record.cislo_domovni.to_string());

            if let Some(orient) = record.cislo_orientacni {
                search_parts.push(orient.to_string());

                search_parts.push(format!("{}/{}", record.cislo_domovni, orient));
                search_parts.push(format!("{}/{}", orient, record.cislo_domovni));
            }

            search_parts.push(normalize(&record.nazev_obce));

            if let Some(ref cast) = record.nazev_casti_obce {
                if cast != &record.nazev_obce {
                    search_parts.push(normalize(cast));
                }
            }

            search_parts.push(record.psc.clone());

            let search = search_parts.join(" ")
                .split_whitespace()
                .map(|t| pad_token(t))
                .collect::<Vec<_>>()
                .join(" ");

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
                plati_od,
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
            znak_cisla_orientacniho, psc, souradnice_y, souradnice_x, plati_od, search
        ) "
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
            .push_bind(&adresa.psc)
            .push_bind(adresa.souradnice_y)
            .push_bind(adresa.souradnice_x)
            .push_bind(adresa.plati_od)
            .push_bind(&adresa.search);
    });

    let query = query_builder.build();
    query.execute(pool).await.context("Failed to insert batch of Adresa records")?;

    Ok(())
}