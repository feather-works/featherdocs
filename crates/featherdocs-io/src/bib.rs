//! BibTeX (`.bib`): ler as fontes de um arquivo (o que Zotero, Mendeley, JabRef e o Google
//! Acadêmico exportam) e gravá-las de volta. Lê `@tipo{chave, campo = {valor} | "valor" | número}`,
//! com chaves aninhadas, concatenação `#` simples e os acentos do LaTeX mais comuns.

use featherdocs_core::modelo::{Autor, Fonte, TipoDeFonte};

/// O tipo da fonte a partir do tipo do BibTeX.
fn tipo_do_bibtex(t: &str) -> TipoDeFonte {
    match t.to_ascii_lowercase().as_str() {
        "article" => TipoDeFonte::Artigo,
        "incollection" | "inbook" | "inproceedings" | "conference" => TipoDeFonte::Capitulo,
        "online" | "misc" | "electronic" | "www" => TipoDeFonte::Site,
        "phdthesis" | "mastersthesis" | "thesis" => TipoDeFonte::Tese,
        "techreport" | "report" => TipoDeFonte::Relatorio,
        "book" | "booklet" | "manual" => TipoDeFonte::Livro,
        _ => TipoDeFonte::Outro,
    }
}

fn tipo_para_bibtex(t: TipoDeFonte) -> &'static str {
    match t {
        TipoDeFonte::Livro => "book",
        TipoDeFonte::Capitulo => "incollection",
        TipoDeFonte::Artigo => "article",
        TipoDeFonte::Site => "online",
        TipoDeFonte::Tese => "phdthesis",
        TipoDeFonte::Relatorio => "techreport",
        TipoDeFonte::Outro => "misc",
    }
}

/// Os acentos do LaTeX que aparecem em `.bib`: `{\'a}`, `\~{a}`, `\c{c}`…
fn acentos(s: &str) -> String {
    const TROCAS: [(&str, &str); 40] = [
        ("\\'a", "á"), ("\\'e", "é"), ("\\'i", "í"), ("\\'o", "ó"), ("\\'u", "ú"), ("\\'A", "Á"), ("\\'E", "É"), ("\\'I", "Í"), ("\\'O", "Ó"), ("\\'U", "Ú"),
        ("\\`a", "à"), ("\\`A", "À"), ("\\^a", "â"), ("\\^e", "ê"), ("\\^o", "ô"), ("\\^A", "Â"), ("\\^E", "Ê"), ("\\^O", "Ô"),
        ("\\~a", "ã"), ("\\~o", "õ"), ("\\~n", "ñ"), ("\\~A", "Ã"), ("\\~O", "Õ"), ("\\c{c}", "ç"), ("\\c{C}", "Ç"), ("\\c c", "ç"),
        ("\\\"a", "ä"), ("\\\"o", "ö"), ("\\\"u", "ü"), ("\\\"U", "Ü"), ("\\'{\\i}", "í"), ("\\'\\i", "í"),
        ("\\&", "&"), ("\\%", "%"), ("\\_", "_"), ("\\$", "$"), ("\\#", "#"), ("--", "–"), ("~", " "), ("\\ ", " "),
    ];
    let mut t = s.to_string();
    // `\'{a}` vira `\'a` antes das trocas.
    for marca in ["\\'", "\\`", "\\^", "\\~", "\\\""] {
        let mut saida = String::new();
        let mut resto = t.as_str();
        while let Some(i) = resto.find(&format!("{marca}{{")) {
            saida.push_str(&resto[..i]);
            let depois = &resto[i + marca.len() + 1..];
            match depois.find('}') {
                Some(f) if f <= 3 => {
                    saida.push_str(marca);
                    saida.push_str(&depois[..f]);
                    resto = &depois[f + 1..];
                }
                _ => {
                    saida.push_str(&resto[i..i + marca.len() + 1]);
                    resto = depois;
                }
            }
        }
        saida.push_str(resto);
        t = saida;
    }
    for (de, para) in TROCAS {
        t = t.replace(de, para);
    }
    t.replace(['{', '}'], "")
}

