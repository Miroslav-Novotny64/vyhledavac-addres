# Prioritni search - pseudokod implementace

Tento dokument je navrh pseudokodu pro implementaci `search_adresa`.

## 1) Cile

- fulltext pres slova (FTS) je povinny v kazde vetvi
- presne cisla maji vyssi prioritu nez obecny match
- pouzit `UNION ALL`, lokalni `LIMIT 20` na kazdou vetev
- deduplikovat vysledky podle `kod_adm`
- finalne radit podle `priority DESC`, `fts_score DESC`, `LIMIT 20`

## 2) Parser vstupu

```text
function parse_input(raw_input):
    normalized = normalize(raw_input)
    tokens = split_whitespace(normalized)

    if tokens empty:
        return empty_parser_result

    text_tokens = []
    numeric_tokens = []

    for token in tokens:
        if token is all digits:
            numeric_tokens.push(token)
        else:
            text_tokens.push(token)

    # FTS je povinny => bez text tokenu nema smysl hledat
    if text_tokens empty:
        return empty_parser_result

    number_candidates = set<int>()          # 1..4 cifry
    psc_exact_candidates = set<int>()       # presne 5 cifer
    psc_prefix_candidates = set<int>()      # 2..4 cifry
    slash_pairs = set<(int,int)>()          # a/b nebo b/a

    for n in numeric_tokens:
        value = parse_int(n)
        if length(n) in [1..4]:
            number_candidates.add(value)
        if length(n) == 5:
            psc_exact_candidates.add(value)
        if length(n) in [2..4]:
            psc_prefix_candidates.add(value)

    # PSC z dvojice 3+2 tokenu, napr. "251 01"
    for each adjacent pair (a, b) in tokens:
        if a is 3 digits and b is 2 digits:
            psc_exact_candidates.add(parse_int(a + b))

    # slash pair z puvodniho inputu, aby se zachovalo '/' 
    for raw_token in split_whitespace(raw_input):
        cleaned = keep_digits_and_slash(raw_token)
        if cleaned matches "left/right" and both sides are 1..4 digits:
            a = parse_int(left)
            b = parse_int(right)
            slash_pairs.add((a,b))
            number_candidates.add(a)
            number_candidates.add(b)

    return {
        text_tokens,
        number_candidates,
        psc_exact_candidates,
        psc_prefix_candidates,
        slash_pairs
    }
```

## 3) Skladani FTS dotazu

```text
function build_fts_query(text_tokens):
    # vsechny text tokeny povinne (+), prefixove (*)
    parts = []
    for t in text_tokens:
        parts.push("+" + pad_token(t) + "*")

    fts_query = join(parts, " ")
    if fts_query empty:
        return none

    return fts_query
```

## 4) SQL branch builder (UNION ALL)

```text
function push_branch(branches, binds, priority, where_filter, extra_binds):
    sql = """
      SELECT
        a.kod_adm, a.kod_obce, a.nazev_obce, a.kod_momc, a.nazev_momc,
        a.kod_obvodu_prahy, a.nazev_obvodu_prahy, a.kod_casti_obce, a.nazev_casti_obce,
        a.kod_ulice, a.nazev_ulice, a.typ_so, a.cislo_domovni, a.cislo_orientacni,
        a.znak_cisla_orientacniho, a.psc, a.souradnice_y, a.souradnice_x, a.plati_od,
        a.search,
        {priority} AS priority,
        MATCH(a.search) AGAINST(? IN BOOLEAN MODE) AS fts_score
      FROM adresa a
      WHERE MATCH(a.search) AGAINST(? IN BOOLEAN MODE)
        AND ({where_filter})
      LIMIT 20
    """

    branches.push(sql)
    binds.push(fts_query)   # pro score
    binds.push(fts_query)   # pro where
    binds.extend(extra_binds)
```

## 5) Priority vetve

```text
function build_priority_branches(parsed, fts_query):
    branches = []
    binds = []

    # P1: presne domovni/orientacni
    # - cislo_domovni IN (...) OR cislo_orientacni IN (...)
    # - domovni_orientacni_klic IN (...)
    # - orientacni_domovni_klic IN (...)
    # - explicitni dvojice (domovni=?, orientacni=?) + swap
    if parsed.number_candidates not empty OR parsed.slash_pairs not empty:
        where_p1, binds_p1 = build_p1_filter(parsed)
        push_branch(branches, binds, 500, where_p1, binds_p1)

    # P2: cislo v nazvu ulice
    if parsed.number_candidates not empty:
        where_p2 = "a.ulice_cislo IN ( ... )"
        binds_p2 = values(parsed.number_candidates)
        push_branch(branches, binds, 400, where_p2, binds_p2)

    # P3: obvod Prahy
    has_praha = "praha" in parsed.text_tokens
    obvod_candidates = filter_1_to_22(parsed.number_candidates)
    if has_praha AND obvod_candidates not empty:
        where_p3 = "a.obvod_prahy_cislo IN ( ... )"
        binds_p3 = values(obvod_candidates)
        push_branch(branches, binds, 300, where_p3, binds_p3)

    # P4a: PSC exact (5 cifer)
    if parsed.psc_exact_candidates not empty:
        where_p4_exact = "a.psc IN ( ... )"
        binds_p4_exact = values(parsed.psc_exact_candidates)
        push_branch(branches, binds, 220, where_p4_exact, binds_p4_exact)

    # P4b: PSC prefix/range (2-4 cifry)
    if parsed.psc_prefix_candidates not empty:
        # 25 -> 25000..25999
        # 251 -> 25100..25199
        # 2510 -> 25100..25109
        where_p4_prefix, binds_p4_prefix = build_psc_range_filter(parsed.psc_prefix_candidates)
        push_branch(branches, binds, 210, where_p4_prefix, binds_p4_prefix)

    # P5: fallback FTS
    push_branch(branches, binds, 100, "1=1", [])

    return branches, binds
```

