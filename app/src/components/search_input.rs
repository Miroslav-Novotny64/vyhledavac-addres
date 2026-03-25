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
        let mut short_numeric = None;
        for token in &tokens {
            if token.chars().all(|c| c.is_numeric()) {
                if token.len() >= 3 {
                    fts_parts.push(format!("+{}", pad_token(token)));
                } else if short_numeric.is_none() {
                    short_numeric = Some(*token);
                }
            } else {
                fts_parts.push(format!("+{}*", token));
            }
        }

        let fts_query = fts_parts.join(" ");

        let results = if let Some(num) = short_numeric {
            let num_val: i32 = num.parse().unwrap_or(0);
            let num_prefix = format!("{}%", num);

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
                      AND (cislo_domovni LIKE ? OR cislo_orientacni LIKE ? OR psc LIKE ?)
                      AND cislo_domovni != ? AND (cislo_orientacni IS NULL OR cislo_orientacni != ?)
                    LIMIT 20
                )
                ORDER BY priority ASC, cislo_orientacni IS NULL ASC
                LIMIT 20
            "#;

            sqlx::query_as::<_, Adresa>(query_str)
                .bind(&fts_query)
                .bind(&fts_query)
                .bind(num_val)
                .bind(num_val)
                .bind(&fts_query)
                .bind(&fts_query)
                .bind(&num_prefix)
                .bind(&num_prefix)
                .bind(&num_prefix)
                .bind(num_val)
                .bind(num_val)
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