use leptos::prelude::*;
use leptos::task::spawn_local;
use core_db::Adresa;

#[cfg(feature = "ssr")]
use core_db::create_pool;

#[server]
pub async fn search_adresa(v: String) -> Result<Vec<Adresa>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use core_db::{create_pool, normalize, pad_token};

        let pool = create_pool().await.map_err(|e| ServerFnError::new(e.to_string()))?;

        let prepared_search = normalize(&v)
            .split_whitespace()
            .map(|t| pad_token(t))
            .map(|t| format!("+{}*", t))
            .collect::<Vec<_>>()
            .join(" ");

        if prepared_search.is_empty() {
            return Ok(Vec::new());
        }

        let results = sqlx::query_as::<_, Adresa>(
            "SELECT * FROM adresa WHERE MATCH(search) AGAINST(? IN BOOLEAN MODE) LIMIT 20"
        )
        .bind(prepared_search)
        .fetch_all(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

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
                        if v.is_empty() {
                            results.set(Vec::new());
                            return;
                        }

                        last_request_id.update(|id| *id += 1);
                        let request_id = last_request_id.get_untracked();

                        spawn_local(async move {
                            if let Ok(res) = search_adresa(v).await {
                                if last_request_id.get_untracked() == request_id {
                                    results.set(res);
                                }
                            }
                        });
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