/// O valor de um campo começando em `s` (já sem espaço na frente): `{…}`, `"…"` ou uma palavra.
/// Devolve o valor cru e quantos bytes leu.
fn valor(s: &str) -> (String, usize) {
    let mut chars = s.char_indices();
    match chars.next() {
        Some((_, '{')) => {
            let mut nivel = 0;
            for (i, c) in s.char_indices() {
                match c {
                    '{' => nivel += 1,
                    '}' => {
                        nivel -= 1;
                        if nivel == 0 {
                            return (s[1..i].to_string(), i + 1);
                        }
                    }
                    _ => {}
                }
            }
            (s[1..].to_string(), s.len())
        }
        Some((_, '"')) => {
            let mut nivel = 0;
            for (i, c) in s.char_indices().skip(1) {
                match c {
                    '{' => nivel += 1,
                    '}' => nivel -= 1,
                    '"' if nivel == 0 => return (s[1..i].to_string(), i + 1),
                    _ => {}
                }
            }
            (s[1..].to_string(), s.len())
        }
        _ => {
            let fim = s.find([',', '}', '#', '\n']).unwrap_or(s.len());
            (s[..fim].trim().to_string(), fim)
        }
    }
}

/// Os campos de uma entrada, de depois da chave até o `}` que a fecha.
fn campos(corpo: &str) -> Vec<(String, String)> {
    let mut v = Vec::new();
    let mut resto = corpo;
    while let Some(igual) = resto.find('=') {
        let nome = resto[..igual].trim().trim_start_matches(',').trim().to_ascii_lowercase();
        let mut depois = resto[igual + 1..].trim_start();
        let mut texto = String::new();
        loop {
            let (v, n) = valor(depois);
            texto.push_str(&v);
            depois = depois[n..].trim_start();
            match depois.strip_prefix('#') {
                Some(d) => depois = d.trim_start(),
                None => break,
            }
        }
        // Os autores ficam com as chaves: `{Agência Nacional}` é uma entidade, e o "and" de dentro não separa.
        let limpo = if nome == "author" || nome == "editor" { texto } else { acentos(&texto) };
        v.push((nome, limpo.split_whitespace().collect::<Vec<_>>().join(" ")));
        resto = depois.trim_start_matches(',');
    }
    v
}

/// "Silva, João and Souza, Maria" ou "João Silva and Maria Souza"; `{Agência Nacional}` (tudo entre
/// chaves) é uma entidade e vai inteira no sobrenome. Recebe o texto ainda com as chaves.
fn autores(s: &str) -> Vec<Autor> {
    // Separa no " and " fora de chaves.
    let mut partes = Vec::new();
    let (mut nivel, mut inicio) = (0i32, 0);
    for (i, c) in s.char_indices() {
        match c {
            '{' => nivel += 1,
            '}' => nivel -= 1,
            ' ' if nivel == 0 && s[i..].starts_with(" and ") => {
                partes.push(&s[inicio..i]);
                inicio = i + 5;
            }
            _ => {}
        }
    }
    partes.push(&s[inicio..]);
    partes
        .into_iter()
        .map(str::trim)
        // O "and others" do BibTeX é o "et al.", não um autor chamado Others.
        .filter(|a| !a.is_empty() && !a.eq_ignore_ascii_case("others"))
        .map(|a| {
            // Entidade: a chave que abre no começo só fecha no último caractere.
            let mut nivel = 0;
            let fecha_no_fim = a.len() >= 2 && a.starts_with('{')
                && a.char_indices().all(|(i, c)| {
                    nivel += (c == '{') as i32 - (c == '}') as i32;
                    nivel > 0 || i + 1 == a.len()
                });
            match fecha_no_fim.then(|| &a[1..a.len() - 1]) {
                Some(e) => Autor { sobrenome: acentos(e), nome: String::new() },
                None => pessoa(&acentos(a)),
            }
        })
        .collect()
}

fn pessoa(a: &str) -> Autor {
    match a.split_once(',') {
        Some((sobrenome, nome)) => Autor { sobrenome: sobrenome.trim().to_string(), nome: nome.trim().to_string() },
        None => match a.rsplit_once(' ') {
            Some((nome, sobrenome)) => Autor { sobrenome: sobrenome.to_string(), nome: nome.to_string() },
            None => Autor { sobrenome: a.to_string(), nome: String::new() },
        },
    }
}

