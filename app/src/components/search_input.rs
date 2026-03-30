use leptos::prelude::*;
use leptos::task::spawn_local;
use core_db::Adresa;
#[cfg(feature = "ssr")]
use core_db::{normalize};
#[cfg(feature = "ssr")]
use std::collections::HashSet;

#[cfg(feature = "ssr")]
struct ParserResult {
    text_tokens: Vec<String>,
    number_candidates: HashSet<i32>,
    psc_exact_candidates: HashSet<i32>,
    psc_prefix_candidates: HashSet<i32>,
    slash_pairs: HashSet<(String, String)>,
}

#[cfg(feature = "ssr")]
fn parse_input(raw_input: &str) -> Option<ParserResult> {
    let normalized = normalize(raw_input);
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let mut text_tokens = vec![];
    let mut num_tokens = vec![];

    for &token in &tokens {
        if token.chars().all(|c| c.is_ascii_digit()) {
            num_tokens.push(token);
        } else {
            text_tokens.push(token.to_string());
        }
    }

    // FTS je povinný
    if text_tokens.is_empty() {
        return None;
    }

    let mut number_candidates = HashSet::new();
    let mut psc_exact_candidates = HashSet::new();
    let mut psc_prefix_candidates = HashSet::new();
    let mut slash_pairs = HashSet::new();

    for n in num_tokens {
        if let Ok(value) = n.parse::<i32>() {
            match n.len() {
                1..=4 => {
                    number_candidates.insert(value);
                    if n.len() >= 2 {
                        psc_prefix_candidates.insert(value);
                    }
                }
                5 => {
                    psc_exact_candidates.insert(value);
                }
                _ => {}
            }
        }
    }

    // PSC z dvojice 3+2 tokenů, např. "251 01"
    for window in tokens.windows(2) {
        if window[0].len() == 3 && window[1].len() == 2 {
            if let (Ok(a), Ok(b)) = (window[0].parse::<i32>(), window[1].parse::<i32>()) {
                psc_exact_candidates.insert(a * 100 + b);
            }
        }
    }

    // Slash pair z původního inputu
    for raw_token in raw_input.split_whitespace() {
        if raw_token.contains('/') {
            let parts: Vec<&str> = raw_token.split('/').collect();
            if parts.len() == 2 {
                if let (Ok(a), Ok(b)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                    slash_pairs.insert((parts[0].to_string(), parts[1].to_string()));
                    number_candidates.insert(a);
                    number_candidates.insert(b);
                }
            }
        }
    }

    Some(ParserResult {
        text_tokens,
        number_candidates,
        psc_exact_candidates,
        psc_prefix_candidates,
        slash_pairs,
    })
}

#[cfg(feature = "ssr")]
fn build_fts_query(text_tokens: &[String]) -> String {
    let mut parts = vec![];
    for t in text_tokens {
        parts.push(format!("+{}*", t));
    }
    parts.join(" ")
}

#[cfg(feature = "ssr")]
fn push_branch(
    branches: &mut Vec<String>,
    binds: &mut Vec<String>,
    priority: i32,
    where_filter: &str,
    fts_query: &str,
    extra_binds: Vec<String>,
) {
    let sql = format!(
        "SELECT
        a.kod_adm, a.kod_obce, a.nazev_obce, a.kod_momc, a.nazev_momc,
        a.kod_obvodu_prahy, a.nazev_obvodu_prahy, a.kod_casti_obce, a.nazev_casti_obce,
        a.kod_ulice, a.nazev_ulice, a.typ_so, a.cislo_domovni, a.cislo_orientacni,
        a.znak_cisla_orientacniho, a.psc, a.souradnice_y, a.souradnice_x, a.plati_od,
        a.search,
        ? AS priority
      FROM adresa a
      WHERE MATCH(a.search) AGAINST(? IN BOOLEAN MODE)
        AND ({})
      LIMIT 20",
        where_filter
    );

    branches.push(sql);
    binds.push(priority.to_string());
    binds.push(fts_query.to_string());
    binds.extend(extra_binds);
}

