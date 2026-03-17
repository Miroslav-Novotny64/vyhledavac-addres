use sqlx::mysql::MySqlPool;
use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Adresa {
    pub kod_adm: i32,
    pub kod_obce: i32,
    pub nazev_obce: String,
    pub kod_momc: Option<i32>,
    pub nazev_momc: Option<String>,
    pub kod_obvodu_prahy: Option<i32>,
    pub nazev_obvodu_prahy: Option<String>,
    pub kod_casti_obce: Option<i32>,
    pub nazev_casti_obce: Option<String>,
    pub kod_ulice: Option<i32>,
    pub nazev_ulice: Option<String>,
    pub typ_so: String,
    pub cislo_domovni: i32,
    pub cislo_orientacni: Option<i32>,
    pub znak_cisla_orientacniho: Option<String>,
    pub psc: String,
    pub souradnice_y: Option<f64>,
    pub souradnice_x: Option<f64>,
    pub plati_od: NaiveDateTime,
    pub search: String,
}

pub async fn create_pool() -> Result<MySqlPool, sqlx::Error> {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MySqlPool::connect(&database_url).await
}