---

## 5a) build_p1_filter — přesné číslo popisné / orientační (priorita 500)

```text
function build_p1_filter(parsed):
    conditions = []
    binds = []

    # 1) přímý match cislo_domovni nebo cislo_orientacni
    if number_candidates not empty:
        dom_ph = join(["?"] * count, ", ")
        ori_ph = join(["?"] * count, ", ")
        conditions.push("(a.cislo_domovni IN ({dom_ph}) OR a.cislo_orientacni IN ({ori_ph}))")
        binds.extend(number_candidates)   # pro domovni
        binds.extend(number_candidates)   # pro orientacni

    # 2) složený klíč domovni_orientacni_klic = "<dom>_<ori>"
    #    cross-product všech number_candidates
    if number_candidates not empty:
        keys = [f"{a}_{b}" for a in number_candidates for b in number_candidates]
        conditions.push("a.domovni_orientacni_klic IN ({ph})")
        binds.extend(keys)

    # 3) swap klíče orientacni_domovni_klic = "<ori>_<dom>"
    if number_candidates not empty:
        keys = [f"{b}_{a}" for a in number_candidates for b in number_candidates]
        conditions.push("a.orientacni_domovni_klic IN ({ph})")
        binds.extend(keys)

    # 4) explicitní slash páry (domovni=left AND orientacni=right) + swap
    for (left, right) in slash_pairs:
        conditions.push("(a.cislo_domovni = ? AND a.cislo_orientacni = ?)")
        binds.extend([left, right])
        conditions.push("(a.cislo_domovni = ? AND a.cislo_orientacni = ?)")
        binds.extend([right, left])

    where_clause = join(conditions, " OR ")
    return where_clause, binds
```

**Příklad vstupu:** `"Dlouhá 12/5 Praha"`
→ `number_candidates = {12, 5}`, `slash_pairs = {(12, 5)}`

```sql
SELECT ..., 500 AS priority, MATCH(a.search) AGAINST('+Dlouha* +Praha*' IN BOOLEAN MODE) AS fts_score
FROM adresa a
WHERE MATCH(a.search) AGAINST('+Dlouha* +Praha*' IN BOOLEAN MODE)
  AND (
    (a.cislo_domovni IN (12, 5) OR a.cislo_orientacni IN (12, 5))
    OR a.domovni_orientacni_klic IN ('12_5', '5_12', '12_12', '5_5')
    OR a.orientacni_domovni_klic IN ('5_12', '12_5', '12_12', '5_5')
    OR (a.cislo_domovni = 12 AND a.cislo_orientacni = 5)
    OR (a.cislo_domovni = 5  AND a.cislo_orientacni = 12)
  )
LIMIT 20
```

---

## 5b) build_p2_filter — číslo v názvu ulice (priorita 400)

Určeno pro ulice pojmenované číselně, např. *17. listopadu*, *5. května*.
Pole `ulice_cislo` je denormalizovaný INT vyextrahovaný z názvu ulice při importu.

```text
function build_p2_filter(parsed):
    ph = join(["?"] * count(number_candidates), ", ")
    where = "a.ulice_cislo IN ({ph})"
    binds = values(number_candidates)
    return where, binds
```

**Příklad vstupu:** `"17 listopadu 25"`
→ `number_candidates = {17, 25}`

```sql
SELECT ..., 400 AS priority, MATCH(a.search) AGAINST('+listopadu*' IN BOOLEAN MODE) AS fts_score
FROM adresa a
WHERE MATCH(a.search) AGAINST('+listopadu*' IN BOOLEAN MODE)
  AND (a.ulice_cislo IN (17, 25))
LIMIT 20
```

---

## 5c) build_p3_filter — obvod Prahy (priorita 300)

Aktivuje se pouze pokud `"praha"` je v `text_tokens` A zároveň máme číslo 1–22.

```text
function build_p3_filter(parsed):
    obvod_candidates = filter(number_candidates, 1 <= x <= 22)
    ph = join(["?"] * count(obvod_candidates), ", ")
    where = "a.obvod_prahy_cislo IN ({ph})"
    binds = values(obvod_candidates)
    return where, binds
```

**Příklad vstupu:** `"Praha 5"`
→ `text_tokens = ["Praha"]`, `obvod_candidates = {5}`