fn fonte_de(tipo: &str, chave: &str, campos: Vec<(String, String)>) -> Fonte {
    let mut f = Fonte { chave: chave.trim().to_string(), tipo: tipo_do_bibtex(tipo), ..Fonte::default() };
    for (nome, v) in campos {
        match nome.as_str() {
            "author" | "editor" if f.autores.is_empty() => f.autores = autores(&v),
            "title" => f.titulo = v,
            "subtitle" => f.subtitulo = v,
            "year" => f.ano = v,
            "date" if f.ano.is_empty() => f.ano = v.chars().take(4).collect(),
            "edition" => f.edicao = v,
            "address" | "location" => f.local = v,
            "publisher" | "school" | "institution" | "organization" => f.editora = v,
            "journal" | "journaltitle" | "booktitle" | "type" => f.veiculo = v,
            "volume" => f.volume = v,
            "number" | "issue" => f.numero = v,
            "pages" => f.paginas = v,
            "url" => f.url = v,
            "urldate" | "accessed" | "note" if f.acesso.is_empty() && (nome != "note" || v.to_lowercase().contains("acess")) => {
                // "Acesso em 10 out. 2024" no campo: a referência já escreve o "Acesso em:".
                let t = v.trim();
                let sem = t.get(..9).filter(|p| p.eq_ignore_ascii_case("acesso em")).map_or(t, |_| t[9..].trim_start_matches(':').trim());
                f.acesso = sem.to_string();
            }
            "doi" => f.doi = v,
            "keywords" => f.palavras = v,
            // `note = {\url{https://…}}`: só o endereço (as chaves já saíram).
            "note" => f.nota = v.trim().trim_start_matches("\\url").trim_matches(['{', '}']).to_string(),
            _ => {}
        }
    }
    f
}

/// Lê as entradas de um `.bib` (`@comment`, `@string` e `@preamble` são pulados).
pub fn ler(texto: &str) -> Vec<Fonte> {
    let mut v = Vec::new();
    let mut resto = texto;
    while let Some(arroba) = resto.find('@') {
        resto = &resto[arroba + 1..];
        let Some(abre) = resto.find(['{', '(']) else { break };
        let tipo = resto[..abre].trim().to_string();
        let corpo_e_resto = &resto[abre..];
        // `@book(chave, …)` vale como `@book{…}`; só o parêntese que abre a entrada vira chave (um
        // `(` dentro de uma URL não).
        let (corpo, n) = match corpo_e_resto.strip_prefix('(') {
            Some(r) => valor(&format!("{{{}", r.replacen(')', "}", 1))),
            None => valor(corpo_e_resto),
        };
        resto = &corpo_e_resto[n.min(corpo_e_resto.len())..];
        if matches!(tipo.to_ascii_lowercase().as_str(), "comment" | "string" | "preamble") {
            continue;
        }
        let Some((chave, campos_txt)) = corpo.split_once(',') else { continue };
        v.push(fonte_de(&tipo, chave, campos(campos_txt)));
    }
    v
}

fn escapar(s: &str) -> String {
    s.replace('&', "\\&").replace('%', "\\%").replace('#', "\\#").replace('_', "\\_")
}

/// Grava as fontes em BibTeX (UTF-8, sem trocar acento por comando).
pub fn gravar(fontes: &[Fonte]) -> String {
    let mut s = String::new();
    for f in fontes {
        s.push_str(&format!("@{}{{{},\n", tipo_para_bibtex(f.tipo), f.chave));
        let autores = f.autores.iter().map(|a| if a.nome.is_empty() && a.sobrenome.contains(' ') { format!("{{{}}}", a.sobrenome) } else if a.nome.is_empty() { a.sobrenome.clone() } else { format!("{}, {}", a.sobrenome, a.nome) }).collect::<Vec<_>>().join(" and ");
        let veiculo = match f.tipo {
            TipoDeFonte::Artigo => "journal",
            TipoDeFonte::Capitulo => "booktitle",
            _ => "type",
        };
        let editora = match f.tipo {
            TipoDeFonte::Tese => "school",
            TipoDeFonte::Relatorio => "institution",
            _ => "publisher",
        };
        for (nome, v) in [
            ("author", autores.as_str()),
            ("title", f.titulo.as_str()),
            ("subtitle", f.subtitulo.as_str()),
            ("year", f.ano.as_str()),
            ("edition", f.edicao.as_str()),
            ("address", f.local.as_str()),
            (editora, f.editora.as_str()),
            (veiculo, f.veiculo.as_str()),
            ("volume", f.volume.as_str()),
            ("number", f.numero.as_str()),
            ("pages", f.paginas.as_str()),
            ("doi", f.doi.as_str()),
            ("keywords", f.palavras.as_str()),
            ("note", f.nota.as_str()),
            ("url", f.url.as_str()),
            ("urldate", f.acesso.as_str()),
        ] {
            if !v.is_empty() {
                let v = if nome == "url" || nome == "doi" { v.to_string() } else { escapar(v) };
                s.push_str(&format!("  {nome} = {{{v}}},\n"));
            }
        }
        s.push_str("}\n\n");
    }
    s
}

