#[cfg(all(test, feature = "ssr"))]
mod tests {
    use crate::components::search_input::search_adresa_impl;
    use core_db::Adresa;
    use sqlx::mysql::MySqlPool;
    use std::env;

    async fn get_test_pool() -> MySqlPool {
        dotenvy::dotenv().ok();
        let url = env::var("DATABASE_URL").expect("DATABASE_URL musí být nastavena pro testy");
        MySqlPool::connect(&url).await.expect("Selhalo připojení k testovací databázi")
    }

    /// Pomocná funkce pro ověření, zda výsledky hledání obsahují daný řetězec
    fn assert_contains_address(results: &[Adresa], expected_part: &str) {
        let found = results.iter().any(|res| {
            let street = res.nazev_ulice.clone()
                .or_else(|| res.nazev_casti_obce.clone())
                .unwrap_or_else(|| res.nazev_obce.clone());

            let numbers = if let Some(orient) = res.cislo_orientacni {
                let znak = res.znak_cisla_orientacniho.as_deref().unwrap_or("");
                format!("{}/{}{}", res.cislo_domovni, orient, znak)
            } else {
                res.cislo_domovni.to_string()
            };

            let psc_formatted = res.psc.to_string();
            // PSČ v DB je bez mezer, ale v UI bývá s mezerou (XXX XX).
            let psc_with_space = if psc_formatted.len() == 5 {
                format!("{} {}", &psc_formatted[0..3], &psc_formatted[3..5])
            } else {
                psc_formatted.clone()
            };

            let full = format!("{} {} {} {} {}", 
                street, 
                numbers, 
                res.nazev_obce, 
                psc_formatted,
                psc_with_space
            );
            
            // Normalizace pro porovnání: vše na lowercase, odstranit mezery, nahradit / za _
            let normalize = |s: &str| s.to_lowercase().replace(" ", "").replace("/", "_");
            normalize(&full).contains(&normalize(expected_part))
        });
        
        let first_res = results.first().map(|r| {
            format!("{} {}/{} {} {}", 
                r.nazev_ulice.as_deref().unwrap_or("?"), 
                r.cislo_domovni, 
                r.cislo_orientacni.unwrap_or(0),
                r.nazev_obce,
                r.psc
            )
        });

        assert!(found, "Výsledky neobsahují očekávanou adresu: '{}'. Počet výsledků: {}. První výsledek: {:?}", 
            expected_part, results.len(), first_res
        );
    }

    #[tokio::test]
    async fn test_17_listopadu_extra_numbers() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "17. listopadu 30/8 Říčany 25101".to_string()).await.unwrap();
        assert_contains_address(&results, "17. listopadu");
        assert_contains_address(&results, "30");
        assert_contains_address(&results, "8");
    }

    #[tokio::test]
    async fn test_mirova_partial_match() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "mírová 1 Říčany".to_string()).await.unwrap();
        assert_contains_address(&results, "Mírová");
        assert_contains_address(&results, "1");
    }

    #[tokio::test]
    async fn test_vyletni_psc_matching() {
        let pool = get_test_pool().await;
        let res1 = search_adresa_impl(&pool, "výletní 251".to_string()).await.unwrap();
        assert_contains_address(&res1, "Výletní");
        assert_contains_address(&res1, "25162");

        let res2 = search_adresa_impl(&pool, "výletní 251 62".to_string()).await.unwrap();
        assert_contains_address(&res2, "Výletní");
        assert_contains_address(&res2, "25162");
    }

    #[tokio::test]
    async fn test_roztylska_exact_house() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "Roztylská 1860/1".to_string()).await.unwrap();
        assert_contains_address(&results, "Roztylská");
        assert_contains_address(&results, "1860/1");
        assert_contains_address(&results, "148 00");
    }

    #[tokio::test]
    async fn test_swapped_numbers() {
        let pool = get_test_pool().await;
        // Test prohozených čísel (vstup 1/38 najde 38/1)
        let results = search_adresa_impl(&pool, "Rýdlova 1/38".to_string()).await.unwrap();
        assert_contains_address(&results, "Rýdlova");
        assert_contains_address(&results, "38/1");
    }

    #[tokio::test]
    async fn test_street_typo_with_numbers() {
        let pool = get_test_pool().await;
        // Záměrně vrací empty pro překlep v kombinaci s čísly
        let results = search_adresa_impl(&pool, "Rýdlovva 1/38".to_string()).await.unwrap();
        assert!(results.is_empty(), "Očekáváno prázdné výsledky pro překlep dle zadání");
    }

    #[tokio::test]
    async fn test_praha_obvod_matching() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "Budějovická 779 Praha 4".to_string()).await.unwrap();
        assert_contains_address(&results, "Budějovická");
        assert_contains_address(&results, "779");
        assert_contains_address(&results, "Praha");
    }

    #[tokio::test]
    async fn test_just_numbers_and_psc() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "25101 1 38".to_string()).await.unwrap();
        assert!(results.is_empty(), "Očekáváno prázdné výsledky pro samotná čísla bez ulice");
    }

    #[tokio::test]
    async fn test_orientation_prefix_matching() {
        let pool = get_test_pool().await;
        // Test doplňování orientačního čísla (52/1 najde 52/13)
        let results = search_adresa_impl(&pool, "Rýdlova 52/1".to_string()).await.unwrap();
        assert_contains_address(&results, "Rýdlova");
        assert_contains_address(&results, "52/13");
    }

/*     #[tokio::test]
    async fn test_orientation_prefix_matching_swapped() {
        let pool = get_test_pool().await;
        // Test doplňování orientačního čísla (13/5 najde 52/13)
        let results = search_adresa_impl(&pool, "Rýdlova 13/5".to_string()).await.unwrap();
        assert_contains_address(&results, "Rýdlova");
        assert_contains_address(&results, "52/13");
    } */

    #[tokio::test]
    async fn test_bad_numbers_return_empty() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "Mírová 999 888".to_string()).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_alphanumeric_orientation() {
        let pool = get_test_pool().await;
        // Test pro adresu s písmenem v orientačním čísle (10a)
        // Očekáváme, že i když 10a není čisté číslo, Fulltext ji najde v kombinaci s ulicí
        let results = search_adresa_impl(&pool, "Osadnická 572/10a".to_string()).await.unwrap();
        
        assert_contains_address(&results, "Osadnická");
        assert_contains_address(&results, "572");
        assert_contains_address(&results, "10a");
        assert_contains_address(&results, "Havířov");
    }

    #[tokio::test]
    async fn test_orientacni_znak() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "Osadnická 572 10a Šumbark Havířov 73601".to_string()).await.unwrap();
        
        assert_contains_address(&results, "Osadnická");
        assert_contains_address(&results, "572");
        assert_contains_address(&results, "10a");
        assert_contains_address(&results, "Havířov");
    }

    #[tokio::test]
    async fn test_short_city() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "Okružní 32 Aš".to_string()).await.unwrap();
        assert_contains_address(&results, "Okružní");
        assert_contains_address(&results, "Aš");
    }
    #[tokio::test]
    async fn test_mirova_1_ri_mandatory() {
        let pool = get_test_pool().await;
        let results = search_adresa_impl(&pool, "mirova 1 ri".to_string()).await.unwrap();
        assert_contains_address(&results, "říčany");
        assert_contains_address(&results, "1");
    }
}
