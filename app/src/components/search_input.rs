#[cfg(feature = "ssr")]
use core_db::normalize;
use core_db::Adresa;
use leptos::prelude::*;
use leptos::task::spawn_local;
#[cfg(feature = "ssr")]
use std::collections::HashSet;

#[cfg(feature = "ssr")]
struct ParserResult {
    text_tokens: Vec<String>,
    number_candidates: Vec<i32>,
    psc_exact_candidates: Vec<i32>,
    psc_parts: Vec<(i32, i32)>,
    slash_pairs: Vec<String>,
}

#[cfg(feature = "ssr")]
#[derive(Clone, Default, Eq, PartialEq, Hash)]
struct Assignment {
    ulice: Option<i32>,
    dom: Option<i32>,
    ori: Option<i32>,
    psc: Option<i32>,
    psc_is_exact: bool,
    obvod: Option<i32>,
}

#[cfg(feature = "ssr")]
struct GeneratedCase {
    where_sql: String,
    binds: Vec<String>,
    score: i32,
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
    let mut slash_pairs = vec![];

    for &token in &tokens {
        if token.chars().all(|c| c.is_ascii_digit()) {
            num_tokens.push(token);
        } else if token.contains('_') {
            let parts: Vec<&str> = token.split('_').collect();
            if parts.len() == 2 && parts[0].chars().all(|c| c.is_ascii_digit()) && parts[1].chars().all(|c| c.is_ascii_digit()) {
                slash_pairs.push(token.to_string());
            } else {
                text_tokens.push(token.to_string());
            }
        } else {
            text_tokens.push(token.to_string());
        }
    }

    // FTS je povinný jen pokud nemáme jiný silný selektor
    if text_tokens.is_empty() && slash_pairs.is_empty() {
        return None;
    }

    let mut number_candidates = Vec::new();
    let mut psc_exact_candidates = Vec::new();
    let mut psc_parts = Vec::new();

    for n in num_tokens {
        if let Ok(value) = n.parse::<i32>() {
            match n.len() {
                1..=4 => {
                    number_candidates.push(value);
                }
                5 => {
                    psc_exact_candidates.push(value);
                }
                _ => {}
            }
        }
    }

    // PSC z dvojice 3+2 tokenů, např. "251 01"
    for window in tokens.windows(2) {
        if window[0].len() == 3 && window[1].len() == 2 {
            if let (Ok(a), Ok(b)) = (window[0].parse::<i32>(), window[1].parse::<i32>()) {
                psc_exact_candidates.push(a * 100 + b);
                psc_parts.push((a, b));
            }
        }
    }

    Some(ParserResult {
        text_tokens,
        number_candidates,
        psc_exact_candidates,
        psc_parts,
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
    let match_clause = if fts_query.trim().is_empty() {
        "1=1".to_string()
    } else {
        "MATCH(a.search) AGAINST(? IN BOOLEAN MODE)".to_string()
    };

    let sql = format!(
        "SELECT
        a.kod_adm, a.kod_obce, a.nazev_obce, a.kod_momc, a.nazev_momc,
        a.kod_obvodu_prahy, a.nazev_obvodu_prahy, a.kod_casti_obce, a.nazev_casti_obce,
        a.kod_ulice, a.nazev_ulice, a.typ_so, a.cislo_domovni, a.cislo_orientacni,
        a.znak_cisla_orientacniho, a.psc, a.souradnice_y, a.souradnice_x, a.plati_od,
        a.search,
        ? AS priority
      FROM adresa a
      WHERE {}
        AND ({})
      LIMIT 20",
        match_clause, where_filter
    );

    branches.push(sql);
    binds.push(priority.to_string());
    if !fts_query.trim().is_empty() {
        binds.push(fts_query.to_string());
    }
    binds.extend(extra_binds);
}

#[cfg(feature = "ssr")]
fn build_final_sql(branches: &mut Vec<String>) -> String {
    let union_sql = branches
        .iter()
        .map(|b| format!("({})", b))
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let final_sql = format!(
        "
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
    "
    );
    return final_sql;
}

#[cfg(feature = "ssr")]
fn backtrack(
    nums: &[i32],                      // Zbytek 1-4ciferných čísel
    pscs: &[i32],                      // Zbytek 5ciferných PSČ
    current: &mut Assignment,          // co jsme už použili
    has_praha: bool,                   // Jestli je v textu "praha"
    results: &mut HashSet<Assignment>, // Set se všemi úspěšnými results
    psc_parts: &[(i32, i32)],
) {
    // A) Umístíme přesné psč.
    if let Some(&p) = pscs.first() {
        let a_part = p / 100;
        let b_part = p % 100;
        let is_split_psc = psc_parts.contains(&(a_part, b_part));

        if current.psc.is_none() {
            current.psc = Some(p);
            current.psc_is_exact = true;
            
            let mut sub_nums = Vec::new();

            if is_split_psc {
                let mut a_removed = false;
                let mut b_removed = false;
                for &n in nums {
                    if !a_removed && n == a_part {
                        a_removed = true;
                    } else if !b_removed && n == b_part {
                        b_removed = true;
                    } else {
                        sub_nums.push(n);
                    }
                }
            } else {
                sub_nums = nums.to_vec();
            }

            backtrack(&sub_nums, &pscs[1..], current, has_praha, results, psc_parts);
            
            current.psc = None;
            current.psc_is_exact = false;
        }

        if is_split_psc {
            backtrack(nums, &pscs[1..], current, has_praha, results, psc_parts);
        }

        return;
    }

    if let Some(&n) = nums.first() {
        let zbytek_cisel = &nums[1..];

        if current.ulice.is_none() {
            current.ulice = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results, psc_parts);
            current.ulice = None;
        }
        if current.dom.is_none() {
            current.dom = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results, psc_parts);
            current.dom = None;
        }
        if current.ori.is_none() {
            current.ori = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results, psc_parts);
            current.ori = None;
        }
        if current.psc.is_none() {
            current.psc = Some(n);
            current.psc_is_exact = false;
            backtrack(zbytek_cisel, pscs, current, has_praha, results, psc_parts);
            current.psc = None;
        }
        if current.obvod.is_none() && has_praha && n >= 1 && n <= 22 {
            current.obvod = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results, psc_parts);
            current.obvod = None;
        }

        return;
    }
    let mut c = current.clone();

    if c.dom.is_none() && c.ori.is_some() {
        c.dom = c.ori;
        c.ori = None;
    }

    results.insert(c);
}

