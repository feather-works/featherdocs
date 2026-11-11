//! As fontes no `.docx`: o Word guarda o cadastro de fontes (Referências → Gerenciar fontes) num
//! XML à parte, `customXml/item1.xml`, com a raiz `b:Sources`. Aqui se grava e se lê esse XML, e
//! o estilo de citação vai no atributo `StyleName`.

use std::fmt::Write as _;

use featherdocs_core::bibliografia::estilo_do_nome;
use featherdocs_core::modelo::{Autor, Bibliografia, EstiloDeCitacao, Fonte, TipoDeFonte};
use quick_xml::events::Event;
use quick_xml::Reader;

const NS_FONTES: &str = "http://schemas.openxmlformats.org/officeDocument/2006/bibliography";

use crate::docx_gravar::esc;

/// O tipo do Word (`b:SourceType`) de cada tipo de fonte.
fn tipo_word(t: TipoDeFonte) -> &'static str {
    match t {
        TipoDeFonte::Livro => "Book",
        TipoDeFonte::Capitulo => "BookSection",
        TipoDeFonte::Artigo => "JournalArticle",
        TipoDeFonte::Site => "InternetSite",
        TipoDeFonte::Tese | TipoDeFonte::Relatorio => "Report",
        TipoDeFonte::Outro => "Misc",
    }
}

fn nome_do_estilo(e: EstiloDeCitacao) -> &'static str {
    match e {
        EstiloDeCitacao::Abnt => "ABNT",
        EstiloDeCitacao::Apa => "APA",
        EstiloDeCitacao::Vancouver => "Vancouver",
        EstiloDeCitacao::Ieee => "IEEE",
        EstiloDeCitacao::Chicago => "Chicago",
    }
}

/// O `b:Sources` com as fontes do documento.
pub(crate) fn gravar(b: &Bibliografia) -> String {
    let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<b:Sources SelectedStyle=\"\" StyleName=\"{}\" xmlns:b=\"{NS_FONTES}\" xmlns=\"{NS_FONTES}\">", nome_do_estilo(b.estilo));
    for f in &b.fontes {
        let _ = write!(s, "<b:Source><b:Tag>{}</b:Tag><b:SourceType>{}</b:SourceType>", esc(&f.chave), tipo_word(f.tipo));
        if !f.autores.is_empty() {
            s.push_str("<b:Author><b:Author><b:NameList>");
            for a in &f.autores {
                let _ = write!(s, "<b:Person><b:Last>{}</b:Last><b:First>{}</b:First></b:Person>", esc(&a.sobrenome), esc(&a.nome));
            }
            s.push_str("</b:NameList></b:Author></b:Author>");
        }
        let veiculo = match f.tipo {
            TipoDeFonte::Artigo => "JournalName",
            TipoDeFonte::Capitulo => "BookTitle",
            TipoDeFonte::Tese => "ReportType",
            _ => "Comments",
        };
        let titulo = if f.subtitulo.is_empty() { f.titulo.clone() } else { format!("{}: {}", f.titulo, f.subtitulo) };
        let tese = if f.tipo == TipoDeFonte::Tese && f.veiculo.is_empty() { "Tese" } else { &f.veiculo };
        for (tag, v) in [("Title", titulo.as_str()), ("Year", &f.ano), ("Edition", &f.edicao), ("City", &f.local), ("Publisher", &f.editora), (veiculo, tese), ("Volume", &f.volume), ("Issue", &f.numero), ("Pages", &f.paginas), ("URL", &f.url), ("DOI", &f.doi), ("AccessedDate", &f.acesso)] {
            if !v.is_empty() {
                let _ = write!(s, "<b:{tag}>{}</b:{tag}>", esc(v));
            }
        }
        s.push_str("</b:Source>");
    }
    s.push_str("</b:Sources>");
    s
}

/// O `itemProps1.xml` que acompanha a parte (o Word quer um GUID e o espaço de nomes).
pub(crate) fn propriedades() -> String {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n<ds:datastoreItem ds:itemID=\"{{A5F0D1E6-4E9B-4C1A-9C1D-FD0C5F0E0001}}\" xmlns:ds=\"http://schemas.openxmlformats.org/officeDocument/2006/customXml\"><ds:schemaRefs><ds:schemaRef ds:uri=\"{NS_FONTES}\"/></ds:schemaRefs></ds:datastoreItem>")
}

/// Uma fonte em leitura: o elemento aberto e o autor em montagem.
#[derive(Default)]
struct Lendo {
    fonte: Option<Fonte>,
    autor: Option<Autor>,
    elemento: String,
    tipo_word: String,
    /// O `b:First`/`b:Middle` acabou de abrir: o próximo pedaço de texto é outro nome (leva espaço).
    nome_novo: bool,
    /// Dentro de `b:Editor`, `b:Translator`…: as pessoas dali não são autores.
    outro_papel: bool,
}

