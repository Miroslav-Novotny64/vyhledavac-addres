CREATE TABLE IF NOT EXISTS adresa (
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
    psc VARCHAR(10) NOT NULL,
    souradnice_y DOUBLE DEFAULT NULL,
    souradnice_x DOUBLE DEFAULT NULL,
    plati_od DATETIME NOT NULL,
    
    search TEXT NOT NULL,

    PRIMARY KEY (kod_adm),
    FULLTEXT INDEX ft_search (search)
)
