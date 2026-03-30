CREATE TABLE IF NOT EXISTS adresa (
    -- definované v csv
    kod_adm INT NOT NULL,
    kod_obce INT NOT NULL,
    nazev_obce VARCHAR(255) NOT NULL,
    kod_momc INT DEFAULT NULL,
    nazev_momc VARCHAR(255) DEFAULT NULL,
    kod_obvodu_prahy INT DEFAULT NULL,
    nazev_obvodu_prahy VARCHAR(255) DEFAULT NULL,
    kod_casti_obce INT DEFAULT NULL,
    nazev_casti_obce VARCHAR(255) DEFAULT NULL,
    kod_ulice INT DEFAULT NULL,
    nazev_ulice VARCHAR(255) DEFAULT NULL,
    typ_so VARCHAR(50) NOT NULL,
    cislo_domovni INT NOT NULL,
    cislo_orientacni INT DEFAULT NULL,
    znak_cisla_orientacniho VARCHAR(10) DEFAULT NULL,
    psc INT NOT NULL,
    souradnice_y DOUBLE DEFAULT NULL,
    souradnice_x DOUBLE DEFAULT NULL,
    plati_od DATETIME NOT NULL,

    -- Dodatečné argumenty
    ulice_cislo INT DEFAULT NULL,
    obvod_prahy_cislo INT DEFAULT NULL,
    domovni_orientacni_klic VARCHAR(32) DEFAULT NULL,
    orientacni_domovni_klic VARCHAR(32) DEFAULT NULL,
    search TEXT NOT NULL,

    -- indexy
    PRIMARY KEY (kod_adm),
    FULLTEXT INDEX ft_search (search),
    INDEX idx_cislo_domovni (cislo_domovni),
    INDEX idx_cislo_orientacni (cislo_orientacni),
    INDEX idx_psc (psc),
    INDEX idx_ulice_cislo (ulice_cislo),
    INDEX idx_obvod_prahy_cislo (obvod_prahy_cislo),
    INDEX idx_domovni_orientacni_klic (domovni_orientacni_klic),
    INDEX idx_orientacni_domovni_klic (orientacni_domovni_klic),
    INDEX idx_domovni_orientacni (cislo_domovni, cislo_orientacni),
    INDEX idx_orientacni_domovni (cislo_orientacni, cislo_domovni),
    INDEX idx_psc_domovni (psc, cislo_domovni),
    INDEX idx_psc_orientacni (psc, cislo_orientacni)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;