# Krok za Krokem: Implementace Striktního Vyhledávání

Původní dokument byl možná moc abstraktní. Pojďme si to tedy rozdělit na 5 malých, dobře stravitelných kroků. Výsledkem bude funkce, která vezme zadaná čísla a vygeneruje z nich všechny validní scénáře (např. toto číslo bude dům, tohle zase PSČ), oboduje je a vyrobí z nich SQL.

---

## Krok 1: Dvě základní datové struktury

Přidej si tyto dvě struktury někam nahoru nad funkci `search_adresa`.

První struktura drží jednu rozpracovanou „kombinaci“ s tím, co se kam zařadilo. 
Druhá struktura je výstupní výsledek – obsahuje hotové sql `WHERE` výrazy a jejich nabindované hodnoty.

```rust
use std::collections::HashSet;

#[derive(Clone, Default, Eq, PartialEq, Hash)]
struct Assignment {
    ulice: Option<i32>,
    dom: Option<i32>,
    ori: Option<i32>,
    psc: Option<i32>,
    psc_is_exact: bool,
    obvod: Option<i32>,
}

struct GeneratedCase {
    where_sql: String,
    binds: Vec<String>,
    score: i32,
}
```

---

## Krok 2: Samotná "backtrack" funkce

Toto je ta funkce s "velkou strašidelnou smyčkou". Její úkol je vzít *seznam čísel*, vzít první z nich a zkusit ho strčit do každé z 5 krabiček v `Assignment`. Poté zavolá sama sebe, aby zkusila strčit i druhé číslo. Když pole dojdou, uloží platnou kombinaci do výsledků a skončí.

Přidej tuhle funkci (i s kódovými bloky) nad svůj kód:

```rust
fn backtrack(
    nums: &[i32],                  // Zbytek 1-4ciferných čísel
    pscs: &[i32],                  // Zbytek 5ciferných PSČ
    current: &mut Assignment,      // Krabička s tím, co jsme už zaplnili
    has_praha: bool,               // Jestli padlo slovo Praha
    results: &mut HashSet<Assignment> // Set se všemi úspěšnými krabičkami
) {
    // A) Nejprve zkusíme umístit 5ciferné PSČ, pokud ještě nějaké v poli zbylo
    if let Some(&p) = pscs.first() {
        if current.psc.is_none() {
            current.psc = Some(p);
            current.psc_is_exact = true;
            // Zavoláme sami sebe znovu, ale "ukrojíme" první prvek pole (`&pscs[1..]`)
            backtrack(nums, &pscs[1..], current, has_praha, results);
            current.psc = None; // Uklidíme po sobě 
            current.psc_is_exact = false;
        }
        return; // 5ciferné číslo MUSÍ být v PSČ, jiné pokusy ani neděláme
    }
    
    // B) Nyní zpracujeme běžná čísla (1-4 cifry), pokud nějaká zbyla
    if let Some(&n) = nums.first() {
        let zbytek_cisel = &nums[1..];
        
        // Zkoušíme "n" nacpat do jednotlivých volných šuplíků v current
        
        if current.ulice.is_none() {
            current.ulice = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results);
            current.ulice = None;
        }
        if current.dom.is_none() {
            current.dom = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results);
            current.dom = None;
        }
        if current.ori.is_none() {
            current.ori = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results);
            current.ori = None;
        }
        if current.psc.is_none() {
            current.psc = Some(n);
            current.psc_is_exact = false;
            backtrack(zbytek_cisel, pscs, current, has_praha, results);
            current.psc = None;
        }
        if current.obvod.is_none() && has_praha && n >= 1 && n <= 22 {
            current.obvod = Some(n);
            backtrack(zbytek_cisel, pscs, current, has_praha, results);
            current.obvod = None;
        }
        
        return; // Zkusili jsme všechno
    }
    
    // C) Když pole dospěla do konce (našli jsme validní kombinaci, co nikde nezhavarovala)
    let mut c = current.clone();
    
    // Drobné uhlazení, pokud máme jen číslo orientační a nikoliv domovní, přesuneme si 
    // ho do domovní pro lepší čitelnost.
    if c.dom.is_none() && c.ori.is_some() {
        c.dom = c.ori;
        c.ori = None;
    }
    
    results.insert(c);
}
```

