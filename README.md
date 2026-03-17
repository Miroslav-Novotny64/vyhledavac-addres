# Vyhledávač Adres (RÚIAN)

Tento projekt je mou implementací úkolu zadaného v rámci **hodiny databází na střední škole**.

Cílem aplikace je rychlé a efektivní vyhledávání adres v České republice nad daty z RÚIAN.

## Hlavní technické zajímavosti

### Databázová část & Full-text Index
To nejdůležitější se odehrává "pod kapotou" v databázi. Hlavní výzvou bylo zajistit bleskové vyhledávání v milionech záznamů adres.

- **Full-text Index:** Využil jsem `FULLTEXT` index nad speciálně připraveným sloupcem `search`. To umožňuje vyhledávat i neúplné adresy nebo adresy v různém pořadí (např. "Praha Roztylská" i "Roztylská Praha").
- **Normalizace dat:** Při importu dat z CSV (RÚIAN) se vytvoří speciální sloupec `search` ve kterém jsou data normalizovaná (odstraněna diakritika, převedena na malá písmena a padding čísel, aby se mohl plně využít fulltext index).

### Rust & WebAssembly
Aplikace je napsána kompletně v **Rustu**, což zaručuje bezpečnost a výkon.

- **Leptos:** Webový frontend i backend (SSR) běží na frameworku Leptos.
- **WebAssembly (WASM):** Klientská část aplikace je kompilována do WASM, což umožňuje spouštět Rust kód přímo v prohlížeči. Chtěl jsem zkusit psát front end i v něčem jiném jazyce než Typescriptu, na který jsem zvyklý.

## Jak projekt spustit

### Požadavky
- Rust & Cargo
- `cargo-leptos` (`cargo install cargo-leptos`)
- Docker (pro databázi)

### Spuštění
1. **Databáze:** Spusťte databázi pomocí Dockeru:
   ```sh
   docker-compose up -d
   ```
2. **Import dat:** (Volitelné, pokud máte data v `/data`, nejlépe přímo stažéná z [ČÚZK](https://nahlizenidokn.cuzk.gov.cz/StahniAdresniMistaRUIAN.aspx)):
   ```sh
   cargo run -p importer
   ```
3. **Webová aplikace:**
   ```sh
   cd app
   cargo leptos watch
   ```
   Aplikace bude dostupná na `http://localhost:3000`.

---
*Vytvořeno jako školní projekt pro propojení znalostí z databází a programování v Rustu.*