impl Lendo {
    fn texto(&mut self, t: &str) {
        let Some(f) = &mut self.fonte else { return };
        if let Some(a) = &mut self.autor {
            match self.elemento.as_str() {
                "b:Last" => a.sobrenome.push_str(t),
                "b:First" | "b:Middle" => {
                    // Um nome chega em vários pedaços (`Fran&#231;ois`): o espaço só entre nomes.
                    if std::mem::take(&mut self.nome_novo) && !a.nome.is_empty() {
                        a.nome.push(' ');
                    }
                    a.nome.push_str(t);
                }
                _ => {}
            }
            return;
        }
        let campo = match self.elemento.as_str() {
            "b:Tag" => &mut f.chave,
            "b:Title" => &mut f.titulo,
            "b:Year" => &mut f.ano,
            "b:Edition" => &mut f.edicao,
            "b:City" => &mut f.local,
            "b:Publisher" | "b:Institution" => &mut f.editora,
            "b:JournalName" | "b:BookTitle" | "b:ReportType" | "b:PeriodicalTitle" => &mut f.veiculo,
            "b:Volume" => &mut f.volume,
            "b:Issue" => &mut f.numero,
            "b:Pages" => &mut f.paginas,
            "b:URL" => &mut f.url,
            "b:DOI" => &mut f.doi,
            "b:AccessedDate" => &mut f.acesso,
            "b:SourceType" => &mut self.tipo_word,
            _ => return,
        };
        campo.push_str(t);
    }

    fn fim_da_fonte(&mut self) -> Option<Fonte> {
        let mut f = self.fonte.take()?;
        f.tipo = match self.tipo_word.as_str() {
            "Book" => TipoDeFonte::Livro,
            "BookSection" | "ConferenceProceedings" => TipoDeFonte::Capitulo,
            "JournalArticle" | "ArticleInAPeriodical" => TipoDeFonte::Artigo,
            "InternetSite" | "DocumentFromInternetSite" => TipoDeFonte::Site,
            "Report" if ["tese", "thesis", "disserta"].iter().any(|p| f.veiculo.to_lowercase().contains(p)) => TipoDeFonte::Tese,
            "Report" => TipoDeFonte::Relatorio,
            _ => TipoDeFonte::Outro,
        };
        if let Some((t, sub)) = f.titulo.split_once(": ") {
            (f.titulo, f.subtitulo) = (t.to_string(), sub.to_string());
        }
        self.tipo_word.clear();
        Some(f)
    }
}

/// Lê um `b:Sources` (`None` se o XML não é de fontes).
pub(crate) fn ler(xml: &[u8]) -> Option<Bibliografia> {
    let mut r = Reader::from_reader(xml);
    let mut b = Bibliografia::default();
    let mut achou = false;
    let mut l = Lendo::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let nome = e.name().as_ref().to_string();
                match nome.as_str() {
                    "b:Sources" => {
                        achou = true;
                        let estilo = crate::docx::atributo(&e, "StyleName").map(|v| v.to_lowercase());
                        b.estilo = estilo.as_deref().and_then(estilo_do_nome).unwrap_or_default();
                    }
                    "b:Source" => (l.fonte, l.outro_papel) = (Some(Fonte::default()), false),
                    "b:Person" => l.autor = Some(Autor::default()),
                    "b:First" | "b:Middle" => l.nome_novo = true,
                    "b:Editor" | "b:Translator" | "b:Compiler" | "b:Interviewer" | "b:Interviewee" | "b:Inventor" | "b:Counsel" | "b:Director" | "b:Performer" | "b:Producer" | "b:Writer" | "b:Conductor" | "b:Composer" | "b:Artist" => l.outro_papel = true,
                    _ => {}
                }
                l.elemento = nome;
            }
            Ok(Event::Text(t)) => l.texto(&t.xml10_content()),
            Ok(Event::GeneralRef(r)) => l.texto(&crate::docx::referencia(&r)),
            Ok(Event::End(e)) => match e.name().as_ref() {
                "b:Person" => {
                    if let (Some(a), Some(f), false) = (l.autor.take(), l.fonte.as_mut(), l.outro_papel) {
                        f.autores.push(a);
                    }
                }
                "b:Editor" | "b:Translator" | "b:Compiler" | "b:Interviewer" | "b:Interviewee" | "b:Inventor" | "b:Counsel" | "b:Director" | "b:Performer" | "b:Producer" | "b:Writer" | "b:Conductor" | "b:Composer" | "b:Artist" => l.outro_papel = false,
                "b:Source" => b.fontes.extend(l.fim_da_fonte()),
                _ => l.elemento.clear(),
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    achou.then_some(b)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn fontes_vao_e_voltam_pelo_xml_do_word() {
        let b = Bibliografia {
            estilo: EstiloDeCitacao::Vancouver,
            fontes: vec![
                Fonte { chave: "a&b".into(), tipo: TipoDeFonte::Artigo, autores: vec![Autor { sobrenome: "Silva".into(), nome: "Ana Maria".into() }], titulo: "Redes".into(), subtitulo: "teoria".into(), ano: "2020".into(), veiculo: "Revista".into(), volume: "3".into(), ..Fonte::default() },
                Fonte { chave: "t".into(), tipo: TipoDeFonte::Tese, titulo: "Tese".into(), veiculo: "Tese (Doutorado)".into(), editora: "USP".into(), ..Fonte::default() },
            ],
            sem_sl_sn: false,
        };
        assert_eq!(ler(gravar(&b).as_bytes()), Some(b));
        assert_eq!(ler(b"<outro/>"), None);
        let xml = br#"<b:Sources xmlns:b="x"><b:Source><b:Tag>k</b:Tag><b:Author><b:Author><b:NameList><b:Person><b:Last>Dupont</b:Last><b:First>Fran&#231;ois</b:First><b:Middle>Marie</b:Middle></b:Person></b:NameList></b:Author><b:Editor><b:NameList><b:Person><b:Last>Ed</b:Last></b:Person></b:NameList></b:Editor></b:Author></b:Source></b:Sources>"#;
        let autores = &ler(xml).unwrap().fontes[0].autores;
        assert_eq!(autores.len(), 1, "o editor não é autor");
        assert_eq!(autores[0].nome, "François Marie");
    }
}