(Jak vidíš, tato funkce sama od sebe nevygeneruje žádný výsledek, pokud nějaké číslo do ničeho volného nepasuje – vyhledávání pak v tichosti nenajde vůbec nic, přesně jak si to přeješ u překlepů).

---

## Krok 3: Uděláme z kombinací SQL a body

Nyní potřebujeme obalovací funkci, která vezme tvůj `HashSet<Assignment>` a z jejich vlastností (co je kde poskládáno) vyrobí SQL řetězce (ve tvaru `a.ulice_cislo = ?`) a spočítá, kolik si zaslouží ta kombinace bodů. 

```rust
fn generate_assignments(nums: &[i32], pscs: &[i32], has_praha: bool) -> Vec<GeneratedCase> {
    let mut results = HashSet::new();
    let mut initial = Assignment::default();
    
    // Získáme všechny variace do variables `results` z kroku 2
    backtrack(nums, pscs, &mut initial, has_praha, &mut results);
    
    let mut cases = Vec::new();
    for ass in results {
        let mut conds = vec![];
        let mut binds = vec![];
        let mut score = 0;
        
        if let Some(u) = ass.ulice {
            conds.push("a.ulice_cislo = ?".to_string());
            binds.push(u.to_string());
            score += 300;
        }
        
        match (ass.dom, ass.ori) {
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
        
        if let Some(p) = ass.psc {
            if ass.psc_is_exact {
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
        
        if let Some(ob) = ass.obvod {
            conds.push("a.obvod_prahy_cislo = ?".to_string());
            binds.push(ob.to_string());
            score += 250;
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
```

---

## Krok 4: Integrace do `search_adresa`

Toto je místo, kde jsi zkoušel psát něco Ty! Do funkce `search_adresa` stačí zavolat ten náš slavný `generate_assignments`.
Všechny staré větve vyjmeme, projdeme Cyklus přes naše krabičky `cases` a pro každou zavoláme **starý známý `push_branch`** 

**Načti to pomocí tohodle bloku do search_adresa (vymaž `build_priority_branches` a nahraď to tímto):**

```rust
        let has_praha = parsed.text_tokens.iter().any(|t| t == "praha");
        let num_vec: Vec<i32> = parsed.number_candidates.into_iter().collect();
        let psc_vec: Vec<i32> = parsed.psc_exact_candidates.into_iter().collect();

        // 1. Vygenerujeme list validních GeneratedCase!
        let cases = generate_assignments(&num_vec, &psc_vec, has_praha);

        let mut branches = vec![];
        let mut binds = vec![];

        // 2. Každou validní shodu přidáme jako branch přes starý UNION ALL postup,
        //   protože UNION ALL nám ošetřuje dedup přes limit tak jak jsme byli zvyklí
        for case in cases {
            push_branch(&mut branches, &mut binds, case.score, &case.where_sql, &fts_query, case.binds);
        }

        // 3. FALLBACK - zapneme jedině tehdy, když do pole vůbec nevkročilo reálné číslo na vstupu.
        // Tedy když je seznam uživatelských čísel prázdný, hledáme klasicky volně celou databázku (s prioritou 100)
        if num_vec.is_empty() && psc_vec.is_empty() {
            push_branch(&mut branches, &mut binds, 100, "1=1", &fts_query, vec![]);
        }

        // 4. Pokud to sem dorazilo a branches je prázdné (uživatel psal samá blbá čísla, 
        //   nebo čísla navíc = vyhledávač musel hodit chybu), neděláme drahé selecty a vrátíme Ok.
        if branches.is_empty() {
             return Ok(vec![]);
        }
```

To je vše! Můžeš to s klidným srdcem přepsat odshora dolů a uvidíš, že je to naprosto triviální! Každá funkce dělá malou blbůstku – jedna skládá do šuplíčků (Krok 2), druhá počítá skóre (Krok 3) a třetí to dává dohromady při spojení s FTS (Krok 4). Můžeš smazat komplet všechny staré složité funkce (p1 filter, p2 filter atd).