#[cfg(test)]
mod testes {
    use super::*;

    const BIB: &str = r#"
@comment{gerado pelo Zotero}
@Book{silva2023,
  author    = {Silva, Jo{\~a}o Pedro and Maria Souza},
  title     = {Redes el\'{e}tricas},
  subtitle  = "teoria e pr{\'a}tica",
  year      = 2023,
  edition   = {2},
  address   = {S{\~a}o Paulo},
  publisher = {Blucher},
}
@article{lima2020, author = "Lima, Ana", title = {Prote{\c{c}}{\~a}o de {T}ransformadores},
  journal = {Revista} # { Eletricidade}, volume = {12}, number = {3}, pages = {10--20}, year = {2020},
  doi = {10.1000/xyz} }
"#;

    #[test]
    fn autor_com_chave_aberta_nao_derruba() {
        let f = ler("@misc{k, author = {A and {}, title = {T}}");
        assert!(f.len() <= 1);
        let _ = ler("@misc{k, author = A and {\n}");
    }

    #[test]
    fn le_livro_e_artigo_com_acentos_e_concatenacao() {
        let f = ler(BIB);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].chave, "silva2023");
        assert_eq!(f[0].tipo, TipoDeFonte::Livro);
        assert_eq!(f[0].autores, vec![Autor { sobrenome: "Silva".into(), nome: "João Pedro".into() }, Autor { sobrenome: "Souza".into(), nome: "Maria".into() }]);
        assert_eq!((f[0].titulo.as_str(), f[0].subtitulo.as_str(), f[0].ano.as_str(), f[0].local.as_str()), ("Redes elétricas", "teoria e prática", "2023", "São Paulo"));
        assert_eq!(f[1].tipo, TipoDeFonte::Artigo);
        assert_eq!((f[1].titulo.as_str(), f[1].veiculo.as_str(), f[1].paginas.as_str(), f[1].doi.as_str()), ("Proteção de Transformadores", "Revista Eletricidade", "10–20", "10.1000/xyz"));
    }

    #[test]
    fn grava_e_le_de_volta_igual() {
        let f = ler(BIB);
        assert_eq!(ler(&gravar(&f)), f);
    }

    #[test]
    fn acesso_e_others_do_bibtex() {
        let f = &ler("@online{a, author={Rodrigues, W. and others}, title={T}, urldate={Acesso em 10 out. 2024}}")[0];
        assert_eq!(f.acesso, "10 out. 2024");
        assert_eq!(f.autores.len(), 1);
    }

    #[test]
    fn parentese_na_url_nao_engole_as_entradas_seguintes() {
        let f = ler("@misc{a, url={https://x.com/#:~:text=matriz%20(ou,de%20y.}, year={2024}}\n@misc{b, year={2020}}");
        assert_eq!(f.iter().map(|f| (f.chave.as_str(), f.ano.as_str())).collect::<Vec<_>>(), [("a", "2024"), ("b", "2020")]);
    }

    #[test]
    fn entidade_entre_chaves_e_and_dentro_delas() {
        let f = ler("@misc{a, author = {{Ag\\^encia Nacional de Energia} and {Barnes and Noble} and {Ag} {Nac}}, title={T}}");
        assert_eq!(
            f[0].autores,
            vec![
                Autor { sobrenome: "Agência Nacional de Energia".into(), nome: String::new() },
                Autor { sobrenome: "Barnes and Noble".into(), nome: String::new() },
                Autor { sobrenome: "Nac".into(), nome: "Ag".into() },
            ]
        );
        // Volta igual: a entidade é gravada entre chaves.
        assert_eq!(ler(&gravar(&f))[0].autores, f[0].autores);
    }
}