/// Sestaví WHERE filtr pro P1 – přesné shody čísel popisných / orientačních.
///
/// Pokrývá 4 případy:
///  1. `cislo_domovni IN (...)` OR `cislo_orientacni IN (...)`
///  2. `domovni_orientacni_klic IN (...)`  – sestavený klíč "dom_ori"
///  3. `orientacni_domovni_klic IN (...)`  – swap klíče
///  4. Explicitní slash páry: (cislo_domovni=a AND cislo_orientacni=b) + swap
#[cfg(feature = "ssr")]
fn build_p1_filter(parsed: &ParserResult) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = vec![];
    let mut binds: Vec<String> = vec![];

    // 1) cislo_domovni IN (...) OR cislo_orientacni IN (...)
    // Pouze pokud je jediný kandidát – s více čísly by to matchovalo i neúplné záznamy
    if parsed.number_candidates.len() == 1 {
        let placeholders = parsed.number_candidates
            .iter()
            .map(|_| "?");
        let dom_ph = placeholders.clone().collect::<Vec<_>>().join(", ");
        let ori_ph = placeholders.collect::<Vec<_>>().join(", ");

        conditions.push(format!(
            "(a.cislo_domovni IN ({dom_ph}) OR a.cislo_orientacni IN ({ori_ph}))"
        ));
        for v in &parsed.number_candidates {
            binds.push(v.to_string());
        }
        for v in &parsed.number_candidates {
            binds.push(v.to_string());
        }
    }

    // 2) domovni_orientacni_klic IN (...) – klíč tvaru "12/15"
    if !parsed.number_candidates.is_empty() {
        let keys: Vec<String> = parsed.number_candidates
            .iter()
            .flat_map(|&a| parsed.number_candidates.iter().map(move |&b| format!("{a}/{b}")))
            .collect();
        if !keys.is_empty() {
            let ph = keys.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            conditions.push(format!("a.domovni_orientacni_klic IN ({ph})"));
            binds.extend(keys.clone());
        }
    }

    // 3) orientacni_domovni_klic IN (...) – swap klíče "25/1"
    if !parsed.number_candidates.is_empty() {
        let keys: Vec<String> = parsed.number_candidates
            .iter()
            .flat_map(|&a| parsed.number_candidates.iter().map(move |&b| format!("{b}/{a}")))
            .collect();
        if !keys.is_empty() {
            let ph = keys.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            conditions.push(format!("a.orientacni_domovni_klic IN ({ph})"));
            binds.extend(keys);
        }
    }

    // 4) Explicitní slash páry: (domovni=a AND orientacni=b) OR (domovni=b AND orientacni=a)
    for (left, right) in &parsed.slash_pairs {
        conditions.push(format!(
            "(a.cislo_domovni = ? AND a.cislo_orientacni = ?)"
        ));
        binds.push(left.clone());
        binds.push(right.clone());

        // swap
        conditions.push(format!(
            "(a.cislo_domovni = ? AND a.cislo_orientacni = ?)"
        ));
        binds.push(right.clone());
        binds.push(left.clone());
    }

    let where_clause = if conditions.is_empty() {
        "1=0".to_string() // žádné číslo ani slash pair => nic nevyhovuje
    } else {
        conditions.join(" OR ")
    };

    (where_clause, binds)
}

#[cfg(feature = "ssr")]
fn build_p2_filter(parsed: &ParserResult) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = vec![];
    let mut binds: Vec<String> = vec![];

    if !parsed.number_candidates.is_empty() {
        let ph = parsed.number_candidates
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");
        conditions.push(format!("a.ulice_cislo IN ({ph})"));
        binds.extend(parsed.number_candidates.iter().map(|&n| n.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        "1=0".to_string() // žádné číslo => nic nevyhovuje
    } else {
        conditions.join(" OR ")
    };

    (where_clause, binds)
}

#[cfg(feature = "ssr")]
fn build_p3_filter(parsed: &ParserResult) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = vec![];
    let mut binds: Vec<String> = vec![];

    // Z number_candidates vybereme jen hodnoty 1–22 (platné obvody Prahy)
    let obvod_candidates: Vec<i32> = parsed.number_candidates
        .iter()
        .copied()
        .filter(|&n| n >= 1 && n <= 22)
        .collect();

    if obvod_candidates.is_empty() {
        return ("1=0".to_string(), vec![]);
    }

    let ph = obvod_candidates.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    conditions.push(format!("a.obvod_prahy_cislo IN ({ph})"));
    binds.extend(obvod_candidates.iter().map(|n| n.to_string()));

    let where_clause = if conditions.is_empty() {
        "1=0".to_string() // žádné obvody => nic nevyhovuje
    } else {
        conditions.join(" OR ")
    };

    (where_clause, binds)
}

#[cfg(feature = "ssr")]
fn build_p4a_filter(parsed: &ParserResult) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = vec![];
    let mut binds: Vec<String> = vec![];

    if !parsed.psc_exact_candidates.is_empty() {
        let ph = parsed.psc_exact_candidates.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        conditions.push(format!("a.psc IN ({ph})"));
        binds.extend(parsed.psc_exact_candidates.iter().map(|&n| n.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        "1=0".to_string() // žádné PSC => nic nevyhovuje
    } else {
        conditions.join(" OR ")
    };

    (where_clause, binds)
}

#[cfg(feature = "ssr")]
fn build_p4b_filter(parsed: &ParserResult) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = vec![];
    let mut binds: Vec<String> = vec![];

    if !parsed.psc_prefix_candidates.is_empty() {
        for &n in &parsed.psc_prefix_candidates {
            let s = n.to_string();
            let missing = 5 - s.len();
            conditions.push("a.psc BETWEEN ? AND ?".to_string());
            binds.push(format!("{}{}", s, "0".repeat(missing)));
            binds.push(format!("{}{}", s, "9".repeat(missing)));
        }
    }

    let where_clause = if conditions.is_empty() {
        "1=0".to_string() // žádné PSC => nic nevyhovuje
    } else {
        conditions.join(" OR ")
    };

    (where_clause, binds)
}

