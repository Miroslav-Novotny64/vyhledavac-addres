use leptos::prelude::*;
use leptos::task::spawn_local;
use core_db::Adresa;
#[server]
pub async fn search_adresa(v: String) -> Result<Vec<Adresa>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos_actix::extract;
        use actix_web::web::Data;
        use core_db::{normalize, pad_token};
        use sqlx::mysql::MySqlPool;

        let pool = extract::<Data<MySqlPool>>().await?.into_inner().clone();

        let normalized = normalize(&v);
        let tokens: Vec<&str> = normalized.split_whitespace().collect();

        if tokens.is_empty() {
            return Ok(Vec::new());
        }

        let mut fts_parts = Vec::new();
        let mut short_numerics: Vec<&str> = Vec::new();
        for token in &tokens {
            if token.chars().all(|c| c.is_numeric()) {
                if token.len() >= 3 {
                    fts_parts.push(format!("+{}*", pad_token(token)));
                } else if short_numerics.len() < 2 {
                    short_numerics.push(token);
                }
            } else {
                fts_parts.push(format!("+{}*", token));
            }
        }

        let fts_query = fts_parts.join(" ");

        let results = if short_numerics.len() == 2 {
            let a: i32 = short_numerics[0].parse().unwrap_or(0);
            let b: i32 = short_numerics[1].parse().unwrap_or(0);
            let a_str = short_numerics[0];
            let b_str = short_numerics[1];

            let psc_range = |num: i32, s: &str| -> (String, String) {
                let padding = 5usize.saturating_sub(s.len());
                let mult = 10_i32.pow(padding as u32);
                let lo = num.saturating_mul(mult);
                let hi = num.saturating_add(1).saturating_mul(mult).saturating_sub(1);
                (format!("{:05}", lo), format!("{:05}", hi))
            };
            let (a_psc_lo, a_psc_hi) = psc_range(a, a_str);
            let (b_psc_lo, b_psc_hi) = psc_range(b, b_str);

            let query_str = r#"
                (
                    SELECT *, 1 as priority
                    FROM adresa
                    WHERE (MATCH(search) AGAINST(? IN BOOLEAN MODE) OR ? = '')
                      AND (
                          (cislo_domovni = ? AND cislo_orientacni = ?)
                          OR (cislo_domovni = ? AND cislo_orientacni = ?)
                      )
                    LIMIT 20
                )
                UNION ALL
                (
                    SELECT *, 2 as priority
                    FROM adresa
                    WHERE (MATCH(search) AGAINST(? IN BOOLEAN MODE) OR ? = '')
                      AND (
                          ((cislo_domovni = ? OR cislo_orientacni = ?) AND psc BETWEEN ? AND ?)
                          OR ((cislo_domovni = ? OR cislo_orientacni = ?) AND psc BETWEEN ? AND ?)
                      )
                      AND NOT (
                          (cislo_domovni = ? AND cislo_orientacni = ?)
                          OR (cislo_domovni = ? AND cislo_orientacni = ?)
                      )
                    LIMIT 20
                )
                ORDER BY priority ASC, cislo_orientacni IS NULL ASC
                LIMIT 20
            "#;

            sqlx::query_as::<_, Adresa>(query_str)
                .bind(&fts_query).bind(&fts_query)
                .bind(a).bind(b)
                .bind(b).bind(a)
                .bind(&fts_query).bind(&fts_query)
                .bind(a).bind(a).bind(&b_psc_lo).bind(&b_psc_hi)
                .bind(b).bind(b).bind(&a_psc_lo).bind(&a_psc_hi)
                .bind(a).bind(b)
                .bind(b).bind(a)
                .fetch_all(&*pool)
                .await?

        } else if let Some(num) = short_numerics.first().copied() {
            let num_val: i32 = num.parse().unwrap_or(0);

            let padding = 5usize.saturating_sub(num.len());
            let multiplier = 10_i32.pow(padding as u32);
            let psc_lower = num_val.saturating_mul(multiplier);
            let psc_upper = num_val.saturating_add(1).saturating_mul(multiplier).saturating_sub(1);
            let psc_lower_str = format!("{:05}", psc_lower);
            let psc_upper_str = format!("{:05}", psc_upper);

            let query_str = r#"
                (
                    SELECT *, 1 as priority
                    FROM adresa
                    WHERE (MATCH(search) AGAINST(? IN BOOLEAN MODE) OR ? = '')
                      AND (cislo_domovni = ? OR cislo_orientacni = ?)
                    LIMIT 20
                )
                UNION ALL
                (
                    SELECT *, 2 as priority
                    FROM adresa
                    WHERE (MATCH(search) AGAINST(? IN BOOLEAN MODE) OR ? = '')
                      AND psc BETWEEN ? AND ?
                      AND cislo_domovni != ? AND (cislo_orientacni IS NULL OR cislo_orientacni != ?)
                    LIMIT 20
                )
                ORDER BY priority ASC, cislo_orientacni IS NULL ASC
                LIMIT 20
            "#;

            sqlx::query_as::<_, Adresa>(query_str)
                .bind(&fts_query).bind(&fts_query)
                .bind(num_val).bind(num_val)
                .bind(&fts_query).bind(&fts_query)
                .bind(&psc_lower_str).bind(&psc_upper_str)
                .bind(num_val).bind(num_val)
                .fetch_all(&*pool)
                .await?

        } else {
            let query_str = "SELECT * FROM adresa WHERE MATCH(search) AGAINST(? IN BOOLEAN MODE) LIMIT 20";
            sqlx::query_as::<_, Adresa>(query_str)
                .bind(&fts_query)
                .fetch_all(&*pool)
                .await?
        };

        Ok(results)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = v;
        Err(ServerFnError::new("Server-side only"))
    }
}

