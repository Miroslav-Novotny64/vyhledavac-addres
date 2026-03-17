use leptos::prelude::*;
use core_db::Adresa;

#[component]
pub fn AddressDisplay(#[prop(into)] address: Signal<Adresa>) -> impl IntoView {
    view! {
        <div class="address-display">
            <Show
                when=move || address.get().kod_adm != 0
                fallback=move || view! { <p class="no-address">"No address selected"</p> }
            >
                {move || {
                    let adr = address.get();
                    let street = adr.nazev_ulice.clone()
                        .or_else(|| adr.nazev_casti_obce.clone())
                        .unwrap_or_else(|| adr.nazev_obce.clone());

                    let numbers = format!("{}{}", 
                        adr.cislo_domovni, 
                        adr.cislo_orientacni.map(|o| format!("/{}", o)).unwrap_or_default()
                    );

                    let city_part = adr.nazev_momc.clone()
                        .or_else(|| adr.nazev_casti_obce.clone())
                        .filter(|name| name != &adr.nazev_obce);

                    let location = match city_part {
                        Some(part) => format!("{}, {} {}", part, adr.nazev_obce, adr.psc),
                        None => format!("{} {}", adr.nazev_obce, adr.psc),
                    };

                    view! {
                        <div class="address-card">
                            <h2>{street} " " {numbers}</h2>
                            <p>{location}</p>
                            <p class="postal-code">"PSC: " {adr.psc}</p>
                        </div>
                    }
                }}
            </Show>
        </div>
    }
}