#[cfg(feature = "ssr")]
fn generate_assignments(
    nums: &[i32],
    pscs: &[i32],
    has_praha: bool,
    psc_parts: &[(i32, i32)],
    slash_pairs: &[String],
) -> Vec<GeneratedCase> {
    let mut results = HashSet::new();
    let mut initial = Assignment::default();

    backtrack(nums, pscs, &mut initial, has_praha, &mut results, psc_parts);

    let mut cases = Vec::new();
    for addr in results {
        let mut conds = vec![];
        let mut binds = vec![];
        let mut score = 0;

        if let Some(u) = addr.ulice {
            conds.push("a.ulice_cislo = ?".to_string());
            binds.push(u.to_string());
            score += 300;
        }
        match (addr.dom, addr.ori) {
            (Some(d), Some(o)) => {
                conds.push("a.cislo_domovni = ?".to_string());
                binds.push(d.to_string());
                conds.push("a.cislo_orientacni = ?".to_string());
                binds.push(o.to_string());
                score += 900;
            }
            (Some(d), None) => {
                conds.push("(a.cislo_domovni = ? OR a.cislo_orientacni = ?)".to_string());
                binds.push(d.to_string());
                binds.push(d.to_string());
                score += 400;
            }
            _ => {}
        }

        if let Some(p) = addr.psc {
            if addr.psc_is_exact {
                conds.push("a.psc = ?".to_string());
                binds.push(p.to_string());
            } else {
                let s = p.to_string();
                let missing = 5 - s.len();
                conds.push("a.psc BETWEEN ? AND ?".to_string());
                binds.push(format!("{}{}", s, "0".repeat(missing)));
                binds.push(format!("{}{}", s, "9".repeat(missing)));
            }
            score += 200;
        }

        if let Some(ob) = addr.obvod {
            conds.push("a.obvod_prahy_cislo = ?".to_string());
            binds.push(ob.to_string());
            score += 250;
        }

        for sp in slash_pairs {
            let swapped = if let Some((a, b)) = sp.split_once('_') {
                format!("{}_{}", b, a)
            } else {
                sp.to_string()
            };
            conds.push("(a.domovni_orientacni_klic = ? OR a.domovni_orientacni_klic = ?)".to_string());
            binds.push(sp.to_string());
            binds.push(swapped);
            score += 900;
        }

        if !conds.is_empty() {
            cases.push(GeneratedCase {
                where_sql: conds.join(" AND "),
                binds,
                score,
            });
        }
    }
    cases
}

#[server]
pub async fn search_adresa(v: String) -> Result<Vec<Adresa>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use actix_web::web::Data;
        use leptos_actix::extract;
        use sqlx::mysql::MySqlPool;

        let pool = extract::<Data<MySqlPool>>().await?.into_inner().clone();

        let parsed = parse_input(&v);

        if parsed.is_none() {
            return Ok(vec![]);
        }

        let parsed = parsed.unwrap();

        let fts_query = build_fts_query(&parsed.text_tokens);

        if fts_query.is_empty() && parsed.slash_pairs.is_empty() {
            return Ok(vec![]);
        }

        let has_praha = parsed.text_tokens.iter().any(|t| t == "praha");
        let num_vec: Vec<i32> = parsed.number_candidates.clone();
        let psc_vec: Vec<i32> = parsed.psc_exact_candidates.clone();
        let psc_parts = parsed.psc_parts;
        let slash_pairs = parsed.slash_pairs;

        // 1. Vygenerujeme list validních GeneratedCase!
        let cases = generate_assignments(&num_vec, &psc_vec, has_praha, &psc_parts, &slash_pairs);

        // TODO: Zde implementuj finální smyčku, která nahradí pevné větve dynamickými.
        // Volání build_priority_branches je pryč.
        let mut branches = vec![];
        let mut binds = vec![];

        for case in cases {
            push_branch(
                &mut branches,
                &mut binds,
                case.score,
                &case.where_sql,
                &fts_query,
                case.binds,
            );
        }

        if num_vec.is_empty() && psc_vec.is_empty() && slash_pairs.is_empty() {
            push_branch(&mut branches, &mut binds, 100, "1=1", &fts_query, vec![]);
        }

        if branches.is_empty() {
            return Ok(vec![]);
        }

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