#[component]
pub fn SearchInput(
    #[prop(into)] placeholder: String,
    on_select: Callback<Adresa>,
) -> impl IntoView {
    let value = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<Adresa>::new());
    let last_request_id = RwSignal::new(0u64);

    view! {
        <div class="search-container">
            <div class="search-input-wrapper">
                <input
                    type="text"
                    placeholder=placeholder
                    prop:value=value
                    on:input=move |ev| {
                        let v = event_target_value(&ev);
                        value.set(v.clone());
                        if v.len() < 3 {
                            results.set(Vec::new());
                            return;
                        }

                        last_request_id.update(|id| *id += 1);
                        let request_id = last_request_id.get_untracked();

                        set_timeout(move || {
                            if last_request_id.get_untracked() == request_id {
                                spawn_local(async move {
                                    if let Ok(res) = search_adresa(v).await {
                                        if last_request_id.get_untracked() == request_id {
                                            results.set(res);
                                        }
                                    }
                                });
                            }
                        }, std::time::Duration::from_millis(300));
                    }
                />
            </div>
            <Show
                when=move || !results.get().is_empty()
            >
                <ul class="search-results">
                    <For
                        each=move || results.get()
                        key=|res| res.kod_adm
                        let:res
                    >
                        {
                            let street = res.nazev_ulice.clone()
                                .or_else(|| res.nazev_casti_obce.clone())
                                .unwrap_or_else(|| res.nazev_obce.clone());

                            let numbers = format!("{}{}", 
                                res.cislo_domovni, 
                                res.cislo_orientacni.map(|o| format!("/{}", o)).unwrap_or_default()
                            );

                            let city_part = res.nazev_momc.clone()
                                .or_else(|| res.nazev_casti_obce.clone())
                                .filter(|name| name != &res.nazev_obce);

                            let location_no_street = match city_part {
                                Some(part) => format!("{}, {}", part, res.nazev_obce),
                                None => res.nazev_obce.clone(),
                            };

                            let full_address = format!("{} {} {} {}", 
                                street, 
                                numbers, 
                                location_no_street, 
                                res.psc
                            ).replace("  ", " ").trim().to_string();

                            let res_clone = res.clone();
                            view! {
                                <li 
                                    class="address-item"
                                    on:click=move |_| {
                                        on_select.run(res_clone.clone());
                                        results.set(Vec::new());
                                        value.set(String::new());
                                    }
                                >
                                    <span class="full-address">{full_address}</span>
                                </li>
                            }
                        }
                    </For>
                </ul>
            </Show>
        </div>
    }
}
