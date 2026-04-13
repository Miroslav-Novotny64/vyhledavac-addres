use core_db::Adresa;
#[cfg(feature = "ssr")]
use core_db::{normalize, pad_token};
use leptos::prelude::*;
use leptos::task::spawn_local;
#[cfg(feature = "ssr")]
use sqlx;

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
                        let input = event_target_value(&ev);
                        value.set(input.clone());
                        if input.len() < 3 {
                            results.set(Vec::new());
                            return;
                        }

                        last_request_id.update(|id| *id += 1);
                        let request_id = last_request_id.get_untracked();

                        set_timeout(move || {
                            if last_request_id.get_untracked() == request_id {
                                spawn_local(async move {
                                    if let Ok(res) = search_adresa(input).await {
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

                            let typ_so = if res.typ_so == "č.ev." { "č.ev. " } else { "" };

                            let mut numbers = format!("{}{}", typ_so, res.cislo_domovni);
                            if let Some(o) = res.cislo_orientacni {
                                let znak = res.znak_cisla_orientacniho.clone().unwrap_or_default();
                                numbers.push_str(&format!("/{o}{znak}"));
                            }

                            let mut location_parts = Vec::new();

                            let part = res.nazev_momc.clone()
                                .or_else(|| res.nazev_casti_obce.clone());

                            if let Some(ref p) = part {
                                if p != &street && p != &res.nazev_obce {
                                    location_parts.push(p.clone());
                                }
                            }

                            if let Some(obvod) = res.nazev_obvodu_prahy.clone() {
                                if obvod != res.nazev_obce && !location_parts.contains(&obvod) {
                                    location_parts.push(obvod);
                                }
                            }

                            if street != res.nazev_obce {
                                let obec = &res.nazev_obce;
                                // Přidáme obec jen pokud tam už není zmíněna (např. v "Praha 4" už "Praha" je)
                                let obec_already_in_parts = location_parts.iter().any(|p| p.starts_with(obec));
                                if !obec_already_in_parts {
                                    location_parts.push(obec.clone());
                                }
                            }

                            let location_str = location_parts.join(", ");

                            let psc_str = res.psc.to_string();
                            let formatted_psc = if psc_str.len() == 5 {
                                format!("{} {}", &psc_str[0..3], &psc_str[3..5])
                            } else {
                                psc_str
                            };

                            let full_address = if location_str.is_empty() {
                                format!("{} {}, {}", street, numbers, formatted_psc)
                            } else {
                                format!("{} {}, {} {}", street, numbers, location_str, formatted_psc)
                            };

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

#[server]
pub async fn search_adresa(input: String) -> Result<Vec<Adresa>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use actix_web::web::Data;
        use leptos_actix::extract;
        use sqlx::mysql::MySqlPool;

        let pool = extract::<Data<MySqlPool>>()
            .await
            .map_err(|e| ServerFnError::new(format!("Failed to extract pool: {e}")))?
            .into_inner()
            .clone();

        search_adresa_impl(&pool, input)
            .await
            .map_err(|e| ServerFnError::new(format!("Query error: {e}")))
    }

    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new("Server-side only"))
    }
}

#[cfg(feature = "ssr")]
pub async fn search_adresa_impl(
    pool: &sqlx::MySqlPool,
    input: String,
) -> Result<Vec<Adresa>, sqlx::Error> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    let mut norm_tokens = Vec::new();
    let mut num_tokens = Vec::new();

    for t in tokens {
        let norm = normalize(t);
        if norm.is_empty() {
            continue;
        }

        if let Ok(num) = norm.parse::<i32>() {
            num_tokens.push(num);
        }
        norm_tokens.push(norm);
    }

    if norm_tokens.is_empty() {
        return Ok(Vec::new());
    }

    let padded_tokens: Vec<String> = norm_tokens.iter().map(|t| pad_token(t)).collect();
    let fts_query = padded_tokens
        .iter()
        .map(|t| format!("+{}*", t))
        .collect::<Vec<String>>()
        .join(" ");

    let mut query_builder = sqlx::QueryBuilder::new(
        "SELECT * FROM adresa WHERE MATCH(search) AGAINST(",
    );
    query_builder.push_bind(&fts_query);
    query_builder.push(" IN BOOLEAN MODE)");

    if !num_tokens.is_empty() {
        query_builder.push(" ORDER BY (");
        for (i, &num) in num_tokens.iter().enumerate() {
            if i > 0 {
                query_builder.push(" OR ");
            }
            query_builder.push("cislo_domovni = ");
            query_builder.push_bind(num);
        }
        query_builder.push(") DESC, (");
        for (i, &num) in num_tokens.iter().enumerate() {
            if i > 0 {
                query_builder.push(" OR ");
            }
            query_builder.push("cislo_orientacni = ");
            query_builder.push_bind(num);
        }
        query_builder.push(") DESC");
    }

    query_builder.push(" LIMIT 20");

    let query = query_builder.build_query_as::<Adresa>();
    query.fetch_all(pool).await
}