```sql
SELECT ..., 300 AS priority, MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE) AS fts_score
FROM adresa a
WHERE MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE)
  AND (a.obvod_prahy_cislo IN (5))
LIMIT 20
```

---

## 5d) build_p4a_filter — PSČ exact (priorita 220)

Pro **přesně 5ciferná** čísla nebo dvojici 3+2 tokenů.

```text
function build_p4a_filter(parsed):
    ph = join(["?"] * count(psc_exact_candidates), ", ")
    where = "a.psc IN ({ph})"
    binds = values(psc_exact_candidates)
    return where, binds
```

**Příklad vstupu:** `"Praha 25101"` nebo `"Praha 251 01"`
→ `psc_exact_candidates = {25101}`

```sql
SELECT ..., 220 AS priority, MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE) AS fts_score
FROM adresa a
WHERE MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE)
  AND (a.psc IN (25101))
LIMIT 20
```

---

## 5e) build_p4b_filter — PSČ prefix / range (priorita 210)

Pro **2–4ciferné** číselné tokeny — expanduje se na rozsah PSČ pomocí `BETWEEN`.

```text
# Pravidla pro expanzi prefixu:
#   2 cifry (25)   → 25000 .. 25999  (range = 1000)
#   3 cifry (251)  → 25100 .. 25199  (range = 100)
#   4 cifry (2510) → 25100 .. 25109  (range = 10)
function expand_psc_prefix(value, length):
    multiplier = 10 ^ (5 - length)
    low  = value * multiplier
    high = low + multiplier - 1
    return low, high

function build_p4b_filter(parsed):
    conditions = []
    binds = []
    for (value, length) in psc_prefix_candidates:
        low, high = expand_psc_prefix(value, length)
        conditions.push("a.psc BETWEEN ? AND ?")
        binds.extend([low, high])
    where = join(conditions, " OR ")
    return where, binds
```

**Příklad vstupu:** `"Praha 251"`
→ `psc_prefix_candidates = {251}` (délka 3) → rozsah 25100–25199

```sql
SELECT ..., 210 AS priority, MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE) AS fts_score
FROM adresa a
WHERE MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE)
  AND (a.psc BETWEEN 25100 AND 25199)
LIMIT 20
```

---

## 5f) P5 — fallback FTS (priorita 100)

Vždy přítomná záchranná větev bez dodatečného filtru. Vrátí vše, co matchuje FTS.

```sql
SELECT ..., 100 AS priority, MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE) AS fts_score
FROM adresa a
WHERE MATCH(a.search) AGAINST('+Praha*' IN BOOLEAN MODE)
  AND (1=1)
LIMIT 20
```

---

## 6) Deduplikace a finalni order

```text
function build_final_sql(branches):
    union_sql = join(branches, " UNION ALL ")

    final_sql = """
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
            ORDER BY ranked.priority DESC, ranked.fts_score DESC
          ) AS rn
        FROM (
          {union_sql}
        ) ranked
      ) dedup
      WHERE dedup.rn = 1
      ORDER BY dedup.priority DESC, dedup.fts_score DESC
      LIMIT 20
    """

    return final_sql
```

## 7) End-to-end pseudokod search_adresa

```text
function search_adresa(input):
    parsed = parse_input(input)
    if parsed is empty:
        return []

    fts_query = build_fts_query(parsed.text_tokens)
    if fts_query is none:
        return []

    branches, binds = build_priority_branches(parsed, fts_query)
    final_sql = build_final_sql(branches)

    query = sqlx.query_as<Adresa>(final_sql)
    for b in binds:
        query.bind(b)

    return query.fetch_all(pool)
```

## 8) Doporucene indexy

```sql
-- ponechat
FULLTEXT INDEX ft_search (search)
INDEX idx_psc (psc)
INDEX idx_ulice_cislo (ulice_cislo)
INDEX idx_obvod_prahy_cislo (obvod_prahy_cislo)
INDEX idx_domovni_orientacni_klic (domovni_orientacni_klic)
INDEX idx_orientacni_domovni_klic (orientacni_domovni_klic)
INDEX idx_domovni_orientacni (cislo_domovni, cislo_orientacni)
INDEX idx_orientacni_domovni (cislo_orientacni, cislo_domovni)
```

Poznamka:
- `BETWEEN` nad `psc` umi vyuzit B-tree index (`idx_psc`), takze interval je obvykle rychly.
- realny dopad overit pres `EXPLAIN ANALYZE` na typickych vstupech.

## 9) Minimalni test checklist

- `"17 listopadu 25"` -> P2 ma byt vys nez fallback
- `"praha 5"` -> P3 vys nez fallback
- `"5 kvetna 12"` -> cislo v nazvu ulice se nechova jako domovni, pokud neodpovida P1
- `"25101 praha"` -> P4 exact aktivni
- `"251 praha"` -> P4 prefix/range aktivni
- `"10/12 praha"` -> P1 slash pair aktivni
- duplicity stejneho `kod_adm` se vraci jen jednou

