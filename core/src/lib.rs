#[cfg(feature = "ssr")]
use sqlx::mysql::MySqlPool;
use serde::{Deserialize, Serialize};
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
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
    pub psc: i32,
    pub souradnice_y: Option<f64>,
    pub souradnice_x: Option<f64>,
    pub plati_od: NaiveDateTime,
    pub search: String,
}

#[cfg(feature = "ssr")]
pub async fn create_pool() -> Result<MySqlPool, sqlx::Error> {
    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    MySqlPool::connect(&database_url).await
}

pub fn normalize(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut last_was_space = false;

    for c in s.chars() {
        let mapped = match c {
            'á' | 'ä' | 'Á' | 'Ä' => Some('a'),
            'č' | 'Č' => Some('c'),
            'ď' | 'Ď' => Some('d'),
            'é' | 'ě' | 'ë' | 'É' | 'Ě' | 'Ë' => Some('e'),
            'í' | 'Í' => Some('i'),
            'ň' | 'Ň' => Some('n'),
            'ó' | 'ö' | 'Ó' | 'Ö' => Some('o'),
            'ř' | 'Ř' => Some('r'),
            'š' | 'Š' => Some('s'),
            'ť' | 'Ť' => Some('t'),
            'ú' | 'ů' | 'ü' | 'Ú' | 'Ů' | 'Ü' => Some('u'),
            'ý' | 'Ý' => Some('y'),
            'ž' | 'Ž' => Some('z'),
            '/' | '_' => Some('l'),
            _ if c.is_alphanumeric() => Some(c.to_ascii_lowercase()),
            _ => None,
        };

        match mapped {
            Some(m) => {
                result.push(m);
                last_was_space = false;
            }
            None => {
                if !last_was_space && !result.is_empty() {
                    result.push(' ');
                    last_was_space = true;
                }
            }
        }
    }

    result.trim().to_string()
}

pub fn pad_token(token: &str) -> String {
    let t = token.trim();

    if t.is_empty() {
        return String::new();
    }

    if t == "as" || t == "es" {
        return format!("xxx{}", t);
    }

    if t.chars().all(|c| c.is_numeric()) {
        return format!("xxx{}", t);
    } else {
        return t.to_string();
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pad_token() {
        assert_eq!(pad_token("as"), "xxxas"); // Aš
        assert_eq!(pad_token("es"), "xxxes"); // Eš
        assert_eq!(pad_token("1"), "xxx1");    
        assert_eq!(pad_token("17"), "xxx17");  
        assert_eq!(pad_token("52l13"), "52l13"); // lomítko
        assert_eq!(pad_token("praha"), "praha");
        assert_eq!(pad_token(""), "");
    }
}