#[cfg(feature = "ssr")]
fn build_priority_branches(parsed: &ParserResult, fts_query: &str) -> (Vec<String>, Vec<String>) {
    let mut branches: Vec<String> = vec![];
    let mut binds: Vec<String> = vec![];

    // P1: přesné číslo popisné / orientační (priorita 500)
    // Aktivuje se, pokud máme alespoň jedno číslo nebo slash pár.
    if !parsed.number_candidates.is_empty() || !parsed.slash_pairs.is_empty() {
        let (where_p1, binds_p1) = build_p1_filter(parsed);
        push_branch(&mut branches, &mut binds, 500, &where_p1, fts_query, binds_p1);
    }

    // P2: Číslo v názvu ulice (priorita 400)
    if !parsed.number_candidates.is_empty() {
        let (where_p2, binds_p2) = build_p2_filter(parsed);
        push_branch(&mut branches, &mut binds, 400, &where_p2, fts_query, binds_p2);
    }

    // P3: obvod Prahy, například „Praha 5" (priorita 300)
    // Aktivuje se jen pokud je token „praha" a číslo v rozsahu 1–22
    let has_praha = parsed.text_tokens.iter().any(|t| t == "praha");
    if has_praha && parsed.number_candidates.iter().any(|&n| n >= 1 && n <= 22) {
        let (where_p3, binds_p3) = build_p3_filter(parsed);
        push_branch(&mut branches, &mut binds, 300, &where_p3, fts_query, binds_p3);
    }

    // TODO P4a: PSC exact (priorita 220)
    if !parsed.psc_exact_candidates.is_empty() {
        let (where_p4a, binds_p4a) = build_p4a_filter(parsed);
        push_branch(&mut branches, &mut binds, 400, &where_p4a, fts_query, binds_p4a);
    }
    
    // TODO P4b: PSC prefix/range (priorita 210)
    if !parsed.psc_prefix_candidates.is_empty() {
        let (where_p4b, binds_p4b) = build_p4b_filter(parsed);
        push_branch(&mut branches, &mut binds, 400, &where_p4b, fts_query, binds_p4b);
    }
    // P5: fallback FTS – vždy přidat jako záchrannou větev (priorita 100)
    push_branch(&mut branches, &mut binds, 100, "1=1", fts_query, vec![]);

    (branches, binds)
}

#[cfg(feature = "ssr")]
fn build_final_sql(branches: &mut Vec<String>) -> String {
    let union_sql = branches.iter()
        .map(|b| format!("({})", b))
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let final_sql = format!("
      SELECT
        kod_adm, kod_obce, nazev_obce, kod_momc, nazev_momc,
        kod_obvodu_prahy, nazev_obvodu_prahy, kod_casti_obce, nazev_casti_obce,
        kod_ulice, nazev_ulice, typ_so, cislo_domovni, cislo_orientacni,
        znak_cisla_orientacniho, psc, souradnice_y, souradnice_x, plati_od, search
      FROM (
        SELECT
          ranked.*,
          ROW_NUMBER() OVER (
            PARTITION BY ranked.kod_adm
            ORDER BY ranked.priority DESC
          ) AS rn
        FROM (
          {union_sql}
        ) ranked
      ) dedup
      WHERE dedup.rn = 1
      ORDER BY dedup.priority DESC
      LIMIT 20
    ");
    return final_sql;
}

#[server]
pub async fn search_adresa(v: String) -> Result<Vec<Adresa>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos_actix::extract;
        use actix_web::web::Data;
        use sqlx::mysql::MySqlPool;

        let pool = extract::<Data<MySqlPool>>().await?.into_inner().clone();
        
        let parsed = parse_input(&v);

        if parsed.is_none() {
            return Ok(vec![]);
        }

        let parsed = parsed.unwrap();

        let fts_query = build_fts_query(&parsed.text_tokens);

        if fts_query.is_empty() {
            return Ok(vec![]);
        }

        let (mut branches, binds) = build_priority_branches(&parsed, &fts_query);
        let final_sql = build_final_sql(&mut branches);

        let mut q = sqlx::query_as::<_, Adresa>(&final_sql);
        for bind in &binds {
            q = q.bind(bind);
        }

        let results = q.fetch_all(&*pool).await?;

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