use leptos::prelude::*;
use leptos_meta::{provide_meta_context, Stylesheet};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment, WildcardSegment,
};

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/vyhledavac-addres.css"/>
        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=move || "Not found.">
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=WildcardSegment("any") view=NotFound/>
                </Routes>
            </main>
        </Router>
    }
}

use leptos::callback::Callback;
use core_db::Adresa;

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    let address = RwSignal::new(Adresa {
        kod_adm: 0,
        kod_obce: 0,
        nazev_obce: String::new(),
        kod_momc: None,
        nazev_momc: None,
        kod_obvodu_prahy: None,
        nazev_obvodu_prahy: None,
        kod_casti_obce: None,
        nazev_casti_obce: None,
        kod_ulice: None,
        nazev_ulice: None,
        typ_so: String::new(),
        cislo_domovni: 0,
        cislo_orientacni: None,
        znak_cisla_orientacniho: None,
        psc: String::new(),
        souradnice_y: None,
        souradnice_x: None,
        plati_od: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap(),
        search: String::new(),
    });

    view! {
        <div class="home-container">
            <div class="display-area">
                <crate::components::AddressDisplay
                    address=Signal::from(address)
                />
            </div>
            <crate::components::SearchInput
                placeholder="Search addresses..."
                on_select=Callback::new(move |addr| address.set(addr))
            />
        </div>
    }
}

/// 404 - Not Found
#[component]
fn NotFound() -> impl IntoView {
    // set an HTTP status code 404
    // this is feature gated because it can only be done during
    // initial server-side rendering
    // if you navigate to the 404 page subsequently, the status
    // code will not be set because there is not a new HTTP request
    // to the server
    #[cfg(feature = "ssr")]
    {
        // this can be done inline because it's synchronous
        // if it were async, we'd use a server function
        let resp = expect_context::<leptos_actix::ResponseOptions>();
        resp.set_status(actix_web::http::StatusCode::NOT_FOUND);
    }

    view! {
        <h1>"Not Found"</h1>
    }
}
