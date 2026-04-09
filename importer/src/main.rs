use std::fs;
use std::collections::HashSet;
use std::string::String as StdString;
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

fn parse_psc(raw: &str) -> i32 {
    let digits: StdString = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 5 {
        digits.as_str().parse::<i32>().unwrap_or(0)
    } else {
        0
    }
}

fn extract_first_number(value: &str) -> Option<i32> {
    normalize(value)
        .as_str()
        .split_whitespace()
        .find_map(|token| {
            if token.chars().all(|c| c.is_ascii_digit()) {
                token.parse::<i32>().ok()
            } else {
                None
            }
        })
}

fn extract_praha_obvod_number(
    nazev_obvodu_prahy: Option<&str>,
    nazev_momc: Option<&str>,
    kod_obvodu_prahy: Option<i32>,
) -> Option<i32> {
    if let Some(name) = nazev_obvodu_prahy {
        if let Some(n) = extract_first_number(name) {
            return Some(n);
        }
    }

    if let Some(name) = nazev_momc {
        if let Some(n) = extract_first_number(name) {
            return Some(n);
        }
    }

    kod_obvodu_prahy.filter(|v| (1..=22).contains(v))
}

fn domovni_orientacni_key(domovni: i32, orientacni: Option<i32>, znak: Option<&str>) -> Option<String> {
    orientacni.map(|o| {
        let znak = znak.unwrap_or("").trim().to_ascii_lowercase();
        if znak.is_empty() {
            format!("{domovni}_{o}")
        } else {
            format!("{domovni}_{o}{znak}")
        }
    })
}

fn append_unique_token(search: &mut StdString, seen: &mut HashSet<StdString>, token: StdString) {
    if token.is_empty() {
        return;
    }

    if seen.contains(token.as_str()) {
        return;
    }

    if !search.is_empty() {
        search.push(' ');
    }
    search.push_str(token.as_str());
    seen.insert(token);
}

fn append_normalized_tokens(search: &mut StdString, seen: &mut HashSet<StdString>, value: Option<&str>) {
    if let Some(v) = value {
        let normalized = normalize(v);
        for token in normalized.as_str().split_whitespace() {
            append_unique_token(search, seen, pad_token(token));
        }
    }
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

        let mut batch = Vec::with_capacity(1000);

        for result in rdr.deserialize() {
            let record: CsvAdresa = result.context("Failed to parse CSV record")?;
            
            let plati_od = NaiveDateTime::parse_from_str(&record.plati_od, "%Y-%m-%dT%H:%M:%S")
                .context("Failed to parse date")?;

            let psc = parse_psc(&record.psc);
            let mut search = StdString::new();
            let mut seen = HashSet::new();

            append_normalized_tokens(&mut search, &mut seen, record.nazev_ulice.as_deref());
            append_normalized_tokens(&mut search, &mut seen, Some(&record.nazev_obce));
            append_normalized_tokens(&mut search, &mut seen, record.nazev_casti_obce.as_deref());
            append_normalized_tokens(&mut search, &mut seen, record.nazev_momc.as_deref());
            append_normalized_tokens(&mut search, &mut seen, record.nazev_obvodu_prahy.as_deref());

            append_unique_token(&mut search, &mut seen, pad_token(&record.cislo_domovni.to_string()));
            if let Some(orient) = record.cislo_orientacni {
                append_unique_token(&mut search, &mut seen, pad_token(&orient.to_string()));
                
                let znak = record.znak_cisla_orientacniho.as_deref().unwrap_or("").trim();
                if !znak.is_empty() {
                    let orient_znak = format!("{}{}", orient, znak);
                    append_unique_token(&mut search, &mut seen, pad_token(&orient_znak));
                }

                append_unique_token(&mut search, &mut seen, format!("{}_{}", record.cislo_domovni, orient));
                append_unique_token(&mut search, &mut seen, format!("{}_{}", orient, record.cislo_domovni));

                if !znak.is_empty() {
                    append_unique_token(&mut search, &mut seen, format!("{}_{}{}", record.cislo_domovni, orient, znak));
                }
            }

            if psc > 0 {
                append_unique_token(&mut search, &mut seen, psc.to_string());
                append_unique_token(&mut search, &mut seen, format!("{:03}", psc / 100));
                append_unique_token(&mut search, &mut seen, format!("{:02}", psc % 100));
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
                psc: psc,
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
            znak_cisla_orientacniho, psc,
            ulice_cislo, obvod_prahy_cislo, domovni_orientacni_klic,
            souradnice_y, souradnice_x, plati_od, search
        ) "
    );

    query_builder.push_values(batch, |mut b, adresa| {
        let ulice_cislo = adresa.nazev_ulice.as_deref().and_then(extract_first_number);
        let obvod_prahy_cislo = extract_praha_obvod_number(
            adresa.nazev_obvodu_prahy.as_deref(),
            adresa.nazev_momc.as_deref(),
            adresa.kod_obvodu_prahy,
        );
        let domovni_orientacni_klic = domovni_orientacni_key(
            adresa.cislo_domovni,
            adresa.cislo_orientacni,
            adresa.znak_cisla_orientacniho.as_deref(),
        );

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
            .push_bind(ulice_cislo)
            .push_bind(obvod_prahy_cislo)
            .push_bind(domovni_orientacni_klic)
            .push_bind(adresa.souradnice_y)
            .push_bind(adresa.souradnice_x)
            .push_bind(adresa.plati_od)
            .push_bind(&adresa.search);
    });

    let query = query_builder.build();
    query.execute(pool).await.context("Failed to insert batch of Adresa records")?;

    Ok(())
}